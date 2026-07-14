#include "logos_inspector_async_bridge.h"
#include "logos_protocol_host_transport.h"

#include "logos_protocol.h"
#include "logos_provider_interface.h"
#include "module_proxy.h"
#include "plugin_registry.h"

#include <QCoreApplication>
#include <QEventLoop>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonObject>
#include <QTemporaryDir>
#include <QThread>
#include <QVariant>
#include <QVariantList>

#include <nlohmann/json.hpp>

#include <array>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <string_view>
#include <thread>
#include <utility>
#include <vector>

namespace {
using Json = nlohmann::json;
using Clock = std::chrono::steady_clock;

[[noreturn]] void fail(
    std::string_view expression,
    std::string_view file,
    int line,
    std::string_view detail = {})
{
    std::string message = std::string(file) + ':' + std::to_string(line)
        + ": requirement failed: " + std::string(expression);
    if (!detail.empty()) {
        message += " (" + std::string(detail) + ')';
    }
    throw std::runtime_error(message);
}

#define REQUIRE(condition) \
    do { \
        if (!(condition)) { \
            fail(#condition, __FILE__, __LINE__); \
        } \
    } while (false)

constexpr std::array<std::string_view, 6> kModuleNames = {
    "blockchain_module",
    "storage_module",
    "delivery_module",
    "capability_module",
    "lez_indexer_module",
    "lez_core",
};

bool isTerminalStatus(std::string_view status)
{
    return status == "completed" || status == "dispatched" || status == "failed"
        || status == "canceled" || status == "timed_out";
}

Json parseJson(std::string_view text, std::string_view label)
{
    Json parsed = Json::parse(text.begin(), text.end(), nullptr, false);
    if (parsed.is_discarded()) {
        fail(label, __FILE__, __LINE__, text);
    }
    return parsed;
}

void pumpEventsOnce()
{
    QCoreApplication::processEvents(QEventLoop::AllEvents, 10);
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
}

void pumpEventsFor(std::chrono::milliseconds duration)
{
    const auto deadline = Clock::now() + duration;
    while (Clock::now() < deadline) {
        pumpEventsOnce();
    }
}

class IntegrationProvider final : public LogosProviderObject
{
public:
    explicit IntegrationProvider(QString moduleName)
        : moduleName_(std::move(moduleName))
    {
    }

    QVariant callMethod(const QString& method, const QVariantList& args) override
    {
        callThread_.store(QThread::currentThread(), std::memory_order_release);
        callCount_.fetch_add(1, std::memory_order_relaxed);

        if (moduleName_ == QLatin1String("storage_module")) {
            if (method == QLatin1String("returnsNull")) {
                return {};
            }
            if (method == QLatin1String("throws")) {
                throw std::runtime_error("provider failure");
            }
        }

        if (moduleName_ == QLatin1String("delivery_module")
            && method == QLatin1String("send")) {
            if (args.size() != 2 || args[0].toString() != QLatin1String("/topic")
                || args[1].toString() != QLatin1String("hello")) {
                throw std::runtime_error("delivery arguments crossed the host boundary incorrectly");
            }
            lastArgs_ = args;
            emitDeliveryReceipt("request-46", "hash-original");
            return QStringLiteral("request-46");
        }

        throw std::runtime_error("unexpected composed integration method");
    }

    bool informModuleToken(const QString&, const QString&) override
    {
        return true;
    }

    QJsonArray getMethods() override
    {
        if (moduleName_ == QLatin1String("storage_module")) {
            return {
                QJsonObject {
                    { QStringLiteral("name"), QStringLiteral("returnsNull") },
                    { QStringLiteral("isInvokable"), true },
                },
                QJsonObject {
                    { QStringLiteral("name"), QStringLiteral("throws") },
                    { QStringLiteral("isInvokable"), true },
                },
            };
        }
        if (moduleName_ == QLatin1String("delivery_module")) {
            return {
                QJsonObject {
                    { QStringLiteral("name"), QStringLiteral("send") },
                    { QStringLiteral("isInvokable"), true },
                },
                QJsonObject {
                    { QStringLiteral("name"), QStringLiteral("messageSent") },
                    { QStringLiteral("type"), QStringLiteral("event") },
                },
            };
        }
        return {};
    }

    void setEventListener(EventCallback callback) override
    {
        eventCallback_ = std::move(callback);
    }

    void init(void*) override {}

    QString providerName() const override
    {
        return moduleName_;
    }

    QString providerVersion() const override
    {
        return QStringLiteral("1.0.0");
    }

    void emitDeliveryReceipt(const QString& requestId, const QString& hash)
    {
        if (eventCallback_) {
            eventCallback_(
                QStringLiteral("messageSent"),
                QVariantList { requestId, hash });
        }
    }

    int callCount() const noexcept
    {
        return callCount_.load(std::memory_order_relaxed);
    }

    QThread* callThread() const noexcept
    {
        return callThread_.load(std::memory_order_acquire);
    }

    const QVariantList& lastArgs() const noexcept
    {
        return lastArgs_;
    }

private:
    QString moduleName_;
    EventCallback eventCallback_;
    std::atomic<int> callCount_ { 0 };
    std::atomic<QThread*> callThread_ { nullptr };
    QVariantList lastArgs_;
};

class RegisteredProviders
{
public:
    RegisteredProviders() = default;

    ~RegisteredProviders()
    {
        for (auto module = registeredModules_.rbegin();
             module != registeredModules_.rend();
             ++module) {
            static_cast<void>(PluginRegistry::unregisterPlugin(*module));
        }
    }

    RegisteredProviders(const RegisteredProviders&) = delete;
    RegisteredProviders& operator=(const RegisteredProviders&) = delete;

    void registerAll()
    {
        REQUIRE(lp_set_mode("local") == LP_OK);
        providers_.reserve(kModuleNames.size());
        proxies_.reserve(kModuleNames.size());
        registeredModules_.reserve(kModuleNames.size());

        for (const std::string_view moduleName : kModuleNames) {
            const QString module = QString::fromUtf8(
                moduleName.data(),
                static_cast<qsizetype>(moduleName.size()));
            const std::string token = "composed-token-" + std::string(moduleName);
            REQUIRE(lp_token_save(moduleName.data(), token.c_str()) == LP_OK);

            auto provider = std::make_unique<IntegrationProvider>(module);
            auto proxy = std::make_unique<ModuleProxy>(provider.get());
            REQUIRE(proxy->saveToken(
                QStringLiteral("logos_inspector"),
                QString::fromStdString(token)));
            PluginRegistry::registerPlugin(proxy.get(), module);
            REQUIRE(PluginRegistry::hasPlugin(module));

            registeredModules_.push_back(module);
            providers_.push_back(std::move(provider));
            proxies_.push_back(std::move(proxy));
        }
    }

    IntegrationProvider& provider(std::string_view moduleName)
    {
        for (const auto& provider : providers_) {
            if (provider->providerName().toStdString() == moduleName) {
                return *provider;
            }
        }
        fail("registered provider exists", __FILE__, __LINE__, moduleName);
    }

private:
    std::vector<std::unique_ptr<IntegrationProvider>> providers_;
    std::vector<std::unique_ptr<ModuleProxy>> proxies_;
    std::vector<QString> registeredModules_;
};

Json waitForBridgeResponse(
    LogosInspectorAsyncBridge& bridge,
    const std::string& admissionText,
    std::chrono::seconds timeout = std::chrono::seconds(10))
{
    const Json admission = parseJson(admissionText, "bridge admission JSON");
    REQUIRE(admission.value("ok", false));
    REQUIRE(admission.contains("value"));
    const std::string token = admission.at("value").value("token", std::string {});
    REQUIRE(!token.empty());

    const auto deadline = Clock::now() + timeout;
    while (Clock::now() < deadline) {
        pumpEventsOnce();
        const Json poll = parseJson(bridge.pollAsync(token), "bridge poll JSON");
        REQUIRE(poll.value("ok", false));
        const Json& value = poll.at("value");
        const std::string status = value.value("status", std::string {});
        if (status == "ready") {
            const Json response = parseJson(
                value.value("responseJson", std::string {}),
                "core response JSON");
            const Json released = parseJson(
                bridge.releaseAsync(token),
                "bridge release JSON");
            REQUIRE(released.value("ok", false));
            REQUIRE(released.at("value").value("released", false));
            return response;
        }
        REQUIRE(status == "pending");
    }
    fail("bridge response became ready", __FILE__, __LINE__, admissionText);
}

Json invokeInspector(
    LogosInspectorAsyncBridge& bridge,
    std::uint64_t& correlation,
    std::string_view method,
    const Json& args)
{
    const std::string correlationId = "composed-inspector-"
        + std::to_string(correlation++);
    return waitForBridgeResponse(
        bridge,
        bridge.callAsync(correlationId, std::string(method), args.dump()));
}

Json invokeModule(
    LogosInspectorAsyncBridge& bridge,
    std::uint64_t& correlation,
    std::string_view module,
    std::string_view method,
    const Json& args)
{
    const std::string correlationId = "composed-module-"
        + std::to_string(correlation++);
    return waitForBridgeResponse(
        bridge,
        bridge.callModuleAsync(
            correlationId,
            std::string(module),
            std::string(method),
            args.dump()));
}

std::string startOperation(
    LogosInspectorAsyncBridge& bridge,
    std::uint64_t& correlation,
    std::string_view sourceMode)
{
    const Json request = {
        { "domain", "delivery" },
        { "method", "deliverySend" },
        { "adapter", {
            { "source_mode", sourceMode },
            { "inputs", Json::object() },
        } },
        { "mutating_enabled", true },
        { "payload", {
            { "topic", "/topic" },
            { "payload", "hello" },
        } },
    };
    const Json response = invokeInspector(
        bridge,
        correlation,
        "runtimeOperationStart",
        Json::array({ request }));
    REQUIRE(response.value("ok", false));
    const std::string operationId = response.at("value").value(
        "operationId",
        std::string {});
    REQUIRE(!operationId.empty());
    return operationId;
}

Json waitForOperationStatus(
    LogosInspectorAsyncBridge& bridge,
    std::uint64_t& correlation,
    const std::string& operationId,
    std::string_view wantedStatus)
{
    const auto deadline = Clock::now() + std::chrono::seconds(15);
    while (Clock::now() < deadline) {
        const Json response = invokeInspector(
            bridge,
            correlation,
            "runtimeOperationStatus",
            Json::array({ operationId }));
        REQUIRE(response.value("ok", false));
        const Json& operation = response.at("value");
        const std::string status = operation.value("status", std::string {});
        if (status == wantedStatus) {
            return operation;
        }
        if (isTerminalStatus(status)) {
            fail("operation reached expected status", __FILE__, __LINE__, operation.dump());
        }
        pumpEventsOnce();
    }
    fail("operation reached expected status", __FILE__, __LINE__, operationId);
}

void runComposedIntegration(QCoreApplication& application)
{
    QTemporaryDir logoscoreSentinel;
    REQUIRE(logoscoreSentinel.isValid());
    const QString logoscoreMarker = logoscoreSentinel.filePath(
        QStringLiteral("logoscore-invoked"));
    const QString logoscoreProgram = logoscoreSentinel.filePath(
        QStringLiteral("logoscore"));
    QFile sentinelProgram(logoscoreProgram);
    REQUIRE(sentinelProgram.open(QIODevice::WriteOnly | QIODevice::Truncate));
    REQUIRE(sentinelProgram.write(
                "#!/bin/sh\n"
                ": > \"$LOGOS_INSPECTOR_LOGOSCORE_MARKER\"\n"
                "exit 99\n")
        > 0);
    sentinelProgram.close();
    REQUIRE(QFile::setPermissions(
        logoscoreProgram,
        QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner));
    REQUIRE(qputenv("LOGOSCORE_BIN", logoscoreProgram.toUtf8()));
    REQUIRE(qputenv(
        "LOGOS_INSPECTOR_LOGOSCORE_MARKER",
        logoscoreMarker.toUtf8()));

    RegisteredProviders providers;
    providers.registerAll();
    IntegrationProvider& storage = providers.provider("storage_module");
    IntegrationProvider& delivery = providers.provider("delivery_module");

    LogosInspectorAsyncBridge bridge(
        std::make_unique<LogosProtocolHostTransport>());
    REQUIRE(bridge.ownsRuntimeModuleEvents());
    std::uint64_t correlation = 1;

    const auto admissionStarted = Clock::now();
    const std::string nullAdmission = bridge.callModuleAsync(
        "composed-null-admission",
        "storage_module",
        "returnsNull",
        "[]");
    REQUIRE(Clock::now() - admissionStarted < std::chrono::seconds(5));
    REQUIRE(storage.callCount() == 0);
    const Json nullResponse = waitForBridgeResponse(bridge, nullAdmission);
    REQUIRE(nullResponse.value("ok", false));
    REQUIRE(nullResponse.at("value").is_null());
    REQUIRE(storage.callThread() == application.thread());

    const Json errorResponse = invokeModule(
        bridge,
        correlation,
        "storage_module",
        "throws",
        Json::array());
    REQUIRE(!errorResponse.value("ok", true));
    const Json protocolError = parseJson(
        errorResponse.value("error", std::string {}),
        "nested protocol error JSON");
    REQUIRE(protocolError.value("code", std::string {}) == "invoke_failed");
    REQUIRE(protocolError.value("origin", std::string {}) == "storage_module");
    REQUIRE(storage.callCount() == 2);
    REQUIRE(storage.callThread() == application.thread());

    const std::string moduleOperation = startOperation(
        bridge,
        correlation,
        "module");
    const Json completed = waitForOperationStatus(
        bridge,
        correlation,
        moduleOperation,
        "completed");
    REQUIRE(completed.at("result") == Json::array({ "request-46", "hash-original" }));
    REQUIRE(delivery.callCount() == 1);
    REQUIRE(delivery.callThread() == application.thread());
    REQUIRE(delivery.lastArgs().size() == 2);

    const int deliveryCallsBeforeCliRequest = delivery.callCount();
    const std::string cliOperation = startOperation(
        bridge,
        correlation,
        "logoscore_cli");
    const Json failed = waitForOperationStatus(
        bridge,
        correlation,
        cliOperation,
        "failed");
    const std::string failure = failed.value("error", std::string {});
    REQUIRE(failure.find(
                "resolved module transport `logoscore_cli` is unavailable; active transport is `module`")
        != std::string::npos);
    REQUIRE(delivery.callCount() == deliveryCallsBeforeCliRequest);
    REQUIRE(!QFileInfo::exists(logoscoreMarker));

    std::mutex watchdogMutex;
    std::condition_variable watchdogChanged;
    bool closeCompleted = false;
    std::thread watchdog([&] {
        std::unique_lock<std::mutex> lock(watchdogMutex);
        if (!watchdogChanged.wait_for(lock, std::chrono::seconds(10), [&] {
                return closeCompleted;
            })) {
            std::cerr << "FAIL composed Logos protocol host integration: "
                         "foreign bridge close required owner event progress\n"
                      << std::flush;
            std::_Exit(2);
        }
    });

    std::atomic<bool> foreignCloseStarted { false };
    std::atomic<bool> foreignCloseReturned { false };
    std::thread foreignCloser([&] {
        foreignCloseStarted.store(true, std::memory_order_release);
        bridge.close();
        foreignCloseReturned.store(true, std::memory_order_release);
    });
    while (!foreignCloseStarted.load(std::memory_order_acquire)) {
        std::this_thread::yield();
    }
    const auto foreignCloseDeadline = Clock::now() + std::chrono::seconds(5);
    while (!foreignCloseReturned.load(std::memory_order_acquire)
        && Clock::now() < foreignCloseDeadline) {
        // Deliberately do not pump Qt events. Foreign logical close must not
        // depend on the activation thread processing deferred QObject retirement.
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    const bool foreignCloseReturnedWithoutOwnerEvents =
        foreignCloseReturned.load(std::memory_order_acquire);
    foreignCloser.join();
    {
        std::lock_guard<std::mutex> lock(watchdogMutex);
        closeCompleted = true;
    }
    watchdogChanged.notify_all();
    watchdog.join();
    REQUIRE(foreignCloseReturnedWithoutOwnerEvents);
    REQUIRE(!bridge.ownsRuntimeModuleEvents());

    const auto ownerCloseStarted = Clock::now();
    bridge.close();
    REQUIRE(Clock::now() - ownerCloseStarted < std::chrono::seconds(5));

    delivery.emitDeliveryReceipt("post-close", "ignored");
    pumpEventsFor(std::chrono::milliseconds(50));
}
} // namespace

int main(int argc, char** argv)
{
    try {
        QCoreApplication application(argc, argv);
        runComposedIntegration(application);
        std::cout << "PASS composed Logos protocol host integration\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "FAIL composed Logos protocol host integration: "
                  << error.what() << '\n';
        return 1;
    }
}
