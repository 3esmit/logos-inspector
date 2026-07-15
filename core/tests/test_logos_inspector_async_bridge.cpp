#include "logos_inspector_async_bridge.h"
#include "logos_inspector_host_transport.h"
#include "logos_inspector_impl.h"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstring>
#include <deque>
#include <functional>
#include <iostream>
#include <mutex>
#include <new>
#include <stdexcept>
#include <string>
#include <thread>
#include <utility>
#include <vector>

namespace {
using namespace std::chrono_literals;

struct FakeRuntime;

struct PendingReply
{
    uint64_t bridgeId = 0;
    LogosInspectorCoreReplyFn reply = nullptr;
    void* context = nullptr;
};
}

struct LogosInspectorCore
{
    FakeRuntime* runtime = nullptr;
    LogosInspectorHostTransportV1 transport {};
    bool closed = false;
    bool runtimeEventHealth = false;
};

namespace {
struct FakeRuntime
{
    std::mutex mutex;
    std::condition_variable condition;
    LogosInspectorHostTransportV1 capturedTransport {};
    LogosInspectorCore* core = nullptr;
    std::deque<PendingReply> pending;
    std::deque<PendingReply> selected;
    std::string synchronousResponse =
        "{\"ok\":true,\"value\":{\"local\":true},\"text\":\"\",\"error\":\"\"}";
    std::string asynchronousResponse =
        "{\"ok\":true,\"value\":{\"height\":7},\"text\":\"\",\"error\":\"\"}";
    std::string cancellationResponse =
        "{\"ok\":false,\"value\":null,\"text\":\"\",\"error\":\"cancelled\"}";
    std::string lastModule;
    std::string lastMethod;
    std::string lastArgs;
    uint64_t lastBridgeId = 0;
    int newCalls = 0;
    int closeCalls = 0;
    int freeCalls = 0;
    int localCalls = 0;
    int moduleCalls = 0;
    int stringFreeCalls = 0;
    int asyncCalls = 0;
    int cancelCalls = 0;
    int eventHealthCalls = 0;
    bool hostClosed = false;
    bool rejectNext = false;
    bool inlineReply = false;
    bool deferCancelCallback = false;
    bool blockAsyncEntry = false;
    bool asyncEntryEntered = false;
    bool allowAsyncEntry = false;
    bool blockLocalCall = false;
    bool localCallEntered = false;
    bool allowLocalCall = false;

    PendingReply takePending()
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (pending.empty()) {
            throw std::runtime_error("fake core has no pending callback");
        }
        PendingReply value = pending.front();
        pending.pop_front();
        return value;
    }

    PendingReply takeSelected()
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (selected.empty()) {
            throw std::runtime_error("fake core has no selected callback");
        }
        PendingReply value = selected.front();
        selected.pop_front();
        return value;
    }

    void completePending(
        const std::string& response,
        uint64_t callbackIdOverride = 0)
    {
        const PendingReply value = takePending();
        value.reply(
            value.context,
            callbackIdOverride == 0 ? value.bridgeId : callbackIdOverride,
            response.c_str());
    }

    void completeSelected(const std::string& response)
    {
        const PendingReply value = takeSelected();
        value.reply(value.context, value.bridgeId, response.c_str());
    }

    void allowBlockedAsyncEntry()
    {
        std::lock_guard<std::mutex> lock(mutex);
        allowAsyncEntry = true;
        condition.notify_all();
    }

    void allowBlockedLocalCall()
    {
        std::lock_guard<std::mutex> lock(mutex);
        allowLocalCall = true;
        condition.notify_all();
    }

    bool waitFor(const std::function<bool(const FakeRuntime&)>& predicate)
    {
        std::unique_lock<std::mutex> lock(mutex);
        return condition.wait_for(lock, 2s, [&] { return predicate(*this); });
    }
};

FakeRuntime* activeRuntime = nullptr;

class RuntimeScope
{
public:
    explicit RuntimeScope(FakeRuntime& runtime)
    {
        if (activeRuntime != nullptr) {
            throw std::runtime_error("nested fake runtime");
        }
        activeRuntime = &runtime;
    }

    ~RuntimeScope()
    {
        activeRuntime = nullptr;
    }

    RuntimeScope(const RuntimeScope&) = delete;
    RuntimeScope& operator=(const RuntimeScope&) = delete;
};

FakeRuntime& runtime()
{
    if (activeRuntime == nullptr) {
        throw std::runtime_error("fake runtime is not installed");
    }
    return *activeRuntime;
}

void require(bool condition, const char* expression, int line)
{
    if (!condition) {
        throw std::runtime_error(
            "requirement failed at line " + std::to_string(line) + ": " + expression);
    }
}

#define REQUIRE(expression) require((expression), #expression, __LINE__)

std::string extractToken(const std::string& response)
{
    const std::string marker = "\"token\":\"";
    const std::size_t start = response.find(marker);
    if (start == std::string::npos) {
        return {};
    }
    const std::size_t valueStart = start + marker.size();
    const std::size_t end = response.find('"', valueStart);
    if (end == std::string::npos) {
        return {};
    }
    return response.substr(valueStart, end - valueStart);
}

bool contains(const std::string& value, const std::string& expected)
{
    return value.find(expected) != std::string::npos;
}

LogosInspectorCoreApi fakeApi();

char* ownedCopy(const std::string& response)
{
    auto* value = new (std::nothrow) char[response.size() + 1];
    if (value == nullptr) {
        return nullptr;
    }
    std::memcpy(value, response.c_str(), response.size() + 1);
    return value;
}
}

extern "C" LogosInspectorCore* logos_inspector_core_new()
{
    FakeRuntime& fake = runtime();
    auto* core = new (std::nothrow) LogosInspectorCore;
    if (core == nullptr) {
        return nullptr;
    }
    core->runtime = &fake;
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        fake.core = core;
        ++fake.newCalls;
        fake.condition.notify_all();
    }
    return core;
}

extern "C" LogosInspectorCore* logos_inspector_core_new_with_host_transport(
    const LogosInspectorHostTransportV1* transport)
{
    if (transport == nullptr) {
        return nullptr;
    }
    FakeRuntime& fake = runtime();
    auto* core = new (std::nothrow) LogosInspectorCore;
    if (core == nullptr) {
        return nullptr;
    }
    core->runtime = &fake;
    core->transport = *transport;
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        fake.core = core;
        fake.capturedTransport = *transport;
        ++fake.newCalls;
        fake.condition.notify_all();
    }
    return core;
}

extern "C" void logos_inspector_core_close(LogosInspectorCore* core)
{
    if (core == nullptr) {
        return;
    }
    FakeRuntime& fake = *core->runtime;
    std::deque<PendingReply> pending;
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        if (core->closed) {
            return;
        }
        core->closed = true;
        ++fake.closeCalls;
        pending.swap(fake.pending);
        fake.condition.notify_all();
    }
    if (core->transport.close != nullptr) {
        core->transport.close(core->transport.context);
        std::lock_guard<std::mutex> lock(fake.mutex);
        fake.hostClosed = true;
        fake.condition.notify_all();
    }
    for (const PendingReply& value : pending) {
        value.reply(value.context, value.bridgeId, fake.cancellationResponse.c_str());
    }
}

extern "C" void logos_inspector_core_free(LogosInspectorCore* core)
{
    if (core == nullptr) {
        return;
    }
    logos_inspector_core_close(core);
    FakeRuntime& fake = *core->runtime;
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        ++fake.freeCalls;
        fake.core = nullptr;
        fake.condition.notify_all();
    }
    delete core;
}

extern "C" char* logos_inspector_core_call(
    LogosInspectorCore* core,
    const char* method,
    const char* argsJson)
{
    if (core == nullptr || method == nullptr || argsJson == nullptr) {
        return nullptr;
    }
    FakeRuntime& fake = *core->runtime;
    std::string response;
    {
        std::unique_lock<std::mutex> lock(fake.mutex);
        ++fake.localCalls;
        fake.lastMethod = method;
        fake.lastArgs = argsJson;
        if (fake.blockLocalCall) {
            fake.localCallEntered = true;
            fake.condition.notify_all();
            fake.condition.wait(lock, [&] { return fake.allowLocalCall; });
        }
        response = fake.synchronousResponse;
    }
    return ownedCopy(response);
}

extern "C" char* logos_inspector_core_call_module(
    LogosInspectorCore* core,
    const char* module,
    const char* method,
    const char* argsJson)
{
    if (core == nullptr || module == nullptr || method == nullptr || argsJson == nullptr) {
        return nullptr;
    }
    FakeRuntime& fake = *core->runtime;
    std::string response;
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        ++fake.moduleCalls;
        fake.lastModule = module;
        fake.lastMethod = method;
        fake.lastArgs = argsJson;
        response = fake.synchronousResponse;
    }
    return ownedCopy(response);
}

extern "C" void logos_inspector_core_string_free(char* value)
{
    if (value != nullptr) {
        {
            FakeRuntime& fake = runtime();
            std::lock_guard<std::mutex> lock(fake.mutex);
            ++fake.stringFreeCalls;
            fake.condition.notify_all();
        }
        delete[] value;
    }
}

extern "C" int32_t logos_inspector_core_call_module_async(
    LogosInspectorCore* core,
    uint64_t bridgeRequestId,
    const char* module,
    const char* method,
    const char* argsJson,
    LogosInspectorCoreReplyFn reply,
    void* replyContext)
{
    if (core == nullptr || bridgeRequestId == 0 || module == nullptr || method == nullptr
        || argsJson == nullptr || reply == nullptr) {
        return 0;
    }
    FakeRuntime& fake = *core->runtime;
    bool inlineReply = false;
    std::string response;
    {
        std::unique_lock<std::mutex> lock(fake.mutex);
        ++fake.asyncCalls;
        fake.lastBridgeId = bridgeRequestId;
        fake.lastModule = module;
        fake.lastMethod = method;
        fake.lastArgs = argsJson;
        if (fake.blockAsyncEntry) {
            fake.asyncEntryEntered = true;
            fake.condition.notify_all();
            fake.condition.wait(lock, [&] { return fake.allowAsyncEntry; });
        }
        if (core->closed) {
            return 0;
        }
        if (fake.rejectNext) {
            fake.rejectNext = false;
            return 0;
        }
        inlineReply = fake.inlineReply;
        response = fake.asynchronousResponse;
        if (!inlineReply) {
            fake.pending.push_back(PendingReply { bridgeRequestId, reply, replyContext });
        }
        fake.condition.notify_all();
    }
    if (inlineReply) {
        reply(replyContext, bridgeRequestId, response.c_str());
    }
    return 1;
}

extern "C" int32_t logos_inspector_core_cancel(
    LogosInspectorCore* core,
    uint64_t bridgeRequestId)
{
    if (core == nullptr) {
        return 0;
    }
    FakeRuntime& fake = *core->runtime;
    PendingReply selected;
    bool found = false;
    bool defer = false;
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        ++fake.cancelCalls;
        for (auto entry = fake.pending.begin(); entry != fake.pending.end(); ++entry) {
            if (entry->bridgeId != bridgeRequestId) {
                continue;
            }
            selected = *entry;
            fake.pending.erase(entry);
            found = true;
            defer = fake.deferCancelCallback;
            if (defer) {
                fake.selected.push_back(selected);
            }
            break;
        }
        fake.condition.notify_all();
    }
    if (!found) {
        return 0;
    }
    if (!defer) {
        selected.reply(
            selected.context,
            selected.bridgeId,
            fake.cancellationResponse.c_str());
    }
    return 1;
}

extern "C" int32_t logos_inspector_core_ingest_module_event(
    LogosInspectorCore* core,
    const char* module,
    const char* event,
    const char* argsJson)
{
    if (core == nullptr || core->closed || module == nullptr || *module == '\0'
        || event == nullptr || *event == '\0' || argsJson == nullptr) {
        return LOGOS_INSPECTOR_EVENT_REJECTED;
    }
    return LOGOS_INSPECTOR_EVENT_ACCEPTED;
}

extern "C" int32_t logos_inspector_core_set_runtime_module_event_health(
    LogosInspectorCore* core,
    int32_t ready)
{
    if (core == nullptr || core->closed || (ready != 0 && ready != 1)) {
        return 0;
    }
    FakeRuntime& fake = *core->runtime;
    std::lock_guard<std::mutex> lock(fake.mutex);
    core->runtimeEventHealth = ready == 1;
    ++fake.eventHealthCalls;
    fake.condition.notify_all();
    return 1;
}

namespace {
class FakeHostTransport final : public LogosInspectorHostTransport
{
public:
    bool bindCore(
        LogosInspectorCore* core,
        IngestModuleEventFn ingest,
        SetRuntimeModuleEventHealthFn setEventHealth) noexcept override
    {
        if (core == nullptr || ingest == nullptr || setEventHealth == nullptr || bound_) {
            return false;
        }
        core_ = core;
        setEventHealth_ = setEventHealth;
        bound_ = true;
        return true;
    }

    bool activate() noexcept override
    {
        if (!bound_ || active_ || closed_) {
            return false;
        }
        if (setEventHealth_(core_, 1) != 1) {
            return false;
        }
        active_ = true;
        return true;
    }

    LogosInspectorHostTransportV1 vtable() noexcept override
    {
        LogosInspectorHostTransportV1 value {};
        value.abi_version = LOGOS_INSPECTOR_HOST_TRANSPORT_ABI_VERSION;
        value.struct_size = static_cast<uint32_t>(sizeof(value));
        value.context = this;
        value.dispatch = &dispatch;
        value.cancel = &cancel;
        value.close = &closeCallback;
        return value;
    }

    bool ownsRuntimeModuleEvents() const noexcept override
    {
        return active_ && !closed_;
    }

    void close() noexcept override
    {
        active_ = false;
        closed_ = true;
    }

private:
    static int32_t dispatch(
        void*,
        uint64_t,
        const char*,
        const char*,
        const char*,
        LogosInspectorHostReplyFn,
        void*) noexcept
    {
        return 0;
    }

    static void cancel(void*, uint64_t) noexcept
    {
    }

    static void closeCallback(void* context) noexcept
    {
        if (context != nullptr) {
            static_cast<FakeHostTransport*>(context)->close();
        }
    }

    bool bound_ = false;
    bool active_ = false;
    bool closed_ = false;
    LogosInspectorCore* core_ = nullptr;
    SetRuntimeModuleEventHealthFn setEventHealth_ = nullptr;
};

LogosInspectorCoreApi fakeApi()
{
    LogosInspectorCoreApi api;
    api.newWithHostTransport = &logos_inspector_core_new_with_host_transport;
    api.close = &logos_inspector_core_close;
    api.free = &logos_inspector_core_free;
    api.call = &logos_inspector_core_call;
    api.stringFree = &logos_inspector_core_string_free;
    api.callModuleAsync = &logos_inspector_core_call_module_async;
    api.cancel = &logos_inspector_core_cancel;
    api.ingestModuleEvent = &logos_inspector_core_ingest_module_event;
    api.setRuntimeModuleEventHealth =
        &logos_inspector_core_set_runtime_module_event_health;
    return api;
}

LogosInspectorAsyncBridge makeBridge(
    FakeRuntime&,
    LogosInspectorAsyncBridgeLimits limits = {},
    LogosInspectorAsyncBridge::Clock clock = [] {
        return std::chrono::steady_clock::now();
    })
{
    return LogosInspectorAsyncBridge(
        fakeApi(),
        std::move(limits),
        std::move(clock),
        0x1234,
        std::make_unique<FakeHostTransport>());
}

void testWrapperUsesOneCoreForLocalAndAsyncCalls()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    {
        LogosInspectorImpl wrapper([] {
            return std::make_unique<FakeHostTransport>();
        });
        REQUIRE(fake.newCalls == 0);
        REQUIRE(contains(wrapper.call("head", "[]"), "not initialized"));
        REQUIRE(wrapper.asyncBridgeSchema() == "logos-inspector-async-bridge/unavailable");
        REQUIRE(!wrapper.logosInspectorOwnsRuntimeModuleEvents());

        wrapper._logosCoreSetContext_("/module", "instance", "/state");
        REQUIRE(fake.newCalls == 1);
        REQUIRE(fake.eventHealthCalls == 1);
        REQUIRE(fake.core != nullptr && fake.core->runtimeEventHealth);
        REQUIRE(fake.capturedTransport.abi_version == LOGOS_INSPECTOR_HOST_TRANSPORT_ABI_VERSION);
        REQUIRE(fake.capturedTransport.struct_size == sizeof(LogosInspectorHostTransportV1));
        REQUIRE(fake.capturedTransport.context != nullptr);
        REQUIRE(fake.capturedTransport.dispatch != nullptr);
        REQUIRE(fake.capturedTransport.cancel != nullptr);
        REQUIRE(fake.capturedTransport.close != nullptr);
        REQUIRE(wrapper.asyncBridgeSchema() == "logos-inspector-async-bridge/v1");
        REQUIRE(fake.capturedTransport.dispatch(
                    fake.capturedTransport.context,
                    1,
                    "module",
                    "method",
                    "[]",
                    nullptr,
                    nullptr)
            == 0);

        REQUIRE(wrapper.call("head", "[]") == fake.synchronousResponse);
        REQUIRE(fake.localCalls == 1);
        REQUIRE(fake.stringFreeCalls == 1);
        REQUIRE(fake.lastMethod == "head");
        REQUIRE(fake.lastArgs == "[]");

        const std::string rejected = wrapper.callModule("wallet", "accounts", "[]");
        REQUIRE(contains(rejected, "use callModuleAsync"));
        REQUIRE(fake.newCalls == 1);

        const std::string started = wrapper.callAsync("wrapper-correlation", "head", "[]");
        const std::string token = extractToken(started);
        REQUIRE(!token.empty());
        REQUIRE(fake.lastModule == "logos_inspector");
        fake.completePending(fake.asynchronousResponse);
        REQUIRE(contains(wrapper.pollAsync(token), "\"status\":\"ready\""));
        REQUIRE(contains(wrapper.releaseAsync(token), "\"released\":true"));
        REQUIRE(wrapper.logosInspectorOwnsRuntimeModuleEvents());

        wrapper._logosCoreSetContext_("/module", "instance", "/state");
        REQUIRE(fake.newCalls == 1);
    }
    REQUIRE(fake.closeCalls == 1);
    REQUIRE(fake.freeCalls == 1);
    REQUIRE(fake.hostClosed);
}

void testIdentityIdempotentPollAndExplicitRelease()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto bridge = makeBridge(fake);

    const std::string first = bridge.callModuleAsync("correlation", "wallet", "accounts", "[1]");
    const std::string token = extractToken(first);
    REQUIRE(contains(first, "\"schema\":\"logos-inspector-async-bridge/v1\""));
    REQUIRE(contains(first, "\"correlationId\":\"correlation\""));
    REQUIRE(token.size() == 37);
    REQUIRE(token.rfind("liab-", 0) == 0);
    REQUIRE(fake.lastBridgeId != 0);
    REQUIRE(fake.lastModule == "wallet");
    REQUIRE(fake.lastMethod == "accounts");
    REQUIRE(fake.lastArgs == "[1]");

    const std::string duplicate = bridge.callModuleAsync(
        "correlation",
        "wallet",
        "accounts",
        "[1]");
    REQUIRE(duplicate == first);
    REQUIRE(fake.asyncCalls == 1);
    REQUIRE(contains(
        bridge.callModuleAsync("correlation", "wallet", "accounts", "[2]"),
        "different payload"));

    const std::string pending = bridge.pollAsync(token);
    REQUIRE(contains(pending, "\"status\":\"pending\""));
    REQUIRE(bridge.pollAsync(token) == pending);

    std::thread callback([&] { fake.completePending(fake.asynchronousResponse); });
    callback.join();
    const std::string ready = bridge.pollAsync(token);
    REQUIRE(contains(ready, "\"status\":\"ready\""));
    REQUIRE(contains(ready, "\"responseJson\":"));
    REQUIRE(contains(ready, "\\\"height\\\":7"));
    REQUIRE(bridge.pollAsync(token) == ready);
    REQUIRE(contains(bridge.cancelAsync(token), "\"cancelled\":false"));
    REQUIRE(bridge.pollAsync(token) == ready);
    REQUIRE(contains(bridge.releaseAsync(token), "\"released\":true"));
    REQUIRE(contains(bridge.pollAsync(token), "unknown or released"));
    REQUIRE(contains(bridge.releaseAsync(token), "\"released\":false"));
}

void testInlineCallbackCancellationAndRejectedIngress()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto bridge = makeBridge(fake);

    fake.inlineReply = true;
    const std::string inlineStart = bridge.callAsync("inline", "head", "[]");
    const std::string inlineToken = extractToken(inlineStart);
    REQUIRE(!inlineToken.empty());
    REQUIRE(contains(bridge.pollAsync(inlineToken), "\"status\":\"ready\""));
    REQUIRE(contains(bridge.releaseAsync(inlineToken), "\"released\":true"));

    fake.inlineReply = false;
    const std::string cancellable = bridge.callAsync("cancel", "head", "[]");
    const std::string cancelToken = extractToken(cancellable);
    REQUIRE(contains(bridge.cancelAsync(cancelToken), "\"cancelled\":true"));
    const std::string cancelledPoll = bridge.pollAsync(cancelToken);
    REQUIRE(contains(cancelledPoll, "\"status\":\"ready\""));
    REQUIRE(contains(cancelledPoll, "cancelled"));
    REQUIRE(bridge.pollAsync(cancelToken) == cancelledPoll);
    REQUIRE(contains(bridge.releaseAsync(cancelToken), "\"released\":true"));

    fake.rejectNext = true;
    REQUIRE(contains(bridge.callAsync("rejected", "head", "[]"), "rejected the call"));
    const std::string retry = bridge.callAsync("rejected", "head", "[]");
    REQUIRE(!extractToken(retry).empty());
    fake.completePending(fake.asynchronousResponse);
}

void testReleasedPendingCallRetainsSlotUntilCallback()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    LogosInspectorAsyncBridgeLimits limits;
    limits.maxSlots = 1;
    auto bridge = makeBridge(fake, limits);

    fake.deferCancelCallback = true;
    const std::string first = bridge.callAsync("first", "head", "[]");
    const std::string firstToken = extractToken(first);
    REQUIRE(contains(bridge.releaseAsync(firstToken), "\"released\":true"));
    REQUIRE(contains(bridge.callAsync("second", "head", "[]"), "slot capacity"));

    fake.completeSelected(fake.cancellationResponse);
    fake.deferCancelCallback = false;
    const std::string second = bridge.callAsync("second", "head", "[]");
    REQUIRE(!extractToken(second).empty());
    fake.completePending(fake.asynchronousResponse);
}

void testInputAndResponseBudgets()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    LogosInspectorAsyncBridgeLimits limits;
    limits.maxSlots = 4;
    limits.maxSingleInputBytes = 20;
    limits.maxRetainedInputBytes = 20;
    limits.maxRetainedResponseBytes = 8;
    auto bridge = makeBridge(fake, limits);

    const std::string first = bridge.callModuleAsync("c1", "m", "go", "[12345678]");
    const std::string firstToken = extractToken(first);
    REQUIRE(!firstToken.empty());
    REQUIRE(contains(
        bridge.callModuleAsync("c2", "m", "go", "[12345678]"),
        "retained input capacity"));
    REQUIRE(contains(
        bridge.callModuleAsync("large", "m", "go", "[12345678901234567890]"),
        "input exceeds capacity"));

    fake.completePending("01234567890123456789");
    const std::string overLimit = bridge.pollAsync(firstToken);
    REQUIRE(contains(overLimit, "\"status\":\"ready\""));
    REQUIRE(contains(overLimit, "retained response capacity"));
    REQUIRE(contains(bridge.releaseAsync(firstToken), "\"released\":true"));

    const std::string second = bridge.callModuleAsync("c2", "m", "go", "[12345678]");
    const std::string secondToken = extractToken(second);
    REQUIRE(!secondToken.empty());
    fake.completePending("1234567");
    REQUIRE(contains(bridge.pollAsync(secondToken), "1234567"));
}

void testTtlReclaimsPendingAndTerminalEntries()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto now = std::chrono::steady_clock::time_point {};
    LogosInspectorAsyncBridgeLimits limits;
    limits.entryTtl = 10ms;
    auto bridge = makeBridge(fake, limits, [&] { return now; });

    const std::string pending = bridge.callAsync("ttl-pending", "head", "[]");
    const std::string pendingToken = extractToken(pending);
    now += 11ms;
    REQUIRE(contains(bridge.pollAsync(pendingToken), "unknown or released"));
    REQUIRE(fake.cancelCalls == 1);

    const std::string reused = bridge.callAsync("ttl-pending", "head", "[]");
    REQUIRE(extractToken(reused) != pendingToken);
    fake.completePending(fake.asynchronousResponse);
    const std::string reusedToken = extractToken(reused);
    REQUIRE(contains(bridge.releaseAsync(reusedToken), "\"released\":true"));

    const std::string terminal = bridge.callAsync("ttl-terminal", "head", "[]");
    const std::string terminalToken = extractToken(terminal);
    fake.completePending(fake.asynchronousResponse);
    REQUIRE(contains(bridge.pollAsync(terminalToken), "\"status\":\"ready\""));
    const int cancelsBeforeTerminalExpiry = fake.cancelCalls;
    now += 11ms;
    REQUIRE(contains(bridge.pollAsync(terminalToken), "unknown or released"));
    REQUIRE(fake.cancelCalls == cancelsBeforeTerminalExpiry);
}

void testBackupImportPendingOwnershipOutlivesMailboxTtl()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto now = std::chrono::steady_clock::time_point {};
    LogosInspectorAsyncBridgeLimits limits;
    limits.entryTtl = 10ms;
    auto bridge = makeBridge(fake, limits, [&] { return now; });

    const std::string started = bridge.callAsync(
        "authoritative-backup-import",
        "settingsBackupImportApply",
        R"(["backup-1",{}])");
    const std::string token = extractToken(started);
    REQUIRE(!token.empty());
    now += 11ms;
    REQUIRE(contains(bridge.pollAsync(token), "\"status\":\"pending\""));
    REQUIRE(fake.cancelCalls == 0);

    fake.completePending(fake.asynchronousResponse);
    REQUIRE(contains(bridge.pollAsync(token), "\"status\":\"ready\""));
    REQUIRE(contains(bridge.releaseAsync(token), "\"released\":true"));
}

void testCallbackIdMismatchBecomesStableTerminalError()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto bridge = makeBridge(fake);
    const std::string started = bridge.callAsync("mismatch", "head", "[]");
    const std::string token = extractToken(started);
    const uint64_t wrongId = fake.lastBridgeId + 1;
    fake.completePending(fake.asynchronousResponse, wrongId);
    const std::string first = bridge.pollAsync(token);
    REQUIRE(contains(first, "request id mismatch"));
    REQUIRE(bridge.pollAsync(token) == first);
}

void testCloseWaitsForActiveAsyncAbiCall()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto bridge = makeBridge(fake);
    fake.blockAsyncEntry = true;

    std::string startResponseValue;
    std::thread caller([&] {
        startResponseValue = bridge.callAsync("blocked-async", "head", "[]");
    });
    REQUIRE(fake.waitFor([](const FakeRuntime& value) { return value.asyncEntryEntered; }));

    std::thread closer([&] { bridge.close(); });
    std::this_thread::sleep_for(20ms);
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        REQUIRE(fake.closeCalls == 1);
        REQUIRE(fake.freeCalls == 0);
    }
    fake.allowBlockedAsyncEntry();
    caller.join();
    closer.join();
    REQUIRE(contains(startResponseValue, "rejected the call"));
    REQUIRE(fake.closeCalls == 1);
    REQUIRE(fake.freeCalls == 1);
    bridge.close();
    REQUIRE(fake.closeCalls == 1);
    REQUIRE(fake.freeCalls == 1);
}

void testCloseWaitsForActiveLocalCall()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto bridge = makeBridge(fake);
    fake.blockLocalCall = true;

    std::string localResponse;
    std::thread caller([&] { localResponse = bridge.call("head", "[]"); });
    REQUIRE(fake.waitFor([](const FakeRuntime& value) { return value.localCallEntered; }));

    std::thread closer([&] { bridge.close(); });
    std::this_thread::sleep_for(20ms);
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        REQUIRE(fake.closeCalls == 1);
        REQUIRE(fake.freeCalls == 0);
        REQUIRE(fake.stringFreeCalls == 0);
    }
    fake.allowBlockedLocalCall();
    caller.join();
    closer.join();
    REQUIRE(localResponse == fake.synchronousResponse);
    REQUIRE(fake.stringFreeCalls == 1);
    REQUIRE(fake.freeCalls == 1);
}

void testCloseInterruptsPendingHostWorkBeforeJoiningBlockedLocalCall()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto bridge = makeBridge(fake);

    const std::string hostBacked = bridge.callModuleAsync(
        "host-backed",
        "wallet",
        "accounts",
        "[]");
    REQUIRE(!extractToken(hostBacked).empty());
    fake.blockLocalCall = true;
    std::string localResponse;
    std::thread localCaller([&] { localResponse = bridge.call("head", "[]"); });
    REQUIRE(fake.waitFor([](const FakeRuntime& value) { return value.localCallEntered; }));

    std::thread closer([&] { bridge.close(); });
    REQUIRE(fake.waitFor([](const FakeRuntime& value) { return value.closeCalls == 1; }));
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        REQUIRE(fake.pending.empty());
        REQUIRE(fake.freeCalls == 0);
    }

    fake.allowBlockedLocalCall();
    localCaller.join();
    closer.join();
    REQUIRE(localResponse == fake.synchronousResponse);
    REQUIRE(fake.stringFreeCalls == 1);
    REQUIRE(fake.freeCalls == 1);
}

void testCloseWaitsForAcceptedCallbackOwnership()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto bridge = makeBridge(fake);
    fake.deferCancelCallback = true;
    const std::string started = bridge.callAsync("selected", "head", "[]");
    const std::string token = extractToken(started);
    REQUIRE(contains(bridge.releaseAsync(token), "\"released\":true"));

    std::thread closer([&] { bridge.close(); });
    REQUIRE(fake.waitFor([](const FakeRuntime& value) { return value.closeCalls == 1; }));
    {
        std::lock_guard<std::mutex> lock(fake.mutex);
        REQUIRE(fake.freeCalls == 0);
    }
    fake.completeSelected(fake.cancellationResponse);
    closer.join();
    REQUIRE(fake.freeCalls == 1);
}

void testInvalidInputsNeverReachCore()
{
    FakeRuntime fake;
    RuntimeScope runtimeScope(fake);
    auto bridge = makeBridge(fake);
    REQUIRE(contains(bridge.callAsync("", "head", "[]"), "invalid"));
    REQUIRE(contains(bridge.callModuleAsync("c", "", "head", "[]"), "invalid"));
    REQUIRE(contains(bridge.call("", "[]"), "invalid"));
    REQUIRE(contains(bridge.pollAsync("not-a-token"), "invalid"));
    REQUIRE(fake.asyncCalls == 0);
    REQUIRE(fake.localCalls == 0);
}
}

int main()
{
    const std::vector<std::pair<const char*, std::function<void()>>> tests {
        { "wrapper uses one core", testWrapperUsesOneCoreForLocalAndAsyncCalls },
        { "identity and idempotent poll", testIdentityIdempotentPollAndExplicitRelease },
        { "inline cancellation and reject", testInlineCallbackCancellationAndRejectedIngress },
        { "released pending slot", testReleasedPendingCallRetainsSlotUntilCallback },
        { "input and response budgets", testInputAndResponseBudgets },
        { "ttl reclamation", testTtlReclaimsPendingAndTerminalEntries },
        { "backup import ownership outlives ttl",
            testBackupImportPendingOwnershipOutlivesMailboxTtl },
        { "callback id mismatch", testCallbackIdMismatchBecomesStableTerminalError },
        { "active async lifecycle", testCloseWaitsForActiveAsyncAbiCall },
        { "active local lifecycle", testCloseWaitsForActiveLocalCall },
        { "close interrupts host work", testCloseInterruptsPendingHostWorkBeforeJoiningBlockedLocalCall },
        { "callback ownership lifecycle", testCloseWaitsForAcceptedCallbackOwnership },
        { "invalid inputs", testInvalidInputsNeverReachCore },
    };

    for (const auto& test : tests) {
        try {
            test.second();
            std::cout << "PASS: " << test.first << '\n';
        } catch (const std::exception& error) {
            std::cerr << "FAIL: " << test.first << ": " << error.what() << '\n';
            return 1;
        }
    }
    return 0;
}
