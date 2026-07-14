#include "logos_protocol_host_transport.h"

#include <QAbstractEventDispatcher>
#include <QCoreApplication>
#include <QEventLoop>
#include <QThread>

#include <array>
#include <condition_variable>
#include <deque>
#include <limits>
#include <mutex>
#include <string>
#include <string_view>
#include <thread>
#include <unordered_map>
#include <utility>
#include <vector>

namespace {
constexpr std::string_view kOriginModule = "logos_inspector";
constexpr std::size_t kMaxIdentifierBytes = 256;
constexpr char kFaultError[] =
    R"({"code":"transport_closed","message":"native module event ingress failed; host transport closed","origin":"logos_inspector"})";
constexpr char kPayloadRetentionError[] =
    R"({"code":"invoke_failed","message":"host transport could not retain module result","origin":"logos_inspector"})";
constexpr int kOwnerEventPumpSliceMs = 10;
constexpr auto kOwnerEventPumpPause = std::chrono::milliseconds(1);

constexpr std::array<std::string_view, 6> kModules = {
    "blockchain_module",
    "storage_module",
    "delivery_module",
    "capability_module",
    "lez_indexer_module",
    "lez_core",
};

struct EventSpec
{
    std::string_view module;
    std::string_view event;
};

constexpr std::array<EventSpec, 17> kEvents = { {
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

bool checkedAdd(std::size_t& total, std::size_t amount) noexcept
{
    if (amount > (std::numeric_limits<std::size_t>::max)() - total) {
        return false;
    }
    total += amount;
    return true;
}

bool boundedCStringLength(
    const char* value,
    std::size_t maximum,
    std::size_t& length) noexcept
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

bool allowedModule(std::string_view module) noexcept
{
    for (const std::string_view allowed : kModules) {
        if (module == allowed) {
            return true;
        }
    }
    return false;
}

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

std::string immediateErrorJson(int status, std::string_view origin)
{
    std::string_view code = "invoke_failed";
    std::string_view message = "logos-protocol could not dispatch module invocation";
    switch (status) {
    case LP_ERR_INVALID_ARG:
        code = "invalid_argument";
        message = "logos-protocol rejected invocation arguments";
        break;
    case LP_ERR_UNSUPPORTED:
        code = "unsupported";
        message = "logos-protocol does not support module invocation";
        break;
    case LP_ERR_UNAVAILABLE:
        code = "object_unavailable";
        message = "target module/object could not be acquired";
        break;
    case LP_ERR_INTERNAL:
    default:
        break;
    }
    return "{\"code\":\"" + std::string(code) + "\",\"message\":\""
        + std::string(message) + "\",\"origin\":\"" + jsonEscape(origin) + "\"}";
}

std::string malformedResultJson(std::string_view origin)
{
    return "{\"code\":\"invoke_failed\",\"message\":\"logos-protocol returned an invalid result payload\",\"origin\":\""
        + jsonEscape(origin) + "\"}";
}
} // namespace

LogosProtocolApi LogosProtocolApi::production() noexcept
{
    LogosProtocolApi api;
    api.clientCreate = &lp_client_create;
    api.clientDestroy = &lp_client_destroy;
    api.invokeAsync = &lp_invoke_async;
    api.subscribe = &lp_subscribe;
    api.unsubscribe = &lp_unsubscribe;
    return api;
}

class LogosProtocolHostTransport::Impl
{
public:
    Impl(LogosProtocolApi protocolApi, LogosProtocolHostTransportLimits configuredLimits)
        : api_(protocolApi)
        , limits_(configuredLimits)
    {
    }

    ~Impl()
    {
        close();
    }

    bool bindCore(LogosInspectorCore* core, IngestModuleEventFn ingest) noexcept
    {
        if (core == nullptr || ingest == nullptr) {
            return false;
        }
        try {
            std::lock_guard<std::mutex> lock(mutex_);
            if (lifecycle_ != Lifecycle::dormant || core_ != nullptr || ingest_ != nullptr) {
                return false;
            }
            core_ = core;
            ingest_ = ingest;
            return true;
        } catch (...) {
            return false;
        }
    }

    bool activate() noexcept
    {
        try {
            std::unique_lock<std::mutex> startupLock(joinMutex_);
            {
                std::lock_guard<std::mutex> lock(mutex_);
                if (lifecycle_ != Lifecycle::dormant || core_ == nullptr || ingest_ == nullptr
                    || !validApi() || !validLimits()) {
                    return false;
                }
                lifecycle_ = Lifecycle::activating;
                setupComplete_ = false;
                activationInProgress_ = true;
                ownsEvents_ = true;
                protocolOwnerThread_ = std::this_thread::get_id();
                protocolOwnerThreadAssigned_ = true;
            }

            try {
                worker_ = std::thread([this] { workerEntry(); });
            } catch (...) {
                std::lock_guard<std::mutex> lock(mutex_);
                lifecycle_ = Lifecycle::closed;
                setupComplete_ = true;
                activationInProgress_ = false;
                changed_.notify_all();
                return false;
            }
            startupLock.unlock();

            {
                std::unique_lock<std::mutex> lock(mutex_);
                changed_.wait(lock, [this] {
                    return workerLive_ || lifecycle_ == Lifecycle::closed;
                });
                if (!workerLive_ || lifecycle_ != Lifecycle::activating) {
                    setupComplete_ = true;
                    changed_.notify_all();
                    lock.unlock();
                    finishFailedActivation();
                    return false;
                }
            }

            if (!createClients()) {
                finishFailedActivation();
                return false;
            }

            const bool eventCatalogComplete = createSubscriptions();
            {
                std::lock_guard<std::mutex> lock(mutex_);
                if (lifecycle_ != Lifecycle::activating) {
                    setupComplete_ = true;
                    changed_.notify_all();
                }
            }
            if (!isActivating()) {
                finishFailedActivation();
                return false;
            }
            if (!eventCatalogComplete) {
                clearPartialSubscriptions();
                if (!isActivating()) {
                    finishFailedActivation();
                    return false;
                }
            }

            {
                std::lock_guard<std::mutex> lock(mutex_);
                if (lifecycle_ != Lifecycle::activating) {
                    setupComplete_ = true;
                    changed_.notify_all();
                } else {
                    lifecycle_ = Lifecycle::open;
                    setupComplete_ = true;
                    activationInProgress_ = false;
                    ownsEvents_ = eventCatalogComplete;
                    changed_.notify_all();
                    return true;
                }
            }
            finishFailedActivation();
            return false;
        } catch (...) {
            finishFailedActivation();
            return false;
        }
    }

    LogosInspectorHostTransportV1 vtable() noexcept
    {
        LogosInspectorHostTransportV1 result {};
        result.abi_version = LOGOS_INSPECTOR_HOST_TRANSPORT_ABI_VERSION;
        result.struct_size = static_cast<uint32_t>(sizeof(result));
        result.context = this;
        result.dispatch = &dispatchCallback;
        result.cancel = &cancelCallback;
        result.close = &closeCallback;
        return result;
    }

    bool ownsRuntimeModuleEvents() const noexcept
    {
        try {
            std::lock_guard<std::mutex> lock(mutex_);
            return lifecycle_ == Lifecycle::open && ownsEvents_ && workerLive_;
        } catch (...) {
            return false;
        }
    }

    void close() noexcept
    {
        try {
            bool activationOwnsTeardown = false;
            bool retryWorkerClose = false;
            {
                std::lock_guard<std::mutex> lock(mutex_);
                activationOwnsTeardown = activationInProgress_;
                retryWorkerClose = workerThreadAssigned_
                    && workerThread_ == std::this_thread::get_id();
                switch (lifecycle_) {
                case Lifecycle::dormant:
                    lifecycle_ = Lifecycle::closed;
                    setupComplete_ = true;
                    ownsEvents_ = false;
                    changed_.notify_all();
                    return;
                case Lifecycle::activating:
                case Lifecycle::open:
                    lifecycle_ = Lifecycle::closing;
                    ownsEvents_ = false;
                    suppressPendingLocked();
                    changed_.notify_all();
                    break;
                case Lifecycle::faulting:
                    ownsEvents_ = false;
                    changed_.notify_all();
                    break;
                case Lifecycle::closing:
                    break;
                case Lifecycle::closed:
                    return;
                }
            }
            if (retryWorkerClose) {
                return;
            }
            joinWorker();
            if (activationOwnsTeardown) {
                std::unique_lock<std::mutex> lock(mutex_);
                waitWithOwnerEventPumpingLocked(lock, [this] {
                    return lifecycle_ == Lifecycle::closed;
                });
                return;
            }
            teardownProtocolAfterWorker();
        } catch (...) {
            // C transport close must never unwind across the ABI seam.
        }
    }

private:
    enum class Lifecycle : uint8_t { dormant, activating, open, faulting, closing, closed };

    struct ClientRecord
    {
        std::string module;
        lp_client* handle = nullptr;
    };

    struct SubscriptionRecord
    {
        Impl* owner = nullptr;
        std::string module;
        std::string event;
        lp_subscription* handle = nullptr;
    };

    struct PendingRequest
    {
        Impl* owner = nullptr;
        uint64_t requestId = 0;
        lp_client* client = nullptr;
        LogosInspectorHostReplyFn reply = nullptr;
        void* replyContext = nullptr;
        std::string module;
        std::string method;
        std::string argsJson;
        std::size_t retainedBytes = 0;
        bool invoking = true;
        bool callbackFinished = false;
        bool cancelled = false;
        bool terminal = false;
    };

    struct QueuedEvent
    {
        std::string module;
        std::string event;
        std::string argsJson;
        std::size_t retainedBytes = 0;
    };

    struct ReplyAction
    {
        LogosInspectorHostReplyFn reply = nullptr;
        void* context = nullptr;
        uint64_t requestId = 0;
        int32_t ok = 0;
        std::string payload;
        const char* staticPayload = nullptr;
    };

    class ActiveInvokeGuard
    {
    public:
        ActiveInvokeGuard(
            Impl* owner,
            uint64_t requestId,
            PendingRequest* request) noexcept
            : owner_(owner)
            , requestId_(requestId)
            , request_(request)
        {
        }

        ~ActiveInvokeGuard()
        {
            finish();
        }

        ActiveInvokeGuard(const ActiveInvokeGuard&) = delete;
        ActiveInvokeGuard& operator=(const ActiveInvokeGuard&) = delete;

        void finish() noexcept
        {
            if (owner_ == nullptr) {
                return;
            }
            owner_->finishActiveInvoke(requestId_, request_);
            owner_ = nullptr;
        }

    private:
        Impl* owner_ = nullptr;
        uint64_t requestId_ = 0;
        PendingRequest* request_ = nullptr;
    };

    static int32_t dispatchCallback(
        void* context,
        uint64_t moduleRequestId,
        const char* module,
        const char* method,
        const char* argsJson,
        LogosInspectorHostReplyFn reply,
        void* replyContext) noexcept
    {
        if (context == nullptr) {
            return 0;
        }
        try {
            return static_cast<Impl*>(context)->dispatch(
                moduleRequestId,
                module,
                method,
                argsJson,
                reply,
                replyContext);
        } catch (...) {
            return 0;
        }
    }

    static void cancelCallback(void* context, uint64_t moduleRequestId) noexcept
    {
        if (context == nullptr) {
            return;
        }
        try {
            static_cast<Impl*>(context)->cancel(moduleRequestId);
        } catch (...) {
        }
    }

    static void closeCallback(void* context) noexcept
    {
        if (context == nullptr) {
            return;
        }
        static_cast<Impl*>(context)->close();
    }

    static void resultCallback(int ok, const char* json, void* userData) noexcept
    {
        if (userData == nullptr) {
            return;
        }
        auto* request = static_cast<PendingRequest*>(userData);
        try {
            request->owner->complete(request, ok, json);
        } catch (...) {
            try {
                request->owner->complete(request, 0, nullptr);
            } catch (...) {
            }
        }
    }

    static void eventCallback(
        const char* eventName,
        const char* dataJson,
        void* userData) noexcept
    {
        if (userData == nullptr) {
            return;
        }
        auto* subscription = static_cast<SubscriptionRecord*>(userData);
        try {
            subscription->owner->ingestEvent(subscription, eventName, dataJson);
        } catch (...) {
            subscription->owner->requestFaultFromCallback();
        }
    }

    bool validApi() const noexcept
    {
        return api_.clientCreate != nullptr && api_.clientDestroy != nullptr
            && api_.invokeAsync != nullptr && api_.subscribe != nullptr
            && api_.unsubscribe != nullptr;
    }

    bool validLimits() const noexcept
    {
        return limits_.maxPendingRequests > 0 && limits_.maxSingleRequestBytes > 0
            && limits_.maxRetainedRequestBytes >= limits_.maxSingleRequestBytes
            && limits_.maxSingleResultBytes > 0 && limits_.maxQueuedEvents > 0
            && limits_.maxSingleEventBytes > 0
            && limits_.maxQueuedEventBytes >= limits_.maxSingleEventBytes
            && limits_.invokeTimeoutMs > 0 && limits_.retryDelay.count() >= 0;
    }

    bool createClients()
    {
        for (const std::string_view module : kModules) {
            {
                std::lock_guard<std::mutex> lock(mutex_);
                if (lifecycle_ != Lifecycle::activating) {
                    return false;
                }
            }
            lp_client* const handle = api_.clientCreate(
                module.data(),
                kOriginModule.data(),
                nullptr,
                nullptr);
            if (handle == nullptr) {
                return false;
            }
            try {
                std::lock_guard<std::mutex> lock(mutex_);
                clients_.push_back(ClientRecord { std::string(module), handle });
                if (lifecycle_ != Lifecycle::activating) {
                    return false;
                }
            } catch (...) {
                try {
                    api_.clientDestroy(handle);
                } catch (...) {
                }
                throw;
            }
        }
        return true;
    }

    bool createSubscriptions()
    {
        for (const EventSpec& event : kEvents) {
            lp_client* client = nullptr;
            auto record = std::make_unique<SubscriptionRecord>();
            record->owner = this;
            record->module = event.module;
            record->event = event.event;
            SubscriptionRecord* rawRecord = record.get();
            {
                std::lock_guard<std::mutex> lock(mutex_);
                if (lifecycle_ != Lifecycle::activating) {
                    return false;
                }
                client = clientForModuleLocked(event.module);
                if (client == nullptr) {
                    return false;
                }
                subscriptions_.push_back(std::move(record));
            }

            lp_subscription* const handle = api_.subscribe(
                client,
                rawRecord->event.c_str(),
                &eventCallback,
                rawRecord);
            {
                std::lock_guard<std::mutex> lock(mutex_);
                rawRecord->handle = handle;
                if (handle == nullptr || lifecycle_ != Lifecycle::activating) {
                    return false;
                }
            }
        }
        return true;
    }

    bool isActivating() const
    {
        std::lock_guard<std::mutex> lock(mutex_);
        return lifecycle_ == Lifecycle::activating;
    }

    void clearPartialSubscriptions() noexcept
    {
        std::vector<std::unique_ptr<SubscriptionRecord>> subscriptions;
        try {
            {
                std::lock_guard<std::mutex> lock(mutex_);
                ownsEvents_ = false;
                subscriptions = std::move(subscriptions_);
                eventQueue_.clear();
                queuedEventBytes_ = 0;
            }
            quiesceSubscriptions(subscriptions);
            {
                std::lock_guard<std::mutex> lock(mutex_);
                eventQueue_.clear();
                queuedEventBytes_ = 0;
            }
        } catch (...) {
        }
    }

    void finishFailedActivation() noexcept
    {
        try {
            {
                std::lock_guard<std::mutex> lock(mutex_);
                if (lifecycle_ == Lifecycle::activating) {
                    lifecycle_ = Lifecycle::closing;
                    suppressPendingLocked();
                }
                ownsEvents_ = false;
                setupComplete_ = true;
                changed_.notify_all();
            }
            joinWorker();
            teardownProtocolAfterWorker();
        } catch (...) {
        }
    }

    lp_client* clientForModuleLocked(std::string_view module) const noexcept
    {
        for (const ClientRecord& client : clients_) {
            if (client.module == module) {
                return client.handle;
            }
        }
        return nullptr;
    }

    int32_t dispatch(
        uint64_t requestId,
        const char* moduleValue,
        const char* methodValue,
        const char* argsValue,
        LogosInspectorHostReplyFn reply,
        void* replyContext)
    {
        if (requestId == 0 || reply == nullptr) {
            return 0;
        }

        std::size_t moduleLength = 0;
        std::size_t methodLength = 0;
        std::size_t argsLength = 0;
        if (!boundedCStringLength(moduleValue, kMaxIdentifierBytes, moduleLength)
            || moduleLength == 0
            || !boundedCStringLength(methodValue, kMaxIdentifierBytes, methodLength)
            || methodLength == 0
            || !boundedCStringLength(
                argsValue,
                limits_.maxSingleRequestBytes,
                argsLength)) {
            return 0;
        }

        const std::string_view moduleView(moduleValue, moduleLength);
        if (!allowedModule(moduleView)) {
            return 0;
        }

        std::size_t retainedBytes = 0;
        if (!checkedAdd(retainedBytes, moduleLength)
            || !checkedAdd(retainedBytes, methodLength)
            || !checkedAdd(retainedBytes, argsLength)
            || retainedBytes > limits_.maxSingleRequestBytes) {
            return 0;
        }

        auto request = std::make_unique<PendingRequest>();
        request->owner = this;
        request->requestId = requestId;
        request->reply = reply;
        request->replyContext = replyContext;
        request->module.assign(moduleValue, moduleLength);
        request->method.assign(methodValue, methodLength);
        request->argsJson.assign(argsValue, argsLength);
        request->retainedBytes = retainedBytes;
        PendingRequest* const rawRequest = request.get();

        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (lifecycle_ != Lifecycle::open || !workerLive_
                || pending_.size() >= limits_.maxPendingRequests
                || pending_.find(requestId) != pending_.end()
                || retainedBytes > limits_.maxRetainedRequestBytes - retainedRequestBytes_) {
                return 0;
            }
            request->client = clientForModuleLocked(request->module);
            if (request->client == nullptr) {
                return 0;
            }
            pending_.emplace(requestId, std::move(request));
            retainedRequestBytes_ += retainedBytes;
            ++activeInvokes_;
        }
        ActiveInvokeGuard activeInvoke(this, requestId, rawRequest);

        int status = LP_ERR_INTERNAL;
        try {
            status = api_.invokeAsync(
                rawRequest->client,
                rawRequest->method.c_str(),
                rawRequest->argsJson.c_str(),
                limits_.invokeTimeoutMs,
                &resultCallback,
                rawRequest);
        } catch (...) {
            status = LP_ERR_INTERNAL;
        }

        ReplyAction immediate;
        bool issueImmediateReply = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            const auto found = pending_.find(requestId);
            if (found != pending_.end() && found->second.get() == rawRequest) {
                PendingRequest& pending = *found->second;
                if (status != LP_OK) {
                    pending.callbackFinished = true;
                    if (lifecycle_ == Lifecycle::open && !pending.cancelled
                        && !pending.terminal) {
                        immediate.reply = pending.reply;
                        immediate.context = pending.replyContext;
                        immediate.requestId = pending.requestId;
                        immediate.ok = 0;
                        try {
                            immediate.payload = immediateErrorJson(status, pending.module);
                        } catch (...) {
                            immediate.staticPayload = kPayloadRetentionError;
                        }
                        pending.terminal = true;
                        issueImmediateReply = true;
                    }
                }
            }
        }

        if (issueImmediateReply) {
            invokeReply(immediate);
        }
        activeInvoke.finish();
        return 1;
    }

    void cancel(uint64_t requestId)
    {
        std::lock_guard<std::mutex> lock(mutex_);
        const auto found = pending_.find(requestId);
        if (found == pending_.end()) {
            return;
        }
        found->second->cancelled = true;
        eraseFinishedRequestLocked(requestId, found->second.get());
    }

    void complete(PendingRequest* request, int ok, const char* json)
    {
        std::size_t payloadLength = 0;
        const bool validPayload = boundedCStringLength(
            json,
            limits_.maxSingleResultBytes,
            payloadLength);

        ReplyAction action;
        bool issueReply = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            const auto found = pending_.find(request->requestId);
            if (found == pending_.end() || found->second.get() != request) {
                return;
            }
            PendingRequest& pending = *found->second;
            if (lifecycle_ == Lifecycle::open && !pending.cancelled && !pending.terminal) {
                action.reply = pending.reply;
                action.context = pending.replyContext;
                action.requestId = pending.requestId;
                try {
                    if (validPayload) {
                        action.ok = ok != 0 ? 1 : 0;
                        action.payload.assign(json, payloadLength);
                    } else {
                        action.ok = 0;
                        action.payload = malformedResultJson(pending.module);
                    }
                } catch (...) {
                    action.ok = 0;
                    action.staticPayload = kPayloadRetentionError;
                }
                pending.terminal = true;
                issueReply = true;
            }
        }

        if (issueReply) {
            invokeReply(action);
        }

        {
            std::lock_guard<std::mutex> lock(mutex_);
            const auto found = pending_.find(request->requestId);
            if (found != pending_.end() && found->second.get() == request) {
                found->second->callbackFinished = true;
                eraseFinishedRequestLocked(request->requestId, request);
            }
            changed_.notify_all();
        }
    }

    static void invokeReply(const ReplyAction& action) noexcept
    {
        if (action.reply == nullptr) {
            return;
        }
        try {
            action.reply(
                action.context,
                action.requestId,
                action.ok,
                action.staticPayload == nullptr
                    ? action.payload.c_str()
                    : action.staticPayload);
        } catch (...) {
        }
    }

    void finishActiveInvoke(uint64_t requestId, PendingRequest* request) noexcept
    {
        try {
            std::lock_guard<std::mutex> lock(mutex_);
            const auto found = pending_.find(requestId);
            if (found != pending_.end() && found->second.get() == request) {
                found->second->invoking = false;
            }
            if (activeInvokes_ > 0) {
                --activeInvokes_;
            }
            eraseFinishedRequestLocked(requestId, request);
            changed_.notify_all();
        } catch (...) {
        }
    }

    void eraseFinishedRequestLocked(uint64_t requestId, PendingRequest* expected)
    {
        const auto found = pending_.find(requestId);
        if (found == pending_.end() || found->second.get() != expected) {
            return;
        }
        const PendingRequest& request = *found->second;
        if (request.invoking || !request.callbackFinished
            || (!request.terminal && !request.cancelled)) {
            return;
        }
        retainedRequestBytes_ = request.retainedBytes <= retainedRequestBytes_
            ? retainedRequestBytes_ - request.retainedBytes
            : 0;
        pending_.erase(found);
    }

    void ingestEvent(
        const SubscriptionRecord* subscription,
        const char* eventName,
        const char* dataJson)
    {
        std::size_t eventLength = 0;
        std::size_t dataLength = 0;
        const bool valid = boundedCStringLength(
                               eventName,
                               kMaxIdentifierBytes,
                               eventLength)
            && std::string_view(eventName, eventLength) == subscription->event
            && boundedCStringLength(
                dataJson,
                limits_.maxSingleEventBytes,
                dataLength);

        std::size_t retainedBytes = subscription->module.size();
        const bool validSize = valid && checkedAdd(retainedBytes, subscription->event.size())
            && checkedAdd(retainedBytes, dataLength)
            && retainedBytes <= limits_.maxSingleEventBytes;

        std::unique_lock<std::mutex> lock(mutex_);
        if ((lifecycle_ != Lifecycle::activating && lifecycle_ != Lifecycle::open)
            || !ownsEvents_) {
            return;
        }
        if (!validSize) {
            requestFaultLocked();
            return;
        }

        QueuedEvent event;
        event.module = subscription->module;
        event.event = subscription->event;
        event.argsJson.assign(dataJson, dataLength);
        event.retainedBytes = retainedBytes;

        if (lifecycle_ == Lifecycle::activating || !eventQueue_.empty()) {
            enqueueEventLocked(std::move(event));
            return;
        }

        int32_t status = LOGOS_INSPECTOR_EVENT_REJECTED;
        try {
            status = ingest_(
                core_,
                event.module.c_str(),
                event.event.c_str(),
                event.argsJson.c_str());
        } catch (...) {
            status = LOGOS_INSPECTOR_EVENT_REJECTED;
        }
        if (status == LOGOS_INSPECTOR_EVENT_ACCEPTED) {
            return;
        }
        if (status == LOGOS_INSPECTOR_EVENT_BACKPRESSURE) {
            enqueueEventLocked(std::move(event));
            return;
        }
        requestFaultLocked();
    }

    void enqueueEventLocked(QueuedEvent event)
    {
        if (eventQueue_.size() >= limits_.maxQueuedEvents
            || event.retainedBytes > limits_.maxQueuedEventBytes - queuedEventBytes_) {
            requestFaultLocked();
            return;
        }
        queuedEventBytes_ += event.retainedBytes;
        eventQueue_.push_back(std::move(event));
        changed_.notify_all();
    }

    void requestFaultFromCallback() noexcept
    {
        try {
            std::lock_guard<std::mutex> lock(mutex_);
            requestFaultLocked();
        } catch (...) {
        }
    }

    void requestFaultLocked() noexcept
    {
        if (lifecycle_ == Lifecycle::activating || lifecycle_ == Lifecycle::open) {
            lifecycle_ = Lifecycle::faulting;
            ownsEvents_ = false;
            changed_.notify_all();
        }
    }

    void suppressPendingLocked() noexcept
    {
        for (auto& [requestId, request] : pending_) {
            static_cast<void>(requestId);
            request->cancelled = true;
        }
    }

    void workerEntry() noexcept
    {
        try {
            workerLoop();
        } catch (...) {
            emergencyClose();
        }
    }

    void workerLoop()
    {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            workerThread_ = std::this_thread::get_id();
            workerThreadAssigned_ = true;
            workerLive_ = true;
            changed_.notify_all();
        }

        bool faulted = false;
        {
            std::unique_lock<std::mutex> lock(mutex_);
            for (;;) {
                changed_.wait(lock, [this] {
                    const bool canShutdown =
                        (lifecycle_ == Lifecycle::faulting
                            || lifecycle_ == Lifecycle::closing)
                        && setupComplete_;
                    const bool canRetry = lifecycle_ == Lifecycle::open
                        && !eventQueue_.empty();
                    return canShutdown || canRetry;
                });

                if ((lifecycle_ == Lifecycle::faulting
                        || lifecycle_ == Lifecycle::closing)
                    && setupComplete_) {
                    faulted = lifecycle_ == Lifecycle::faulting;
                    eventQueue_.clear();
                    queuedEventBytes_ = 0;
                    break;
                }

                QueuedEvent& event = eventQueue_.front();
                int32_t status = LOGOS_INSPECTOR_EVENT_REJECTED;
                try {
                    status = ingest_(
                        core_,
                        event.module.c_str(),
                        event.event.c_str(),
                        event.argsJson.c_str());
                } catch (...) {
                    status = LOGOS_INSPECTOR_EVENT_REJECTED;
                }
                if (status == LOGOS_INSPECTOR_EVENT_ACCEPTED) {
                    queuedEventBytes_ = event.retainedBytes <= queuedEventBytes_
                        ? queuedEventBytes_ - event.retainedBytes
                        : 0;
                    eventQueue_.pop_front();
                    continue;
                }
                if (status != LOGOS_INSPECTOR_EVENT_BACKPRESSURE) {
                    requestFaultLocked();
                    continue;
                }
                changed_.wait_for(lock, limits_.retryDelay, [this] {
                    return lifecycle_ != Lifecycle::open;
                });
            }
        }

        if (faulted) {
            failPendingAfterFault();
        }
        finishWorker();
    }

    void teardownProtocolAfterWorker() noexcept
    {
        std::vector<std::unique_ptr<SubscriptionRecord>> subscriptions;
        std::vector<ClientRecord> clients;
        try {
            {
                std::unique_lock<std::mutex> lock(mutex_);
                if (lifecycle_ == Lifecycle::closed) {
                    return;
                }
                if (teardownStarted_) {
                    waitWithOwnerEventPumpingLocked(lock, [this] {
                        return lifecycle_ == Lifecycle::closed;
                    });
                    return;
                }
                teardownStarted_ = true;
                waitWithOwnerEventPumpingLocked(lock, [this] {
                    return activeInvokes_ == 0;
                });
                subscriptions = std::move(subscriptions_);
                clients = std::move(clients_);
                eventQueue_.clear();
                queuedEventBytes_ = 0;
            }

            quiesceProtocol(subscriptions, clients);

            std::lock_guard<std::mutex> lock(mutex_);
            pending_.clear();
            retainedRequestBytes_ = 0;
            eventQueue_.clear();
            queuedEventBytes_ = 0;
            ownsEvents_ = false;
            workerLive_ = false;
            activationInProgress_ = false;
            lifecycle_ = Lifecycle::closed;
            changed_.notify_all();
        } catch (...) {
            try {
                std::lock_guard<std::mutex> lock(mutex_);
                ownsEvents_ = false;
                activationInProgress_ = false;
                lifecycle_ = Lifecycle::closed;
                changed_.notify_all();
            } catch (...) {
            }
        }
    }

    template<typename Predicate>
    void waitWithOwnerEventPumpingLocked(
        std::unique_lock<std::mutex>& lock,
        Predicate completed)
    {
        if (completed()) {
            return;
        }

        const bool isProtocolOwner = protocolOwnerThreadAssigned_
            && protocolOwnerThread_ == std::this_thread::get_id();
        const bool hasOwnerEventDispatcher = QCoreApplication::instance() != nullptr
            && QAbstractEventDispatcher::instance(QThread::currentThread()) != nullptr;
        if (!isProtocolOwner || !hasOwnerEventDispatcher) {
            changed_.wait(lock, completed);
            return;
        }

        while (!completed()) {
            lock.unlock();
            bool eventPumpFailed = false;
            try {
                QCoreApplication::processEvents(
                    QEventLoop::AllEvents,
                    kOwnerEventPumpSliceMs);
            } catch (...) {
                eventPumpFailed = true;
            }
            lock.lock();

            if (eventPumpFailed) {
                changed_.wait(lock, completed);
                return;
            }
            if (!completed()) {
                changed_.wait_for(lock, kOwnerEventPumpPause, completed);
            }
        }
    }

    void quiesceProtocol(
        std::vector<std::unique_ptr<SubscriptionRecord>>& subscriptions,
        std::vector<ClientRecord>& clients) noexcept
    {
        quiesceSubscriptions(subscriptions);
        for (const ClientRecord& client : clients) {
            if (client.handle == nullptr) {
                continue;
            }
            try {
                api_.clientDestroy(client.handle);
            } catch (...) {
            }
        }
        clients.clear();
    }

    void quiesceSubscriptions(
        std::vector<std::unique_ptr<SubscriptionRecord>>& subscriptions) noexcept
    {
        for (const auto& subscription : subscriptions) {
            if (subscription->handle == nullptr) {
                continue;
            }
            try {
                api_.unsubscribe(subscription->handle);
            } catch (...) {
            }
        }
        subscriptions.clear();
    }

    void failPendingAfterFault() noexcept
    {
        for (;;) {
            ReplyAction action;
            bool foundReply = false;
            try {
                std::lock_guard<std::mutex> lock(mutex_);
                for (auto& [requestId, request] : pending_) {
                    static_cast<void>(requestId);
                    if (request->cancelled || request->terminal) {
                        continue;
                    }
                    action.reply = request->reply;
                    action.context = request->replyContext;
                    action.requestId = request->requestId;
                    action.ok = 0;
                    action.staticPayload = kFaultError;
                    request->terminal = true;
                    foundReply = true;
                    break;
                }
            } catch (...) {
                return;
            }
            if (!foundReply) {
                return;
            }
            invokeReply(action);
        }
    }

    void finishWorker() noexcept
    {
        try {
            std::lock_guard<std::mutex> lock(mutex_);
            eventQueue_.clear();
            queuedEventBytes_ = 0;
            ownsEvents_ = false;
            workerLive_ = false;
            changed_.notify_all();
        } catch (...) {
        }
    }

    void emergencyClose() noexcept
    {
        bool faulted = false;
        try {
            {
                std::lock_guard<std::mutex> lock(mutex_);
                ownsEvents_ = false;
                if (lifecycle_ == Lifecycle::activating
                    || lifecycle_ == Lifecycle::open) {
                    lifecycle_ = Lifecycle::faulting;
                }
                faulted = lifecycle_ == Lifecycle::faulting;
                eventQueue_.clear();
                queuedEventBytes_ = 0;
                changed_.notify_all();
            }
            if (faulted) {
                failPendingAfterFault();
            }
            finishWorker();
        } catch (...) {
        }
    }

    void joinWorker() noexcept
    {
        try {
            std::lock_guard<std::mutex> joinLock(joinMutex_);
            if (!worker_.joinable()) {
                return;
            }
            if (worker_.get_id() == std::this_thread::get_id()) {
                return;
            }
            worker_.join();
        } catch (...) {
        }
    }

    LogosProtocolApi api_;
    LogosProtocolHostTransportLimits limits_;
    mutable std::mutex mutex_;
    std::mutex joinMutex_;
    std::condition_variable changed_;
    Lifecycle lifecycle_ = Lifecycle::dormant;
    LogosInspectorCore* core_ = nullptr;
    IngestModuleEventFn ingest_ = nullptr;
    bool setupComplete_ = true;
    bool activationInProgress_ = false;
    bool workerLive_ = false;
    bool workerThreadAssigned_ = false;
    bool protocolOwnerThreadAssigned_ = false;
    bool ownsEvents_ = false;
    bool teardownStarted_ = false;
    std::size_t activeInvokes_ = 0;
    std::size_t retainedRequestBytes_ = 0;
    std::size_t queuedEventBytes_ = 0;
    std::vector<ClientRecord> clients_;
    std::vector<std::unique_ptr<SubscriptionRecord>> subscriptions_;
    std::unordered_map<uint64_t, std::unique_ptr<PendingRequest>> pending_;
    std::deque<QueuedEvent> eventQueue_;
    std::thread worker_;
    std::thread::id workerThread_;
    std::thread::id protocolOwnerThread_;
};

LogosProtocolHostTransport::LogosProtocolHostTransport()
    : LogosProtocolHostTransport(
          LogosProtocolApi::production(),
          LogosProtocolHostTransportLimits {})
{
}

LogosProtocolHostTransport::LogosProtocolHostTransport(
    LogosProtocolApi protocolApi,
    LogosProtocolHostTransportLimits limits)
    : impl_(std::make_unique<Impl>(protocolApi, limits))
{
}

LogosProtocolHostTransport::~LogosProtocolHostTransport() = default;

bool LogosProtocolHostTransport::bindCore(
    LogosInspectorCore* core,
    IngestModuleEventFn ingest) noexcept
{
    return impl_->bindCore(core, ingest);
}

bool LogosProtocolHostTransport::activate() noexcept
{
    return impl_->activate();
}

LogosInspectorHostTransportV1 LogosProtocolHostTransport::vtable() noexcept
{
    return impl_->vtable();
}

bool LogosProtocolHostTransport::ownsRuntimeModuleEvents() const noexcept
{
    return impl_->ownsRuntimeModuleEvents();
}

void LogosProtocolHostTransport::close() noexcept
{
    impl_->close();
}
