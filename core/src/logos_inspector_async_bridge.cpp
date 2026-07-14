#include "logos_inspector_async_bridge.h"

#include <algorithm>
#include <atomic>
#include <condition_variable>
#include <initializer_list>
#include <limits>
#include <mutex>
#include <new>
#include <random>
#include <stdexcept>
#include <string_view>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

namespace {
constexpr std::string_view kSchema = "logos-inspector-async-bridge/v1";
constexpr std::string_view kInspectorModule = "logos_inspector";
constexpr std::string_view kTokenPrefix = "liab-";
constexpr std::size_t kTokenHexLength = 32;
constexpr std::size_t kMaxIdentifierBytes = 256;

enum class Lifecycle : uint8_t { open, closing, closed };
enum class EntryStatus : uint8_t {
    pending,
    ready,
    responseLimitExceeded,
    missingResponse,
    callbackIdMismatch,
    callbackFailure,
};

struct BridgeEntry
{
    uint64_t bridgeId = 0;
    std::string correlationId;
    std::string token;
    std::string module;
    std::string method;
    std::string argsJson;
    std::string responseJson;
    std::size_t inputBytes = 0;
    EntryStatus status = EntryStatus::pending;
    bool submitting = true;
    std::chrono::steady_clock::time_point expiresAt;
};

struct BridgeState
{
    explicit BridgeState(
        LogosInspectorAsyncBridgeLimits configuredLimits,
        LogosInspectorAsyncBridge::Clock configuredClock,
        uint64_t configuredTokenNamespace)
        : limits(configuredLimits)
        , clock(std::move(configuredClock))
        , tokenNamespace(configuredTokenNamespace)
    {
    }

    std::mutex mutex;
    std::condition_variable condition;
    Lifecycle lifecycle = Lifecycle::open;
    std::size_t activeAbiCalls = 0;
    std::size_t callbackOwners = 0;
    std::size_t reservedSlots = 0;
    std::size_t retainedInputBytes = 0;
    std::size_t retainedResponseBytes = 0;
    uint64_t nextBridgeId = 1;
    LogosInspectorAsyncBridgeLimits limits;
    LogosInspectorAsyncBridge::Clock clock;
    uint64_t tokenNamespace = 0;
    std::unordered_map<uint64_t, BridgeEntry> entries;
    std::unordered_map<std::string, uint64_t> correlationIndex;
    std::unordered_map<std::string, uint64_t> tokenIndex;
    std::unordered_set<uint64_t> detachedPending;
};

struct CallbackContext
{
    std::shared_ptr<BridgeState> state;
    uint64_t bridgeId = 0;
};

struct HostTransportContext
{
    std::atomic<bool> closed { false };
};

std::string jsonEscape(std::string_view value)
{
    static constexpr char kHex[] = "0123456789abcdef";
    std::string escaped;
    escaped.reserve(value.size());
    for (const unsigned char ch : value) {
        switch (ch) {
        case '\\':
            escaped += "\\\\";
            break;
        case '"':
            escaped += "\\\"";
            break;
        case '\b':
            escaped += "\\b";
            break;
        case '\f':
            escaped += "\\f";
            break;
        case '\n':
            escaped += "\\n";
            break;
        case '\r':
            escaped += "\\r";
            break;
        case '\t':
            escaped += "\\t";
            break;
        default:
            if (ch < 0x20) {
                escaped += "\\u00";
                escaped += kHex[(ch >> 4) & 0x0f];
                escaped += kHex[ch & 0x0f];
            } else {
                escaped += static_cast<char>(ch);
            }
            break;
        }
    }
    return escaped;
}

std::string jsonError(std::string_view error)
{
    return "{\"ok\":false,\"value\":null,\"text\":\"\",\"error\":\""
        + jsonEscape(error) + "\"}";
}

std::string jsonSuccess(std::string valueJson)
{
    return "{\"ok\":true,\"value\":" + std::move(valueJson)
        + ",\"text\":\"\",\"error\":\"\"}";
}

std::string startResponse(std::string_view correlationId, std::string_view token)
{
    return jsonSuccess(
        "{\"schema\":\"" + std::string(kSchema) + "\",\"correlationId\":\""
        + jsonEscape(correlationId) + "\",\"token\":\"" + jsonEscape(token) + "\"}");
}

std::string pollResponse(
    std::string_view token,
    std::string_view status,
    const std::string* responseJson = nullptr)
{
    std::string value = "{\"schema\":\"" + std::string(kSchema) + "\",\"token\":\""
        + jsonEscape(token) + "\",\"status\":\"" + std::string(status) + "\"";
    if (responseJson != nullptr) {
        value += ",\"responseJson\":\"" + jsonEscape(*responseJson) + "\"";
    }
    value += '}';
    return jsonSuccess(std::move(value));
}

std::string booleanResponse(std::string_view token, std::string_view field, bool value)
{
    return jsonSuccess(
        "{\"schema\":\"" + std::string(kSchema) + "\",\"token\":\""
        + jsonEscape(token) + "\",\"" + std::string(field) + "\":"
        + (value ? "true" : "false") + '}');
}

std::string terminalResponseJson(EntryStatus status)
{
    switch (status) {
    case EntryStatus::responseLimitExceeded:
        return jsonError("asynchronous response exceeds retained response capacity");
    case EntryStatus::missingResponse:
        return jsonError("asynchronous core callback returned no response");
    case EntryStatus::callbackIdMismatch:
        return jsonError("asynchronous core callback request id mismatch");
    case EntryStatus::callbackFailure:
        return jsonError("asynchronous core callback failed");
    case EntryStatus::pending:
    case EntryStatus::ready:
        break;
    }
    return jsonError("invalid asynchronous bridge state");
}

bool hasEmbeddedNul(std::string_view value)
{
    return value.find('\0') != std::string_view::npos;
}

bool checkedAdd(std::size_t& total, std::size_t value)
{
    if (value > (std::numeric_limits<std::size_t>::max)() - total) {
        return false;
    }
    total += value;
    return true;
}

bool inputSize(
    std::initializer_list<std::string_view> values,
    std::size_t maximum,
    std::size_t& result)
{
    result = 0;
    for (const std::string_view value : values) {
        if (!checkedAdd(result, value.size()) || result > maximum) {
            return false;
        }
    }
    return true;
}

bool validIdentifier(std::string_view value)
{
    return !value.empty() && value.size() <= kMaxIdentifierBytes && !hasEmbeddedNul(value);
}

bool validToken(std::string_view token)
{
    if (token.size() != kTokenPrefix.size() + kTokenHexLength
        || token.compare(0, kTokenPrefix.size(), kTokenPrefix) != 0) {
        return false;
    }
    for (std::size_t index = kTokenPrefix.size(); index < token.size(); ++index) {
        const char ch = token[index];
        if (!((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f'))) {
            return false;
        }
    }
    return true;
}

std::string hexadecimal(uint64_t value)
{
    static constexpr char kHex[] = "0123456789abcdef";
    std::string result(16, '0');
    for (std::size_t index = result.size(); index > 0; --index) {
        result[index - 1] = kHex[value & 0x0f];
        value >>= 4;
    }
    return result;
}

std::string makeToken(uint64_t tokenNamespace, uint64_t bridgeId)
{
    return std::string(kTokenPrefix) + hexadecimal(tokenNamespace) + hexadecimal(bridgeId);
}

uint64_t randomTokenNamespace()
{
    static std::atomic<uint64_t> sequence { 1 };
    const uint64_t ordinal = sequence.fetch_add(1, std::memory_order_relaxed);
    uint64_t entropy = static_cast<uint64_t>(
        std::chrono::steady_clock::now().time_since_epoch().count());
    try {
        std::random_device random;
        entropy ^= static_cast<uint64_t>(random()) << 32;
        entropy ^= static_cast<uint64_t>(random());
    } catch (...) {
        // Time and the process-local sequence remain a collision-resistant fallback.
    }
    entropy ^= ordinal * UINT64_C(0x9e3779b97f4a7c15);
    return entropy == 0 ? ordinal : entropy;
}

bool boundedCStringLength(const char* value, std::size_t maximum, std::size_t& length) noexcept
{
    if (value == nullptr) {
        return false;
    }
    for (length = 0; length <= maximum; ++length) {
        if (value[length] == '\0') {
            return true;
        }
    }
    return false;
}

void subtractBounded(std::size_t& value, std::size_t amount) noexcept
{
    value = amount <= value ? value - amount : 0;
}

void removeEntryLocked(
    BridgeState& state,
    std::unordered_map<uint64_t, BridgeEntry>::iterator entry)
{
    state.correlationIndex.erase(entry->second.correlationId);
    state.tokenIndex.erase(entry->second.token);
    subtractBounded(state.retainedInputBytes, entry->second.inputBytes);
    subtractBounded(state.retainedResponseBytes, entry->second.responseJson.size());
    subtractBounded(state.reservedSlots, 1);
    state.entries.erase(entry);
}

void detachPendingLocked(
    BridgeState& state,
    std::unordered_map<uint64_t, BridgeEntry>::iterator entry)
{
    state.detachedPending.insert(entry->first);
    state.correlationIndex.erase(entry->second.correlationId);
    state.tokenIndex.erase(entry->second.token);
    subtractBounded(state.retainedInputBytes, entry->second.inputBytes);
    state.entries.erase(entry);
}

std::vector<uint64_t> collectExpiredLocked(
    BridgeState& state,
    std::chrono::steady_clock::time_point now)
{
    std::vector<uint64_t> cancelIds;
    cancelIds.reserve(state.reservedSlots);
    for (auto entry = state.entries.begin(); entry != state.entries.end();) {
        if (entry->second.submitting || entry->second.expiresAt > now) {
            ++entry;
            continue;
        }
        auto expired = entry++;
        if (expired->second.status == EntryStatus::pending) {
            cancelIds.push_back(expired->first);
            detachPendingLocked(state, expired);
        } else {
            removeEntryLocked(state, expired);
        }
    }
    return cancelIds;
}

int32_t rejectingHostDispatch(
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

void rejectingHostCancel(void*, uint64_t) noexcept
{
}

void rejectingHostClose(void* context) noexcept
{
    if (context != nullptr) {
        static_cast<HostTransportContext*>(context)->closed.store(true, std::memory_order_release);
    }
}

class ActiveAbiCall
{
public:
    struct AdoptTag { };

    explicit ActiveAbiCall(std::shared_ptr<BridgeState> state)
        : state_(std::move(state))
    {
        std::lock_guard<std::mutex> lock(state_->mutex);
        if (state_->lifecycle == Lifecycle::open) {
            ++state_->activeAbiCalls;
            active_ = true;
        }
    }

    ActiveAbiCall(std::shared_ptr<BridgeState> state, AdoptTag)
        : state_(std::move(state))
        , active_(true)
    {
    }

    ~ActiveAbiCall()
    {
        release();
    }

    ActiveAbiCall(const ActiveAbiCall&) = delete;
    ActiveAbiCall& operator=(const ActiveAbiCall&) = delete;

    explicit operator bool() const noexcept
    {
        return active_;
    }

    void release()
    {
        if (!active_) {
            return;
        }
        std::lock_guard<std::mutex> lock(state_->mutex);
        subtractBounded(state_->activeAbiCalls, 1);
        active_ = false;
        state_->condition.notify_all();
    }

private:
    std::shared_ptr<BridgeState> state_;
    bool active_ = false;
};

class CoreString
{
public:
    CoreString(char* value, LogosInspectorCoreApi::StringFreeFn release)
        : value_(value)
        , release_(release)
    {
    }

    ~CoreString()
    {
        if (value_ != nullptr) {
            release_(value_);
        }
    }

    CoreString(const CoreString&) = delete;
    CoreString& operator=(const CoreString&) = delete;

private:
    char* value_ = nullptr;
    LogosInspectorCoreApi::StringFreeFn release_ = nullptr;
};

void markCallbackFailure(const std::shared_ptr<BridgeState>& state, uint64_t bridgeId) noexcept
{
    try {
        std::lock_guard<std::mutex> lock(state->mutex);
        const auto entry = state->entries.find(bridgeId);
        if (entry != state->entries.end() && entry->second.status == EntryStatus::pending) {
            entry->second.responseJson.clear();
            entry->second.status = EntryStatus::callbackFailure;
            entry->second.submitting = false;
            entry->second.expiresAt = std::chrono::steady_clock::now() + state->limits.entryTtl;
        } else if (entry == state->entries.end()
            && state->detachedPending.erase(bridgeId) != 0) {
            subtractBounded(state->reservedSlots, 1);
        }
        subtractBounded(state->callbackOwners, 1);
        state->condition.notify_all();
    } catch (...) {
        // Callback boundaries must never unwind into C; close still owns the state.
    }
}

void finishCoreReply(
    const std::shared_ptr<BridgeState>& state,
    uint64_t expectedBridgeId,
    uint64_t callbackBridgeId,
    const char* responseJson)
{
    std::lock_guard<std::mutex> lock(state->mutex);
    const auto entry = state->entries.find(expectedBridgeId);
    if (entry == state->entries.end()) {
        if (state->detachedPending.erase(expectedBridgeId) != 0) {
            subtractBounded(state->reservedSlots, 1);
        }
        subtractBounded(state->callbackOwners, 1);
        state->condition.notify_all();
        return;
    }

    BridgeEntry& value = entry->second;
    value.submitting = false;
    if (value.status == EntryStatus::pending) {
        if (callbackBridgeId != expectedBridgeId) {
            value.status = EntryStatus::callbackIdMismatch;
        } else if (responseJson == nullptr) {
            value.status = EntryStatus::missingResponse;
        } else {
            const std::size_t available = state->limits.maxRetainedResponseBytes
                    >= state->retainedResponseBytes
                ? state->limits.maxRetainedResponseBytes - state->retainedResponseBytes
                : 0;
            std::size_t responseLength = 0;
            if (!boundedCStringLength(responseJson, available, responseLength)) {
                value.status = EntryStatus::responseLimitExceeded;
            } else {
                value.responseJson.assign(responseJson, responseLength);
                state->retainedResponseBytes += responseLength;
                value.status = EntryStatus::ready;
            }
        }
        value.expiresAt = state->clock() + state->limits.entryTtl;
    }
    subtractBounded(state->callbackOwners, 1);
    state->condition.notify_all();
}

void coreReply(
    void* rawContext,
    uint64_t bridgeRequestId,
    const char* responseJson) noexcept
{
    std::unique_ptr<CallbackContext> context(static_cast<CallbackContext*>(rawContext));
    if (context == nullptr || context->state == nullptr) {
        return;
    }
    try {
        finishCoreReply(context->state, context->bridgeId, bridgeRequestId, responseJson);
    } catch (...) {
        markCallbackFailure(context->state, context->bridgeId);
    }
}

std::string publicFailure(std::string_view operation)
{
    return jsonError(std::string(operation) + " failed");
}
}

LogosInspectorCoreApi LogosInspectorCoreApi::production()
{
    LogosInspectorCoreApi api;
    api.newWithHostTransport = &logos_inspector_core_new_with_host_transport;
    api.close = &logos_inspector_core_close;
    api.free = &logos_inspector_core_free;
    api.call = &logos_inspector_core_call;
    api.stringFree = &logos_inspector_core_string_free;
    api.callModuleAsync = &logos_inspector_core_call_module_async;
    api.cancel = &logos_inspector_core_cancel;
    return api;
}

class LogosInspectorAsyncBridge::Impl
{
public:
    Impl(
        LogosInspectorCoreApi coreApi,
        LogosInspectorAsyncBridgeLimits limits,
        Clock clock,
        uint64_t tokenNamespace)
        : coreApi_(coreApi)
        , state_(std::make_shared<BridgeState>(
              validateLimits(limits),
              validateClock(std::move(clock)),
              tokenNamespace))
    {
        validateApi(coreApi_);
        transport_.abi_version = LOGOS_INSPECTOR_HOST_TRANSPORT_ABI_VERSION;
        transport_.struct_size = static_cast<uint32_t>(sizeof(transport_));
        transport_.context = &hostTransportContext_;
        transport_.dispatch = &rejectingHostDispatch;
        transport_.cancel = &rejectingHostCancel;
        transport_.close = &rejectingHostClose;
        core_ = coreApi_.newWithHostTransport(&transport_);
        if (core_ == nullptr) {
            throw std::runtime_error("could not construct Logos Inspector asynchronous core");
        }
    }

    ~Impl()
    {
        close();
    }

    std::string call(const std::string& method, const std::string& argsJson)
    {
        cancelExpired();
        if (!validIdentifier(method) || hasEmbeddedNul(argsJson)) {
            return jsonError("invalid synchronous Logos Inspector call input");
        }
        std::size_t bytes = 0;
        if (!inputSize({ method, argsJson }, state_->limits.maxSingleInputBytes, bytes)) {
            return jsonError("synchronous Logos Inspector call input exceeds capacity");
        }

        ActiveAbiCall active(state_);
        if (!active) {
            return jsonError("Logos Inspector asynchronous bridge is closed");
        }
        char* const rawResponse = coreApi_.call(core_, method.c_str(), argsJson.c_str());
        CoreString ownedResponse(rawResponse, coreApi_.stringFree);
        if (rawResponse == nullptr) {
            return jsonError("synchronous Logos Inspector core returned no response");
        }
        std::size_t responseLength = 0;
        if (!boundedCStringLength(
                rawResponse,
                state_->limits.maxRetainedResponseBytes,
                responseLength)) {
            return jsonError("synchronous Logos Inspector response exceeds capacity");
        }
        return std::string(rawResponse, responseLength);
    }

    std::string callAsync(
        const std::string& correlationId,
        const std::string& method,
        const std::string& argsJson)
    {
        return start(correlationId, std::string(kInspectorModule), method, argsJson);
    }

    std::string callModuleAsync(
        const std::string& correlationId,
        const std::string& module,
        const std::string& method,
        const std::string& argsJson)
    {
        return start(correlationId, module, method, argsJson);
    }

    std::string pollAsync(const std::string& token)
    {
        cancelExpired();
        if (!validToken(token)) {
            return jsonError("invalid asynchronous bridge token");
        }
        std::lock_guard<std::mutex> lock(state_->mutex);
        if (state_->lifecycle != Lifecycle::open) {
            return jsonError("Logos Inspector asynchronous bridge is closed");
        }
        const auto tokenEntry = state_->tokenIndex.find(token);
        if (tokenEntry == state_->tokenIndex.end()) {
            return jsonError("unknown or released asynchronous bridge token");
        }
        const auto entry = state_->entries.find(tokenEntry->second);
        if (entry == state_->entries.end()) {
            return jsonError("unknown or released asynchronous bridge token");
        }
        if (entry->second.status == EntryStatus::pending) {
            return pollResponse(token, "pending");
        }
        const std::string response = entry->second.status == EntryStatus::ready
            ? entry->second.responseJson
            : terminalResponseJson(entry->second.status);
        return pollResponse(token, "ready", &response);
    }

    std::string cancelAsync(const std::string& token)
    {
        cancelExpired();
        if (!validToken(token)) {
            return jsonError("invalid asynchronous bridge token");
        }

        uint64_t bridgeId = 0;
        {
            std::unique_lock<std::mutex> lock(state_->mutex);
            while (state_->lifecycle == Lifecycle::open) {
                const auto tokenEntry = state_->tokenIndex.find(token);
                if (tokenEntry == state_->tokenIndex.end()) {
                    return booleanResponse(token, "cancelled", false);
                }
                const auto entry = state_->entries.find(tokenEntry->second);
                if (entry == state_->entries.end()) {
                    return booleanResponse(token, "cancelled", false);
                }
                if (!entry->second.submitting) {
                    if (entry->second.status != EntryStatus::pending) {
                        return booleanResponse(token, "cancelled", false);
                    }
                    bridgeId = entry->first;
                    ++state_->activeAbiCalls;
                    break;
                }
                state_->condition.wait(lock);
            }
            if (state_->lifecycle != Lifecycle::open) {
                return jsonError("Logos Inspector asynchronous bridge is closed");
            }
        }

        ActiveAbiCall active(state_, ActiveAbiCall::AdoptTag {});
        const bool cancelled = coreApi_.cancel(core_, bridgeId) == 1;
        return booleanResponse(token, "cancelled", cancelled);
    }

    std::string releaseAsync(const std::string& token)
    {
        cancelExpired();
        if (!validToken(token)) {
            return jsonError("invalid asynchronous bridge token");
        }

        uint64_t pendingBridgeId = 0;
        {
            std::unique_lock<std::mutex> lock(state_->mutex);
            while (state_->lifecycle == Lifecycle::open) {
                const auto tokenEntry = state_->tokenIndex.find(token);
                if (tokenEntry == state_->tokenIndex.end()) {
                    return booleanResponse(token, "released", false);
                }
                auto entry = state_->entries.find(tokenEntry->second);
                if (entry == state_->entries.end()) {
                    return booleanResponse(token, "released", false);
                }
                if (entry->second.submitting) {
                    state_->condition.wait(lock);
                    continue;
                }
                if (entry->second.status == EntryStatus::pending) {
                    pendingBridgeId = entry->first;
                    detachPendingLocked(*state_, entry);
                    ++state_->activeAbiCalls;
                } else {
                    removeEntryLocked(*state_, entry);
                }
                break;
            }
            if (state_->lifecycle != Lifecycle::open) {
                return jsonError("Logos Inspector asynchronous bridge is closed");
            }
        }

        if (pendingBridgeId != 0) {
            ActiveAbiCall active(state_, ActiveAbiCall::AdoptTag {});
            static_cast<void>(coreApi_.cancel(core_, pendingBridgeId));
        }
        return booleanResponse(token, "released", true);
    }

    void close()
    {
        LogosInspectorCore* closingCore = nullptr;
        {
            std::unique_lock<std::mutex> lock(state_->mutex);
            if (state_->lifecycle == Lifecycle::closed) {
                return;
            }
            if (state_->lifecycle == Lifecycle::closing) {
                state_->condition.wait(lock, [&] {
                    return state_->lifecycle == Lifecycle::closed;
                });
                return;
            }
            state_->lifecycle = Lifecycle::closing;
            state_->condition.notify_all();
            closingCore = core_;
        }

        if (closingCore != nullptr) {
            coreApi_.close(closingCore);
        }

        {
            std::unique_lock<std::mutex> lock(state_->mutex);
            state_->condition.wait(lock, [&] {
                return state_->activeAbiCalls == 0 && state_->callbackOwners == 0;
            });
        }

        if (closingCore != nullptr) {
            coreApi_.free(closingCore);
        }

        {
            std::lock_guard<std::mutex> lock(state_->mutex);
            core_ = nullptr;
            state_->entries.clear();
            state_->correlationIndex.clear();
            state_->tokenIndex.clear();
            state_->detachedPending.clear();
            state_->reservedSlots = 0;
            state_->retainedInputBytes = 0;
            state_->retainedResponseBytes = 0;
            state_->lifecycle = Lifecycle::closed;
            state_->condition.notify_all();
        }
    }

private:
    static LogosInspectorAsyncBridgeLimits validateLimits(LogosInspectorAsyncBridgeLimits limits)
    {
        if (limits.maxSlots == 0 || limits.maxSingleInputBytes == 0
            || limits.maxRetainedInputBytes == 0
            || limits.maxRetainedResponseBytes == (std::numeric_limits<std::size_t>::max)()
            || limits.entryTtl <= std::chrono::milliseconds::zero()) {
            throw std::invalid_argument("invalid Logos Inspector asynchronous bridge limits");
        }
        return limits;
    }

    static Clock validateClock(Clock clock)
    {
        if (!clock) {
            throw std::invalid_argument("Logos Inspector asynchronous bridge clock is required");
        }
        return clock;
    }

    static void validateApi(const LogosInspectorCoreApi& api)
    {
        if (api.newWithHostTransport == nullptr || api.close == nullptr || api.free == nullptr
            || api.call == nullptr || api.stringFree == nullptr
            || api.callModuleAsync == nullptr || api.cancel == nullptr) {
            throw std::invalid_argument("incomplete Logos Inspector core API");
        }
    }

    std::string start(
        const std::string& correlationId,
        const std::string& module,
        const std::string& method,
        const std::string& argsJson)
    {
        cancelExpired();
        if (!validIdentifier(correlationId) || !validIdentifier(module)
            || !validIdentifier(method) || hasEmbeddedNul(argsJson)) {
            return jsonError("invalid asynchronous Logos Inspector call input");
        }
        std::size_t bytes = 0;
        if (!inputSize(
                { correlationId, module, method, argsJson },
                state_->limits.maxSingleInputBytes,
                bytes)) {
            return jsonError("asynchronous Logos Inspector call input exceeds capacity");
        }

        uint64_t bridgeId = 0;
        std::string token;
        std::unique_ptr<CallbackContext> callbackContext;
        {
            std::unique_lock<std::mutex> lock(state_->mutex);
            for (;;) {
                if (state_->lifecycle != Lifecycle::open) {
                    return jsonError("Logos Inspector asynchronous bridge is closed");
                }
                const auto correlation = state_->correlationIndex.find(correlationId);
                if (correlation == state_->correlationIndex.end()) {
                    break;
                }
                const auto existing = state_->entries.find(correlation->second);
                if (existing == state_->entries.end()) {
                    state_->correlationIndex.erase(correlation);
                    continue;
                }
                if (existing->second.module != module || existing->second.method != method
                    || existing->second.argsJson != argsJson) {
                    return jsonError(
                        "asynchronous correlation id already belongs to a different payload");
                }
                if (!existing->second.submitting) {
                    return startResponse(correlationId, existing->second.token);
                }
                state_->condition.wait(lock);
            }

            if (state_->reservedSlots >= state_->limits.maxSlots) {
                return jsonError("asynchronous bridge slot capacity reached");
            }
            if (bytes > state_->limits.maxRetainedInputBytes - (std::min)(
                    state_->retainedInputBytes,
                    state_->limits.maxRetainedInputBytes)) {
                return jsonError("asynchronous bridge retained input capacity reached");
            }

            for (std::size_t attempts = 0; attempts <= state_->reservedSlots + 1; ++attempts) {
                bridgeId = state_->nextBridgeId++;
                if (bridgeId != 0 && state_->entries.count(bridgeId) == 0
                    && state_->detachedPending.count(bridgeId) == 0) {
                    break;
                }
                bridgeId = 0;
            }
            if (bridgeId == 0) {
                return jsonError("asynchronous bridge request id space exhausted");
            }

            token = makeToken(state_->tokenNamespace, bridgeId);
            callbackContext = std::make_unique<CallbackContext>(
                CallbackContext { state_, bridgeId });
            BridgeEntry entry;
            entry.bridgeId = bridgeId;
            entry.correlationId = correlationId;
            entry.token = token;
            entry.module = module;
            entry.method = method;
            entry.argsJson = argsJson;
            entry.inputBytes = bytes;
            entry.expiresAt = state_->clock() + state_->limits.entryTtl;

            try {
                const auto inserted = state_->entries.emplace(bridgeId, std::move(entry));
                if (!inserted.second) {
                    return jsonError("asynchronous bridge request id collision");
                }
                const auto correlationInserted = state_->correlationIndex.emplace(
                    correlationId,
                    bridgeId);
                if (!correlationInserted.second) {
                    state_->entries.erase(bridgeId);
                    return jsonError("asynchronous bridge correlation collision");
                }
                const auto tokenInserted = state_->tokenIndex.emplace(token, bridgeId);
                if (!tokenInserted.second) {
                    state_->correlationIndex.erase(correlationId);
                    state_->entries.erase(bridgeId);
                    return jsonError("asynchronous bridge token collision");
                }
            } catch (...) {
                state_->tokenIndex.erase(token);
                state_->correlationIndex.erase(correlationId);
                state_->entries.erase(bridgeId);
                throw;
            }
            ++state_->reservedSlots;
            state_->retainedInputBytes += bytes;
            ++state_->callbackOwners;
            ++state_->activeAbiCalls;
        }

        int32_t accepted = 0;
        try {
            accepted = coreApi_.callModuleAsync(
                core_,
                bridgeId,
                module.c_str(),
                method.c_str(),
                argsJson.c_str(),
                &coreReply,
                callbackContext.get());
        } catch (...) {
            accepted = 0;
        }
        if (accepted == 1) {
            [[maybe_unused]] CallbackContext* const transferredContext =
                callbackContext.release();
        }

        {
            std::lock_guard<std::mutex> lock(state_->mutex);
            subtractBounded(state_->activeAbiCalls, 1);
            const auto entry = state_->entries.find(bridgeId);
            if (accepted == 1) {
                if (entry != state_->entries.end()) {
                    entry->second.submitting = false;
                }
            } else {
                if (entry != state_->entries.end()) {
                    removeEntryLocked(*state_, entry);
                }
                subtractBounded(state_->callbackOwners, 1);
            }
            state_->condition.notify_all();
        }

        if (accepted != 1) {
            return jsonError("asynchronous Logos Inspector core rejected the call");
        }
        return startResponse(correlationId, token);
    }

    void cancelExpired()
    {
        std::vector<uint64_t> cancelIds;
        {
            std::lock_guard<std::mutex> lock(state_->mutex);
            if (state_->lifecycle != Lifecycle::open) {
                return;
            }
            cancelIds = collectExpiredLocked(*state_, state_->clock());
        }
        for (const uint64_t bridgeId : cancelIds) {
            ActiveAbiCall active(state_);
            if (!active) {
                return;
            }
            static_cast<void>(coreApi_.cancel(core_, bridgeId));
        }
    }

    LogosInspectorCoreApi coreApi_;
    std::shared_ptr<BridgeState> state_;
    HostTransportContext hostTransportContext_;
    LogosInspectorHostTransportV1 transport_ {};
    LogosInspectorCore* core_ = nullptr;
};

LogosInspectorAsyncBridge::LogosInspectorAsyncBridge()
    : LogosInspectorAsyncBridge(
          LogosInspectorCoreApi::production(),
          LogosInspectorAsyncBridgeLimits {},
          [] { return std::chrono::steady_clock::now(); },
          randomTokenNamespace())
{
}

LogosInspectorAsyncBridge::LogosInspectorAsyncBridge(
    LogosInspectorCoreApi coreApi,
    LogosInspectorAsyncBridgeLimits limits,
    Clock clock,
    uint64_t tokenNamespace)
    : impl_(std::make_unique<Impl>(
          coreApi,
          limits,
          std::move(clock),
          tokenNamespace))
{
}

LogosInspectorAsyncBridge::~LogosInspectorAsyncBridge() = default;

std::string LogosInspectorAsyncBridge::call(
    const std::string& method,
    const std::string& argsJson)
{
    try {
        return impl_->call(method, argsJson);
    } catch (...) {
        return publicFailure("synchronous Logos Inspector call");
    }
}

std::string LogosInspectorAsyncBridge::callAsync(
    const std::string& correlationId,
    const std::string& method,
    const std::string& argsJson)
{
    try {
        return impl_->callAsync(correlationId, method, argsJson);
    } catch (...) {
        return publicFailure("asynchronous Logos Inspector call");
    }
}

std::string LogosInspectorAsyncBridge::callModuleAsync(
    const std::string& correlationId,
    const std::string& module,
    const std::string& method,
    const std::string& argsJson)
{
    try {
        return impl_->callModuleAsync(correlationId, module, method, argsJson);
    } catch (...) {
        return publicFailure("asynchronous Logos module call");
    }
}

std::string LogosInspectorAsyncBridge::pollAsync(const std::string& token)
{
    try {
        return impl_->pollAsync(token);
    } catch (...) {
        return publicFailure("asynchronous Logos Inspector poll");
    }
}

std::string LogosInspectorAsyncBridge::cancelAsync(const std::string& token)
{
    try {
        return impl_->cancelAsync(token);
    } catch (...) {
        return publicFailure("asynchronous Logos Inspector cancellation");
    }
}

std::string LogosInspectorAsyncBridge::releaseAsync(const std::string& token)
{
    try {
        return impl_->releaseAsync(token);
    } catch (...) {
        return publicFailure("asynchronous Logos Inspector release");
    }
}

void LogosInspectorAsyncBridge::close()
{
    impl_->close();
}
