#include "logos_protocol_host_transport.h"

#include <QCoreApplication>
#include <QMetaObject>
#include <QObject>

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <iostream>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <string>
#include <string_view>
#include <thread>
#include <utility>
#include <vector>

namespace {
using namespace std::chrono_literals;

class FakeProtocol;
class FakeIngress;

} // namespace

struct lp_client
{
    FakeProtocol* owner = nullptr;
    std::size_t id = 0;
    std::string module;
    std::thread::id ownerThread;
    bool destroyed = false;
};

struct lp_subscription
{
    FakeProtocol* owner = nullptr;
    std::size_t id = 0;
    lp_client* client = nullptr;
    std::string event;
    lp_event_cb callback = nullptr;
    void* userData = nullptr;
    bool active = true;
};

struct LogosInspectorCore
{
    FakeIngress* ingress = nullptr;
};

extern "C" lp_client* lp_client_create(
    const char*,
    const char*,
    const char*,
    const char*)
{
    return nullptr;
}

extern "C" void lp_client_destroy(lp_client*)
{
}

extern "C" int lp_invoke_async(
    lp_client*,
    const char*,
    const char*,
    int,
    lp_result_cb,
    void*)
{
    return LP_ERR_UNAVAILABLE;
}

extern "C" lp_subscription* lp_subscribe(
    lp_client*,
    const char*,
    lp_event_cb,
    void*)
{
    return nullptr;
}

extern "C" void lp_unsubscribe(lp_subscription*)
{
}

namespace {
constexpr std::array<std::string_view, 6> kExpectedModules = {
    "blockchain_module",
    "storage_module",
    "delivery_module",
    "capability_module",
    "lez_indexer_module",
    "lez_core",
};

constexpr std::array<std::pair<std::string_view, std::string_view>, 17> kExpectedEvents = { {
    { "delivery_module", "messageSent" },
    { "delivery_module", "messageError" },
    { "delivery_module", "messagePropagated" },
    { "delivery_module", "messageReceived" },
    { "delivery_module", "connectionStateChanged" },
    { "delivery_module", "nodeStarted" },
    { "delivery_module", "nodeStopped" },
    { "storage_module", "storageStart" },
    { "storage_module", "storageStop" },
    { "storage_module", "storageConnect" },
    { "storage_module", "storageUploadProgress" },
    { "storage_module", "storageUploadDone" },
    { "storage_module", "storageDownloadProgress" },
    { "storage_module", "storageDownloadDone" },
    { "storage_module", "storageDownloadManifestDone" },
    { "storage_module", "storageRemoveDone" },
    { "blockchain_module", "newBlock" },
} };

#define REQUIRE(condition)                                                                  \
    do {                                                                                    \
        if (!(condition)) {                                                                 \
            std::cerr << __func__ << ':' << __LINE__ << ": requirement failed: "          \
                      << #condition << '\n';                                                \
            return false;                                                                   \
        }                                                                                   \
    } while (false)

template<typename Predicate>
bool waitUntil(Predicate predicate, std::chrono::milliseconds timeout = 2s)
{
    const auto deadline = std::chrono::steady_clock::now() + timeout;
    while (!predicate()) {
        if (std::chrono::steady_clock::now() >= deadline) {
            return false;
        }
        std::this_thread::sleep_for(1ms);
    }
    return true;
}

class FakeProtocol
{
public:
    enum class InvokeMode {
        hold,
        inlineNull,
        inlineFailure,
        immediateUnavailable,
        throwBadAlloc,
    };

    struct CreatedClient
    {
        std::string module;
        std::string origin;
        std::thread::id thread;
    };

    struct CreatedSubscription
    {
        std::string module;
        std::string event;
        std::thread::id thread;
    };

    struct Invocation
    {
        std::size_t clientId = 0;
        std::string module;
        std::string method;
        std::string argsJson;
        int timeoutMs = 0;
        lp_result_cb callback = nullptr;
        void* userData = nullptr;
        bool active = true;
    };

    static LogosProtocolApi api()
    {
        LogosProtocolApi result;
        result.clientCreate = &createClient;
        result.clientDestroy = &destroyClient;
        result.invokeAsync = &invokeAsync;
        result.subscribe = &subscribe;
        result.unsubscribe = &unsubscribe;
        return result;
    }

    void installForActivation()
    {
        std::lock_guard<std::mutex> lock(factoryMutex());
        activeFactory() = this;
    }

    void setInvokeMode(InvokeMode mode)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        invokeMode_ = mode;
    }

    void failSubscriptionAt(std::size_t ordinal)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        failSubscriptionOrdinal_ = ordinal;
    }

    void emitDuringNextUnsubscribe()
    {
        std::lock_guard<std::mutex> lock(mutex_);
        emitDuringUnsubscribe_ = true;
    }

    void failClientAt(std::size_t ordinal)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        failClientOrdinal_ = ordinal;
    }

    void blockNextClientCreate()
    {
        std::lock_guard<std::mutex> lock(mutex_);
        blockClientCreate_ = true;
        releaseClientCreate_ = false;
        clientCreateEntered_ = false;
    }

    bool waitForBlockedClientCreate()
    {
        std::unique_lock<std::mutex> lock(mutex_);
        return changed_.wait_for(lock, 2s, [this] { return clientCreateEntered_; });
    }

    void releaseBlockedClientCreate()
    {
        std::lock_guard<std::mutex> lock(mutex_);
        releaseClientCreate_ = true;
        changed_.notify_all();
    }

    std::vector<CreatedClient> createdClients() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return createdClients_;
    }

    std::vector<CreatedSubscription> createdSubscriptions() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return createdSubscriptions_;
    }

    std::vector<Invocation> invocations() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return invocations_;
    }

    std::vector<std::string> teardownLog() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return teardownLog_;
    }

    std::size_t destroyedClients() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return destroyedClients_;
    }

    std::size_t lifecycleThreadViolations() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return lifecycleThreadViolations_;
    }

    void emulateOwnerMarshalForTeardown(bool enabled)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        emulateOwnerMarshalForTeardown_ = enabled;
    }

    void blockOnOwnerMarshalForTeardown(bool enabled)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        blockOnOwnerMarshalForTeardown_ = enabled;
        ownerMarshalEntered_ = false;
    }

    bool waitForOwnerMarshalEntry()
    {
        std::unique_lock<std::mutex> lock(mutex_);
        return changed_.wait_for(lock, 2s, [this] { return ownerMarshalEntered_; });
    }

    std::size_t marshalledLifecycleCalls() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return marshalledLifecycleCalls_;
    }

    bool completeInvocation(
        std::size_t ordinal,
        int ok,
        const std::string& payload,
        bool foreignThread)
    {
        lp_result_cb callback = nullptr;
        void* userData = nullptr;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (ordinal >= invocations_.size() || !invocations_[ordinal].active) {
                return false;
            }
            Invocation& invocation = invocations_[ordinal];
            invocation.active = false;
            callback = invocation.callback;
            userData = invocation.userData;
        }
        if (foreignThread) {
            std::thread thread([callback, userData, ok, payload] {
                callback(ok, payload.c_str(), userData);
            });
            thread.join();
        } else {
            callback(ok, payload.c_str(), userData);
        }
        return true;
    }

    bool emitEvent(
        std::string_view module,
        std::string_view event,
        const std::string& payload,
        bool foreignThread = false)
    {
        lp_event_cb callback = nullptr;
        void* userData = nullptr;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            for (const auto& subscription : subscriptions_) {
                if (subscription->active && !subscription->client->destroyed
                    && subscription->client->module == module
                    && subscription->event == event) {
                    callback = subscription->callback;
                    userData = subscription->userData;
                    break;
                }
            }
        }
        if (callback == nullptr) {
            return false;
        }
        if (foreignThread) {
            std::thread thread([callback, userData, event, payload] {
                const std::string eventCopy(event);
                callback(eventCopy.c_str(), payload.c_str(), userData);
            });
            thread.join();
        } else {
            const std::string eventCopy(event);
            callback(eventCopy.c_str(), payload.c_str(), userData);
        }
        return true;
    }

private:
    static FakeProtocol*& activeFactory()
    {
        static FakeProtocol* factory = nullptr;
        return factory;
    }

    static std::mutex& factoryMutex()
    {
        static std::mutex mutex;
        return mutex;
    }

    static lp_client* createClient(
        const char* target,
        const char* origin,
        const char*,
        const char*)
    {
        FakeProtocol* factory = nullptr;
        {
            std::lock_guard<std::mutex> lock(factoryMutex());
            factory = activeFactory();
        }
        if (factory == nullptr || target == nullptr || origin == nullptr) {
            return nullptr;
        }
        return factory->createClientImpl(target, origin);
    }

    lp_client* createClientImpl(const char* target, const char* origin)
    {
        std::unique_lock<std::mutex> lock(mutex_);
        if (blockClientCreate_) {
            blockClientCreate_ = false;
            clientCreateEntered_ = true;
            changed_.notify_all();
            changed_.wait(lock, [this] { return releaseClientCreate_; });
        }
        if (createdClients_.size() == failClientOrdinal_) {
            return nullptr;
        }
        auto client = std::make_unique<lp_client>();
        client->owner = this;
        client->id = clients_.size() + 1;
        client->module = target;
        client->ownerThread = std::this_thread::get_id();
        lp_client* const raw = client.get();
        clients_.push_back(std::move(client));
        createdClients_.push_back(
            CreatedClient { target, origin, std::this_thread::get_id() });
        return raw;
    }

    static void destroyClient(lp_client* client)
    {
        if (client == nullptr || client->owner == nullptr) {
            return;
        }
        client->owner->destroyClientImpl(client);
    }

    void destroyClientImpl(lp_client* client)
    {
        if (marshalLifecycleCallToOwner(
                client->ownerThread,
                [this, client] { destroyClientImpl(client); })) {
            return;
        }
        std::lock_guard<std::mutex> lock(mutex_);
        if (client->destroyed) {
            return;
        }
        client->destroyed = true;
        for (Invocation& invocation : invocations_) {
            if (invocation.clientId == client->id) {
                invocation.active = false;
            }
        }
        teardownLog_.push_back("destroy:" + client->module);
        ++destroyedClients_;
    }

    static int invokeAsync(
        lp_client* client,
        const char* method,
        const char* argsJson,
        int timeoutMs,
        lp_result_cb callback,
        void* userData)
    {
        if (client == nullptr || client->owner == nullptr) {
            return LP_ERR_INVALID_ARG;
        }
        return client->owner->invokeAsyncImpl(
            client,
            method,
            argsJson,
            timeoutMs,
            callback,
            userData);
    }

    int invokeAsyncImpl(
        lp_client* client,
        const char* method,
        const char* argsJson,
        int timeoutMs,
        lp_result_cb callback,
        void* userData)
    {
        InvokeMode mode;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (client->destroyed || method == nullptr || argsJson == nullptr
                || callback == nullptr) {
                return LP_ERR_INVALID_ARG;
            }
            mode = invokeMode_;
            if (mode != InvokeMode::throwBadAlloc) {
                invocations_.push_back(Invocation {
                    client->id,
                    client->module,
                    method,
                    argsJson,
                    timeoutMs,
                    callback,
                    userData,
                    mode == InvokeMode::hold,
                });
            }
        }
        if (mode == InvokeMode::throwBadAlloc) {
            throw std::bad_alloc();
        }
        if (mode == InvokeMode::inlineNull) {
            callback(1, "null", userData);
            return LP_OK;
        }
        if (mode == InvokeMode::inlineFailure) {
            callback(
                0,
                R"({"code":"timeout","message":"timed out","origin":"delivery_module"})",
                userData);
            return LP_OK;
        }
        if (mode == InvokeMode::immediateUnavailable) {
            return LP_ERR_UNAVAILABLE;
        }
        return LP_OK;
    }

    static lp_subscription* subscribe(
        lp_client* client,
        const char* event,
        lp_event_cb callback,
        void* userData)
    {
        if (client == nullptr || client->owner == nullptr) {
            return nullptr;
        }
        return client->owner->subscribeImpl(client, event, callback, userData);
    }

    lp_subscription* subscribeImpl(
        lp_client* client,
        const char* event,
        lp_event_cb callback,
        void* userData)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        const std::size_t ordinal = createdSubscriptions_.size();
        if (client->destroyed || event == nullptr || callback == nullptr
            || ordinal == failSubscriptionOrdinal_) {
            return nullptr;
        }
        if (client->ownerThread != std::this_thread::get_id()) {
            ++lifecycleThreadViolations_;
        }
        auto subscription = std::make_unique<lp_subscription>();
        subscription->owner = this;
        subscription->id = subscriptions_.size() + 1;
        subscription->client = client;
        subscription->event = event;
        subscription->callback = callback;
        subscription->userData = userData;
        lp_subscription* const raw = subscription.get();
        subscriptions_.push_back(std::move(subscription));
        createdSubscriptions_.push_back(
            CreatedSubscription { client->module, event, std::this_thread::get_id() });
        return raw;
    }

    static void unsubscribe(lp_subscription* subscription)
    {
        if (subscription == nullptr || subscription->owner == nullptr) {
            return;
        }
        subscription->owner->unsubscribeImpl(subscription);
    }

    void unsubscribeImpl(lp_subscription* subscription)
    {
        if (marshalLifecycleCallToOwner(
                subscription->client->ownerThread,
                [this, subscription] { unsubscribeImpl(subscription); })) {
            return;
        }
        std::lock_guard<std::mutex> lock(mutex_);
        if (!subscription->active) {
            return;
        }
        if (emitDuringUnsubscribe_) {
            emitDuringUnsubscribe_ = false;
            const lp_event_cb callback = subscription->callback;
            void* const userData = subscription->userData;
            const std::string event = subscription->event;
            std::thread callbackThread([callback, userData, event] {
                callback(event.c_str(), R"(["during-unsubscribe"])", userData);
            });
            callbackThread.join();
        }
        subscription->active = false;
        teardownLog_.push_back(
            "unsubscribe:" + subscription->client->module + ':' + subscription->event);
    }

    bool marshalLifecycleCallToOwner(
        std::thread::id ownerThread,
        std::function<void()> operation)
    {
        if (ownerThread == std::this_thread::get_id()) {
            return false;
        }

        bool blockOnOwner = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (blockOnOwnerMarshalForTeardown_) {
                ++marshalledLifecycleCalls_;
                ownerMarshalEntered_ = true;
                blockOnOwner = true;
                changed_.notify_all();
            } else if (emulateOwnerMarshalForTeardown_) {
                ++marshalledLifecycleCalls_;
            } else {
                ++lifecycleThreadViolations_;
            }
        }
        if (!blockOnOwner) {
            return false;
        }

        const bool invoked = QMetaObject::invokeMethod(
            &ownerDispatcher_,
            [operation = std::move(operation)]() mutable { operation(); },
            Qt::BlockingQueuedConnection);
        if (!invoked) {
            std::lock_guard<std::mutex> lock(mutex_);
            ++lifecycleThreadViolations_;
        }
        return true;
    }

    mutable std::mutex mutex_;
    std::condition_variable changed_;
    InvokeMode invokeMode_ = InvokeMode::hold;
    std::size_t failClientOrdinal_ = (std::numeric_limits<std::size_t>::max)();
    std::size_t failSubscriptionOrdinal_ = (std::numeric_limits<std::size_t>::max)();
    std::size_t destroyedClients_ = 0;
    std::size_t lifecycleThreadViolations_ = 0;
    std::size_t marshalledLifecycleCalls_ = 0;
    bool emulateOwnerMarshalForTeardown_ = false;
    bool blockOnOwnerMarshalForTeardown_ = false;
    bool ownerMarshalEntered_ = false;
    bool blockClientCreate_ = false;
    bool clientCreateEntered_ = false;
    bool releaseClientCreate_ = false;
    bool emitDuringUnsubscribe_ = false;
    std::vector<std::unique_ptr<lp_client>> clients_;
    std::vector<std::unique_ptr<lp_subscription>> subscriptions_;
    std::vector<CreatedClient> createdClients_;
    std::vector<CreatedSubscription> createdSubscriptions_;
    std::vector<Invocation> invocations_;
    std::vector<std::string> teardownLog_;
    QObject ownerDispatcher_;
};

class FakeIngress
{
public:
    enum class Mode { accept, backpressure, reject };

    struct Event
    {
        std::string module;
        std::string event;
        std::string argsJson;
    };

    void setMode(Mode mode)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        mode_ = mode;
    }

    int32_t ingest(const char* module, const char* event, const char* argsJson)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        calls_.push_back(Event { module, event, argsJson });
        switch (mode_) {
        case Mode::accept:
            return LOGOS_INSPECTOR_EVENT_ACCEPTED;
        case Mode::backpressure:
            return LOGOS_INSPECTOR_EVENT_BACKPRESSURE;
        case Mode::reject:
            return LOGOS_INSPECTOR_EVENT_REJECTED;
        }
        return LOGOS_INSPECTOR_EVENT_REJECTED;
    }

    std::vector<Event> calls() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return calls_;
    }

private:
    mutable std::mutex mutex_;
    Mode mode_ = Mode::accept;
    std::vector<Event> calls_;
};

int32_t ingestModuleEvent(
    LogosInspectorCore* core,
    const char* module,
    const char* event,
    const char* argsJson)
{
    if (core == nullptr || core->ingress == nullptr || module == nullptr || event == nullptr
        || argsJson == nullptr) {
        return LOGOS_INSPECTOR_EVENT_REJECTED;
    }
    return core->ingress->ingest(module, event, argsJson);
}

struct CapturedReply
{
    uint64_t requestId = 0;
    int32_t ok = 0;
    std::string payload;
};

class ReplyCollector
{
public:
    static void callback(
        void* context,
        uint64_t requestId,
        int32_t ok,
        const char* payload) noexcept
    {
        auto* collector = static_cast<ReplyCollector*>(context);
        try {
            std::lock_guard<std::mutex> lock(collector->mutex_);
            collector->replies_.push_back(
                CapturedReply { requestId, ok, payload == nullptr ? "" : payload });
        } catch (...) {
        }
    }

    std::vector<CapturedReply> replies() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return replies_;
    }

private:
    mutable std::mutex mutex_;
    std::vector<CapturedReply> replies_;
};

class BlockingReplyCollector
{
public:
    static void callback(
        void* context,
        uint64_t requestId,
        int32_t ok,
        const char* payload) noexcept
    {
        auto* collector = static_cast<BlockingReplyCollector*>(context);
        try {
            std::unique_lock<std::mutex> lock(collector->mutex_);
            collector->entered_ = true;
            collector->reply_ = CapturedReply {
                requestId,
                ok,
                payload == nullptr ? "" : payload,
            };
            collector->changed_.notify_all();
            collector->changed_.wait(lock, [collector] { return collector->released_; });
        } catch (...) {
        }
    }

    bool waitUntilEntered()
    {
        std::unique_lock<std::mutex> lock(mutex_);
        return changed_.wait_for(lock, 2s, [this] { return entered_; });
    }

    void release()
    {
        std::lock_guard<std::mutex> lock(mutex_);
        released_ = true;
        changed_.notify_all();
    }

    CapturedReply reply() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return reply_;
    }

private:
    mutable std::mutex mutex_;
    std::condition_variable changed_;
    CapturedReply reply_;
    bool entered_ = false;
    bool released_ = false;
};

struct Fixture
{
    explicit Fixture(LogosProtocolHostTransportLimits limits = {})
        : transport(FakeProtocol::api(), limits)
    {
        protocol.installForActivation();
        core.ingress = &ingress;
    }

    bool activate()
    {
        return transport.bindCore(&core, &ingestModuleEvent) && transport.activate();
    }

    FakeProtocol protocol;
    FakeIngress ingress;
    LogosInspectorCore core;
    LogosProtocolHostTransport transport;
};

bool activationCreatesExactCatalogOnOwnerThread()
{
    Fixture fixture;
    const std::thread::id ownerThread = std::this_thread::get_id();
    REQUIRE(fixture.activate());
    REQUIRE(fixture.transport.ownsRuntimeModuleEvents());

    const auto clients = fixture.protocol.createdClients();
    REQUIRE(clients.size() == kExpectedModules.size());
    for (std::size_t index = 0; index < clients.size(); ++index) {
        REQUIRE(clients[index].module == kExpectedModules[index]);
        REQUIRE(clients[index].origin == "logos_inspector");
        REQUIRE(clients[index].thread == ownerThread);
    }

    const auto subscriptions = fixture.protocol.createdSubscriptions();
    REQUIRE(subscriptions.size() == kExpectedEvents.size());
    for (std::size_t index = 0; index < subscriptions.size(); ++index) {
        REQUIRE(subscriptions[index].module == kExpectedEvents[index].first);
        REQUIRE(subscriptions[index].event == kExpectedEvents[index].second);
        REQUIRE(subscriptions[index].thread == ownerThread);
    }

    fixture.transport.close();
    REQUIRE(!fixture.transport.ownsRuntimeModuleEvents());
    REQUIRE(fixture.protocol.lifecycleThreadViolations() == 0);
    return true;
}

bool activationRollbackFailsClosed()
{
    Fixture fixture;
    fixture.protocol.failClientAt(2);
    REQUIRE(fixture.transport.bindCore(&fixture.core, &ingestModuleEvent));
    REQUIRE(!fixture.transport.activate());
    REQUIRE(!fixture.transport.ownsRuntimeModuleEvents());
    REQUIRE(fixture.protocol.destroyedClients() == 2);
    REQUIRE(fixture.protocol.lifecycleThreadViolations() == 0);
    REQUIRE(fixture.protocol.createdSubscriptions().empty());
    return true;
}

bool missingOptionalSubscriptionKeepsDispatchOpen()
{
    Fixture fixture;
    fixture.protocol.emitDuringNextUnsubscribe();
    fixture.protocol.failSubscriptionAt(5);
    REQUIRE(fixture.activate());
    REQUIRE(!fixture.transport.ownsRuntimeModuleEvents());
    REQUIRE(fixture.protocol.destroyedClients() == 0);
    REQUIRE(!waitUntil([&fixture] { return !fixture.ingress.calls().empty(); }, 50ms));

    const auto activationLog = fixture.protocol.teardownLog();
    REQUIRE(std::count_if(
                activationLog.begin(),
                activationLog.end(),
                [](const std::string& item) {
                    return item.rfind("unsubscribe:", 0) == 0;
                })
        == 5);
    REQUIRE(!fixture.protocol.emitEvent(
        "delivery_module",
        "messageSent",
        "[]"));

    ReplyCollector replies;
    const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    REQUIRE(vtable.dispatch(
                vtable.context,
                7,
                "delivery_module",
                "sendMessage",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    REQUIRE(fixture.protocol.invocations().size() == 1);
    REQUIRE(fixture.protocol.completeInvocation(0, 1, "null", true));
    REQUIRE(waitUntil([&replies] { return replies.replies().size() == 1; }));
    REQUIRE(replies.replies()[0].ok == 1);
    REQUIRE(replies.replies()[0].payload == "null");

    fixture.transport.close();
    REQUIRE(fixture.protocol.destroyedClients() == kExpectedModules.size());
    REQUIRE(fixture.protocol.lifecycleThreadViolations() == 0);
    return true;
}

bool closeWaitsForAdmittedActivationStartup()
{
    Fixture fixture;
    fixture.protocol.blockNextClientCreate();
    REQUIRE(fixture.transport.bindCore(&fixture.core, &ingestModuleEvent));

    std::atomic<bool> activationResult { true };
    std::atomic<bool> closeReturned { false };
    std::thread activation([&fixture, &activationResult] {
        activationResult.store(fixture.transport.activate(), std::memory_order_release);
    });
    const bool createBlocked = fixture.protocol.waitForBlockedClientCreate();
    if (!createBlocked) {
        fixture.protocol.releaseBlockedClientCreate();
        fixture.transport.close();
        activation.join();
        REQUIRE(createBlocked);
    }

    std::thread closer([&fixture, &closeReturned] {
        fixture.transport.close();
        closeReturned.store(true, std::memory_order_release);
    });
    std::this_thread::sleep_for(20ms);
    const bool closeWasBlocked = !closeReturned.load(std::memory_order_acquire);

    fixture.protocol.releaseBlockedClientCreate();
    activation.join();
    closer.join();
    REQUIRE(closeWasBlocked);
    REQUIRE(closeReturned.load(std::memory_order_acquire));
    REQUIRE(!activationResult.load(std::memory_order_acquire));
    REQUIRE(!fixture.transport.ownsRuntimeModuleEvents());
    REQUIRE(fixture.protocol.destroyedClients() == 1);
    REQUIRE(fixture.protocol.lifecycleThreadViolations() == 0);
    const std::size_t createdAfterClose = fixture.protocol.createdClients().size();
    std::this_thread::sleep_for(10ms);
    REQUIRE(fixture.protocol.createdClients().size() == createdAfterClose);
    return true;
}

bool dispatchEnforcesAllowlistAndBounds()
{
    Fixture fixture;
    fixture.protocol.setInvokeMode(FakeProtocol::InvokeMode::immediateUnavailable);
    REQUIRE(fixture.activate());
    const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    ReplyCollector replies;

    REQUIRE(vtable.dispatch(
                vtable.context,
                1,
                "editable_module",
                "call",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 0);
    REQUIRE(vtable.dispatch(
                vtable.context,
                2,
                "delivery_module",
                "",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 0);
    REQUIRE(vtable.dispatch(
                vtable.context,
                3,
                "delivery_module",
                "sendMessage",
                nullptr,
                &ReplyCollector::callback,
                &replies)
        == 0);
    REQUIRE(vtable.dispatch(
                vtable.context,
                4,
                "delivery_module",
                "sendMessage",
                "[\"copied\"]",
                &ReplyCollector::callback,
                &replies)
        == 1);

    const auto invocations = fixture.protocol.invocations();
    REQUIRE(invocations.size() == 1);
    REQUIRE(invocations[0].module == "delivery_module");
    REQUIRE(invocations[0].method == "sendMessage");
    REQUIRE(invocations[0].argsJson == "[\"copied\"]");
    REQUIRE(invocations[0].timeoutMs == 20'000);

    const auto captured = replies.replies();
    REQUIRE(captured.size() == 1);
    REQUIRE(captured[0].requestId == 4);
    REQUIRE(captured[0].ok == 0);
    REQUIRE(captured[0].payload
        == R"({"code":"object_unavailable","message":"target module/object could not be acquired","origin":"delivery_module"})");
    return true;
}

bool pendingAdmissionIsBoundedAndIdsStayReserved()
{
    LogosProtocolHostTransportLimits limits;
    limits.maxPendingRequests = 1;
    Fixture fixture(limits);
    LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    ReplyCollector replies;
    REQUIRE(vtable.dispatch(
                vtable.context,
                61,
                "delivery_module",
                "sendMessage",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 0);
    REQUIRE(fixture.activate());
    vtable = fixture.transport.vtable();
    REQUIRE(vtable.dispatch(
                vtable.context,
                61,
                "delivery_module",
                "sendMessage",
                "[1]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    REQUIRE(vtable.dispatch(
                vtable.context,
                61,
                "delivery_module",
                "sendMessage",
                "[\"duplicate\"]",
                &ReplyCollector::callback,
                &replies)
        == 0);
    REQUIRE(vtable.dispatch(
                vtable.context,
                62,
                "delivery_module",
                "sendMessage",
                "[2]",
                &ReplyCollector::callback,
                &replies)
        == 0);
    REQUIRE(fixture.protocol.completeInvocation(0, 1, "{\"done\":1}", true));
    REQUIRE(vtable.dispatch(
                vtable.context,
                62,
                "delivery_module",
                "sendMessage",
                "[2]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    REQUIRE(fixture.protocol.completeInvocation(1, 1, "{\"done\":2}", true));
    REQUIRE(replies.replies().size() == 2);
    return true;
}

bool foreignResultsPreserveSuccessNullAndCanonicalFailure()
{
    Fixture fixture;
    REQUIRE(fixture.activate());
    const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    ReplyCollector replies;

    REQUIRE(vtable.dispatch(
                vtable.context,
                11,
                "delivery_module",
                "sendMessage",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    REQUIRE(fixture.protocol.completeInvocation(0, 1, "null", true));

    REQUIRE(vtable.dispatch(
                vtable.context,
                12,
                "storage_module",
                "download",
                "[\"cid\"]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    const std::string error =
        R"({"code":"timeout","message":"timed out","origin":"storage_module"})";
    REQUIRE(fixture.protocol.completeInvocation(1, 0, error, true));

    REQUIRE(waitUntil([&replies] { return replies.replies().size() == 2; }));
    const auto captured = replies.replies();
    REQUIRE(captured[0].requestId == 11);
    REQUIRE(captured[0].ok == 1);
    REQUIRE(captured[0].payload == "null");
    REQUIRE(captured[1].requestId == 12);
    REQUIRE(captured[1].ok == 0);
    REQUIRE(captured[1].payload == error);

    fixture.protocol.setInvokeMode(FakeProtocol::InvokeMode::inlineNull);
    REQUIRE(vtable.dispatch(
                vtable.context,
                13,
                "delivery_module",
                "sendMessage",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    fixture.protocol.setInvokeMode(FakeProtocol::InvokeMode::inlineFailure);
    REQUIRE(vtable.dispatch(
                vtable.context,
                14,
                "delivery_module",
                "sendMessage",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    const auto withInline = replies.replies();
    REQUIRE(withInline.size() == 4);
    REQUIRE(withInline[2].requestId == 13);
    REQUIRE(withInline[2].ok == 1);
    REQUIRE(withInline[2].payload == "null");
    REQUIRE(withInline[3].requestId == 14);
    REQUIRE(withInline[3].ok == 0);
    REQUIRE(withInline[3].payload
        == R"({"code":"timeout","message":"timed out","origin":"delivery_module"})");
    return true;
}

bool malformedResultStillCompletesAcceptedDispatch()
{
    LogosProtocolHostTransportLimits limits;
    limits.maxSingleResultBytes = 4;
    Fixture fixture(limits);
    REQUIRE(fixture.activate());
    const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    ReplyCollector replies;

    REQUIRE(vtable.dispatch(
                vtable.context,
                15,
                "delivery_module",
                "sendMessage",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    REQUIRE(fixture.protocol.completeInvocation(0, 1, "12345", true));
    REQUIRE(waitUntil([&replies] { return replies.replies().size() == 1; }));
    REQUIRE(replies.replies()[0].requestId == 15);
    REQUIRE(replies.replies()[0].ok == 0);
    REQUIRE(replies.replies()[0].payload
        == R"({"code":"invoke_failed","message":"logos-protocol returned an invalid result payload","origin":"delivery_module"})");
    return true;
}

bool protocolAllocationExceptionDoesNotPinClose()
{
    Fixture fixture;
    REQUIRE(fixture.activate());
    fixture.protocol.setInvokeMode(FakeProtocol::InvokeMode::throwBadAlloc);
    const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    ReplyCollector replies;

    REQUIRE(vtable.dispatch(
                vtable.context,
                16,
                "delivery_module",
                "sendMessage",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    REQUIRE(replies.replies().size() == 1);
    REQUIRE(replies.replies()[0].requestId == 16);
    REQUIRE(replies.replies()[0].ok == 0);
    REQUIRE(replies.replies()[0].payload
        == R"({"code":"invoke_failed","message":"logos-protocol could not dispatch module invocation","origin":"delivery_module"})");

    fixture.transport.close();
    REQUIRE(fixture.protocol.destroyedClients() == kExpectedModules.size());
    return true;
}

bool closeWaitsForImmediateErrorReplyCallback()
{
    Fixture fixture;
    REQUIRE(fixture.activate());
    fixture.protocol.setInvokeMode(FakeProtocol::InvokeMode::immediateUnavailable);
    const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    BlockingReplyCollector reply;
    std::atomic<bool> dispatchReturned { false };
    std::atomic<bool> closeReturned { false };

    std::thread dispatcher([&] {
        const int32_t accepted = vtable.dispatch(
            vtable.context,
            17,
            "delivery_module",
            "sendMessage",
            "[]",
            &BlockingReplyCollector::callback,
            &reply);
        dispatchReturned.store(accepted == 1, std::memory_order_release);
    });

    const bool replyEntered = reply.waitUntilEntered();
    if (!replyEntered) {
        reply.release();
        dispatcher.join();
        fixture.transport.close();
        REQUIRE(replyEntered);
    }

    std::thread closer([&] {
        fixture.transport.close();
        closeReturned.store(true, std::memory_order_release);
    });
    const bool closeBegan = waitUntil([&fixture] {
        return !fixture.transport.ownsRuntimeModuleEvents();
    });
    const bool closeWaitedForReply = !closeReturned.load(std::memory_order_acquire);

    reply.release();
    dispatcher.join();
    closer.join();

    REQUIRE(closeBegan);
    REQUIRE(closeWaitedForReply);
    REQUIRE(dispatchReturned.load(std::memory_order_acquire));
    REQUIRE(closeReturned.load(std::memory_order_acquire));
    REQUIRE(reply.reply().requestId == 17);
    REQUIRE(fixture.protocol.destroyedClients() == kExpectedModules.size());
    return true;
}

bool cancellationSuppressesReplyAndRetainsCallbackContext()
{
    Fixture fixture;
    REQUIRE(fixture.activate());
    const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    ReplyCollector replies;

    REQUIRE(vtable.dispatch(
                vtable.context,
                21,
                "lez_core",
                "operation",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    vtable.cancel(vtable.context, 21);
    REQUIRE(fixture.protocol.completeInvocation(0, 1, "{\"late\":true}", true));
    REQUIRE(replies.replies().empty());

    fixture.protocol.setInvokeMode(FakeProtocol::InvokeMode::immediateUnavailable);
    REQUIRE(vtable.dispatch(
                vtable.context,
                21,
                "lez_core",
                "operation",
                "[]",
                &ReplyCollector::callback,
                &replies)
        == 1);
    REQUIRE(replies.replies().size() == 1);
    return true;
}

bool foreignCloseQuiescesThroughOwnerMarshal()
{
    Fixture fixture;
    REQUIRE(fixture.activate());
    fixture.protocol.emulateOwnerMarshalForTeardown(true);
    const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
    ReplyCollector replies;
    REQUIRE(vtable.dispatch(
                vtable.context,
                31,
                "blockchain_module",
                "getBlock",
                "[1]",
                &ReplyCollector::callback,
                &replies)
        == 1);

    std::thread closer([&vtable] { vtable.close(vtable.context); });
    closer.join();

    REQUIRE(!fixture.transport.ownsRuntimeModuleEvents());
    REQUIRE(!fixture.protocol.completeInvocation(0, 1, "null", true));
    REQUIRE(replies.replies().empty());
    const auto log = fixture.protocol.teardownLog();
    REQUIRE(log.size() == kExpectedEvents.size() + kExpectedModules.size());
    const auto firstDestroy = std::find_if(log.begin(), log.end(), [](const std::string& item) {
        return item.rfind("destroy:", 0) == 0;
    });
    REQUIRE(firstDestroy == log.begin() + static_cast<std::ptrdiff_t>(kExpectedEvents.size()));
    REQUIRE(fixture.protocol.lifecycleThreadViolations() == 0);
    REQUIRE(fixture.protocol.marshalledLifecycleCalls()
        == kExpectedEvents.size() + kExpectedModules.size());
    return true;
}

bool ownerClosePumpsForeignTeardownAndDestructorStaysIdempotent()
{
    FakeProtocol protocol;
    FakeIngress ingress;
    LogosInspectorCore core { &ingress };
    protocol.installForActivation();
    auto transport = std::make_unique<LogosProtocolHostTransport>(
        FakeProtocol::api(),
        LogosProtocolHostTransportLimits {});
    REQUIRE(transport->bindCore(&core, &ingestModuleEvent));
    REQUIRE(transport->activate());
    protocol.blockOnOwnerMarshalForTeardown(true);

    LogosProtocolHostTransport* const rawTransport = transport.get();
    std::atomic<bool> foreignCloseReturned { false };
    std::thread foreignCloser([rawTransport, &foreignCloseReturned] {
        rawTransport->close();
        foreignCloseReturned.store(true, std::memory_order_release);
    });

    const bool foreignTeardownEntered = protocol.waitForOwnerMarshalEntry();
    if (!foreignTeardownEntered) {
        rawTransport->close();
        foreignCloser.join();
        REQUIRE(foreignTeardownEntered);
    }

    rawTransport->close();
    foreignCloser.join();
    REQUIRE(foreignCloseReturned.load(std::memory_order_acquire));
    REQUIRE(!rawTransport->ownsRuntimeModuleEvents());
    REQUIRE(protocol.lifecycleThreadViolations() == 0);
    REQUIRE(protocol.marshalledLifecycleCalls()
        == kExpectedEvents.size() + kExpectedModules.size());

    const std::size_t teardownCalls = protocol.teardownLog().size();
    REQUIRE(teardownCalls == kExpectedEvents.size() + kExpectedModules.size());
    rawTransport->close();
    transport.reset();
    REQUIRE(protocol.teardownLog().size() == teardownCalls);
    return true;
}

bool backpressureRetriesInFifoOrderWithoutBlockingCallback()
{
    LogosProtocolHostTransportLimits limits;
    limits.retryDelay = 20ms;
    Fixture fixture(limits);
    fixture.ingress.setMode(FakeIngress::Mode::backpressure);
    REQUIRE(fixture.activate());

    const auto start = std::chrono::steady_clock::now();
    REQUIRE(fixture.protocol.emitEvent(
        "delivery_module",
        "messageSent",
        "[\"first\"]",
        true));
    REQUIRE(std::chrono::steady_clock::now() - start < 100ms);
    REQUIRE(fixture.protocol.emitEvent(
        "delivery_module",
        "messageSent",
        "[\"second\"]"));

    fixture.ingress.setMode(FakeIngress::Mode::accept);
    REQUIRE(waitUntil([&fixture] {
        const auto calls = fixture.ingress.calls();
        std::size_t acceptedSequence = 0;
        for (const auto& call : calls) {
            if (call.argsJson == "[\"first\"]") {
                acceptedSequence = std::max<std::size_t>(acceptedSequence, 1);
            } else if (call.argsJson == "[\"second\"]" && acceptedSequence == 1) {
                acceptedSequence = 2;
            }
        }
        return acceptedSequence == 2;
    }));
    REQUIRE(fixture.transport.ownsRuntimeModuleEvents());
    return true;
}

bool queueOverflowAndRejectedIngressFaultTransport()
{
    LogosProtocolHostTransportLimits limits;
    limits.maxQueuedEvents = 1;
    limits.retryDelay = 20ms;
    {
        Fixture fixture(limits);
        fixture.ingress.setMode(FakeIngress::Mode::backpressure);
        REQUIRE(fixture.activate());
        REQUIRE(fixture.protocol.emitEvent(
            "storage_module",
            "storageDownloadProgress",
            "[1]"));
        REQUIRE(fixture.protocol.emitEvent(
            "storage_module",
            "storageDownloadProgress",
            "[2]"));
        REQUIRE(waitUntil([&fixture] {
            return !fixture.transport.ownsRuntimeModuleEvents();
        }));
        REQUIRE(fixture.protocol.destroyedClients() == 0);
        fixture.transport.close();
        REQUIRE(fixture.protocol.destroyedClients() == kExpectedModules.size());
        REQUIRE(fixture.protocol.lifecycleThreadViolations() == 0);
        REQUIRE(!fixture.transport.ownsRuntimeModuleEvents());
    }

    {
        Fixture fixture(limits);
        fixture.ingress.setMode(FakeIngress::Mode::backpressure);
        REQUIRE(fixture.activate());
        ReplyCollector replies;
        const LogosInspectorHostTransportV1 vtable = fixture.transport.vtable();
        REQUIRE(vtable.dispatch(
                    vtable.context,
                    41,
                    "delivery_module",
                    "sendMessage",
                    "[]",
                    &ReplyCollector::callback,
                    &replies)
            == 1);
        REQUIRE(fixture.protocol.emitEvent(
            "delivery_module",
            "messageError",
            "[\"fault\"]"));
        fixture.ingress.setMode(FakeIngress::Mode::reject);
        REQUIRE(waitUntil([&replies] { return replies.replies().size() == 1; }));
        const auto captured = replies.replies();
        REQUIRE(captured[0].requestId == 41);
        REQUIRE(captured[0].ok == 0);
        REQUIRE(captured[0].payload
            == R"({"code":"transport_closed","message":"native module event ingress failed; host transport closed","origin":"logos_inspector"})");
        REQUIRE(fixture.protocol.destroyedClients() == 0);
        REQUIRE(fixture.protocol.completeInvocation(0, 1, "{\"late\":true}", true));
        REQUIRE(replies.replies().size() == 1);
        fixture.transport.close();
        REQUIRE(fixture.protocol.destroyedClients() == kExpectedModules.size());
        REQUIRE(fixture.protocol.lifecycleThreadViolations() == 0);
        REQUIRE(!fixture.protocol.completeInvocation(0, 1, "null", true));
    }
    return true;
}

bool independentHandlesDoNotShareProtocolState()
{
    Fixture first;
    REQUIRE(first.activate());
    Fixture second;
    REQUIRE(second.activate());

    ReplyCollector firstReplies;
    ReplyCollector secondReplies;
    const LogosInspectorHostTransportV1 firstVtable = first.transport.vtable();
    const LogosInspectorHostTransportV1 secondVtable = second.transport.vtable();
    REQUIRE(firstVtable.context != secondVtable.context);
    REQUIRE(firstVtable.dispatch(
                firstVtable.context,
                51,
                "delivery_module",
                "sendMessage",
                "[1]",
                &ReplyCollector::callback,
                &firstReplies)
        == 1);
    REQUIRE(secondVtable.dispatch(
                secondVtable.context,
                51,
                "delivery_module",
                "sendMessage",
                "[2]",
                &ReplyCollector::callback,
                &secondReplies)
        == 1);
    REQUIRE(first.protocol.completeInvocation(0, 1, "{\"owner\":1}", true));
    REQUIRE(second.protocol.completeInvocation(0, 1, "{\"owner\":2}", true));
    REQUIRE(firstReplies.replies()[0].payload == "{\"owner\":1}");
    REQUIRE(secondReplies.replies()[0].payload == "{\"owner\":2}");

    first.transport.close();
    REQUIRE(second.transport.ownsRuntimeModuleEvents());
    return true;
}
} // namespace

int main(int argc, char* argv[])
{
    QCoreApplication application(argc, argv);
    static_cast<void>(application);
    const std::array<std::pair<const char*, std::function<bool()>>, 16> tests = { {
        { "activationCreatesExactCatalogOnOwnerThread", activationCreatesExactCatalogOnOwnerThread },
        { "activationRollbackFailsClosed", activationRollbackFailsClosed },
        { "missingOptionalSubscriptionKeepsDispatchOpen", missingOptionalSubscriptionKeepsDispatchOpen },
        { "closeWaitsForAdmittedActivationStartup", closeWaitsForAdmittedActivationStartup },
        { "dispatchEnforcesAllowlistAndBounds", dispatchEnforcesAllowlistAndBounds },
        { "pendingAdmissionIsBoundedAndIdsStayReserved", pendingAdmissionIsBoundedAndIdsStayReserved },
        { "foreignResultsPreserveSuccessNullAndCanonicalFailure", foreignResultsPreserveSuccessNullAndCanonicalFailure },
        { "malformedResultStillCompletesAcceptedDispatch", malformedResultStillCompletesAcceptedDispatch },
        { "protocolAllocationExceptionDoesNotPinClose", protocolAllocationExceptionDoesNotPinClose },
        { "closeWaitsForImmediateErrorReplyCallback", closeWaitsForImmediateErrorReplyCallback },
        { "cancellationSuppressesReplyAndRetainsCallbackContext", cancellationSuppressesReplyAndRetainsCallbackContext },
        { "foreignCloseQuiescesThroughOwnerMarshal", foreignCloseQuiescesThroughOwnerMarshal },
        { "ownerClosePumpsForeignTeardownAndDestructorStaysIdempotent", ownerClosePumpsForeignTeardownAndDestructorStaysIdempotent },
        { "backpressureRetriesInFifoOrderWithoutBlockingCallback", backpressureRetriesInFifoOrderWithoutBlockingCallback },
        { "queueOverflowAndRejectedIngressFaultTransport", queueOverflowAndRejectedIngressFaultTransport },
        { "independentHandlesDoNotShareProtocolState", independentHandlesDoNotShareProtocolState },
    } };

    for (const auto& [name, test] : tests) {
        if (!test()) {
            std::cerr << "FAILED: " << name << '\n';
            return 1;
        }
        std::cout << "PASS: " << name << '\n';
    }
    return 0;
}
