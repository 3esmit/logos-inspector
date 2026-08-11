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
constexpr std::string_view kBlockchainModule = "blockchain_module";
constexpr std::string_view kNewBlockEvent = "newBlock";
constexpr std::size_t kMaxIdentifierBytes = 256;
constexpr char kFaultError[] =
    R"({"code":"transport_closed","message":"native module event ingress failed; host transport closed","origin":"logos_inspector"})";
constexpr char kPayloadRetentionError[] =
    R"({"code":"invoke_failed","message":"host transport could not retain module result","origin":"logos_inspector"})";
constexpr int kOwnerEventPumpSliceMs = 10;
constexpr auto kOwnerEventPumpPause = std::chrono::milliseconds(1);

constexpr std::array<std::string_view, 7> kModules = {
    "blockchain_module",
    "storage_module",
    "delivery_module",
    "capability_module",
    "core_service",
    "lez_indexer_module",
    "lez_core",
};

struct EventSpec
{
    std::string_view module;
    std::string_view event;
    bool required = true;
};

constexpr std::array<EventSpec, 22> kEvents = { {
    { "delivery_module", "messageSent" },
    { "delivery_module", "messageError" },
    { "delivery_module", "messagePropagated" },
    { "delivery_module", "messageReceived" },
    { "delivery_module", "connectionStateChanged" },
    { "delivery_module", "nodeStarted" },
    { "delivery_module", "nodeStopped" },
    { "delivery_module", "nodeChanged", false },
    { "storage_module", "storageStart" },
    { "storage_module", "storageStop" },
    { "storage_module", "storageConnect" },
    { "storage_module", "storageUploadProgress" },
    { "storage_module", "storageUploadDone" },
    { "storage_module", "storageDownloadProgress" },
    { "storage_module", "storageDownloadDone" },
    { "storage_module", "storageDownloadProgressV2" },
    { "storage_module", "storageDownloadDoneV2" },
    { "storage_module", "storageDownloadManifestDone" },
    { "storage_module", "storageRemoveDone" },
    { kBlockchainModule, kNewBlockEvent },
    { kBlockchainModule, "nodeChanged", false },
    { "storage_module", "nodeChanged", false },
} };

bool isLatestBlockEvent(std::string_view module, std::string_view event) noexcept
{
    return module == kBlockchainModule && event == kNewBlockEvent;
}

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
    api.clientCreateInstance = &lp_client_create_instance;
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

    bool bindCore(
        LogosInspectorCore* core,
        IngestModuleEventFn ingest,
        IngestModuleInstanceEventFn ingestInstance,
        SetRuntimeModuleEventHealthFn setEventHealth) noexcept
    {
        if (core == nullptr || ingest == nullptr || setEventHealth == nullptr) {
            return false;
        }
        try {
            std::lock_guard<std::mutex> lock(mutex_);
            if (lifecycle_ != Lifecycle::dormant || core_ != nullptr || ingest_ != nullptr
                || ingestInstance_ != nullptr || setEventHealth_ != nullptr) {
                return false;
            }
            core_ = core;
            ingest_ = ingest;
            ingestInstance_ = ingestInstance;
            setEventHealth_ = setEventHealth;
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
                    || setEventHealth_ == nullptr || !validApi() || !validLimits()) {
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
            bool opened = false;
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
                    opened = true;
                }
            }
            if (opened) {
                if (!publishActivatedEventHealth(eventCatalogComplete)) {
                    requestFaultFromCallback();
                    close();
                    return false;
                }
                return true;
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

    LogosInspectorHostTransportV2 vtableV2() noexcept
    {
        LogosInspectorHostTransportV2 result {};
        result.v1 = vtable();
        if (api_.clientCreateInstance == nullptr) {
            return result;
        }
        result.v1.abi_version = LOGOS_INSPECTOR_HOST_TRANSPORT_ABI_VERSION_V2;
        result.v1.struct_size = static_cast<uint32_t>(sizeof(result));
        result.dispatch_instance = &dispatchInstanceCallback;
        result.subscribe_instance = &subscribeInstanceCallback;
        result.unsubscribe_instance = &unsubscribeInstanceCallback;
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
                    static_cast<void>(publishEventHealth(false));
                    lifecycle_ = Lifecycle::closed;
                    setupComplete_ = true;
                    ownsEvents_ = false;
                    changed_.notify_all();
                    return;
                case Lifecycle::activating:
                case Lifecycle::open:
                    static_cast<void>(publishEventHealth(false));
                    lifecycle_ = Lifecycle::closing;
                    ownsEvents_ = false;
                    suppressPendingLocked();
                    changed_.notify_all();
                    break;
                case Lifecycle::faulting:
                    static_cast<void>(publishEventHealth(false));
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
        std::string instanceId;
        lp_client* handle = nullptr;
        std::size_t references = 0;
    };

    struct SubscriptionRecord
    {
        Impl* owner = nullptr;
        std::string module;
        std::string instanceId;
        std::string event;
        lp_subscription* handle = nullptr;
        lp_client* client = nullptr;
        std::size_t references = 1;
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
        bool scopedClientReference = false;
        bool invoking = true;
        bool callbackFinished = false;
        bool cancelled = false;
        bool terminal = false;
    };

    struct QueuedEvent
    {
        std::string module;
        std::string instanceId;
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
                nullptr,
                method,
                argsJson,
                reply,
                replyContext);
        } catch (...) {
            return 0;
        }
    }

    static int32_t dispatchInstanceCallback(
        void* context,
        uint64_t moduleRequestId,
        const char* module,
        const char* instanceId,
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
                instanceId,
                method,
                argsJson,
                reply,
                replyContext);
        } catch (...) {
            return 0;
        }
    }

    static int32_t subscribeInstanceCallback(
        void* context,
        const char* module,
        const char* instanceId,
        const char* event) noexcept
    {
        if (context == nullptr) {
            return 0;
        }
        try {
            return static_cast<Impl*>(context)->subscribeInstance(module, instanceId, event)
                ? 1
                : 0;
        } catch (...) {
            return 0;
        }
    }

    static int32_t unsubscribeInstanceCallback(
        void* context,
        const char* module,
        const char* instanceId,
        const char* event) noexcept
    {
        if (context == nullptr) {
            return 0;
        }
        try {
            return static_cast<Impl*>(context)->unsubscribeInstance(module, instanceId, event)
                ? 1
                : 0;
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
            && limits_.maxScopedClients > 0 && limits_.maxScopedSubscriptions > 0
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
                clients_.push_back(ClientRecord { std::string(module), {}, handle });
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
                client = clientForModuleLocked(event.module, {});
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
                if (lifecycle_ != Lifecycle::activating) {
                    return false;
                }
                if (handle == nullptr) {
                    if (event.required) {
                        return false;
                    }
                    if (subscriptions_.empty() || subscriptions_.back().get() != rawRecord) {
                        return false;
                    }
                    subscriptions_.pop_back();
                    continue;
                }
                rawRecord->handle = handle;
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

    lp_client* clientForModuleLocked(
        std::string_view module,
        std::string_view instanceId) const noexcept
    {
        for (const ClientRecord& client : clients_) {
            if (client.module == module && client.instanceId == instanceId) {
                return client.handle;
            }
        }
        return nullptr;
    }

    lp_client* acquireScopedClient(std::string_view module, std::string_view instanceId)
    {
        if (instanceId.empty() || api_.clientCreateInstance == nullptr) {
            return nullptr;
        }
        std::lock_guard<std::mutex> creationLock(clientCreationMutex_);
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (lifecycle_ != Lifecycle::open || !workerLive_) {
                return nullptr;
            }
            if (lp_client* const existing = clientForModuleLocked(module, instanceId)) {
                for (ClientRecord& client : clients_) {
                    if (client.handle == existing) {
                        ++client.references;
                        break;
                    }
                }
                return existing;
            }
            std::size_t scopedClients = 0;
            for (const ClientRecord& client : clients_) {
                scopedClients += !client.instanceId.empty() ? 1U : 0U;
            }
            if (scopedClients >= limits_.maxScopedClients) {
                return nullptr;
            }
        }

        const std::string moduleText(module);
        const std::string instanceText(instanceId);
        lp_client* const handle = api_.clientCreateInstance(
            moduleText.c_str(),
            instanceText.c_str(),
            kOriginModule.data(),
            nullptr,
            nullptr);
        if (handle == nullptr) {
            return nullptr;
        }

        bool retain = false;
        try {
            std::lock_guard<std::mutex> lock(mutex_);
            if (lifecycle_ == Lifecycle::open && workerLive_) {
                clients_.push_back(ClientRecord { moduleText, instanceText, handle, 1 });
                retain = true;
            }
        } catch (...) {
        }
        if (!retain) {
            try {
                api_.clientDestroy(handle);
            } catch (...) {
            }
            return nullptr;
        }
        return handle;
    }

    void releaseScopedClient(lp_client* handle) noexcept
    {
        if (handle == nullptr) {
            return;
        }
        lp_client* destroy = nullptr;
        try {
            {
                std::lock_guard<std::mutex> creationLock(clientCreationMutex_);
                std::lock_guard<std::mutex> lock(mutex_);
                const auto found = std::find_if(
                    clients_.begin(),
                    clients_.end(),
                    [handle](const ClientRecord& client) {
                        return client.handle == handle && !client.instanceId.empty();
                    });
                if (found == clients_.end()) {
                    return;
                }
                if (found->references > 0) {
                    --found->references;
                }
                if (found->references != 0) {
                    return;
                }
                const bool referencedBySubscription = std::any_of(
                    subscriptions_.begin(),
                    subscriptions_.end(),
                    [handle](const std::unique_ptr<SubscriptionRecord>& subscription) {
                        return subscription->client == handle && subscription->references > 0;
                    });
                if (referencedBySubscription) {
                    return;
                }
                destroy = found->handle;
                clients_.erase(found);
            }
            if (destroy != nullptr) {
                api_.clientDestroy(destroy);
            }
        } catch (...) {
        }
    }

    bool subscribeInstance(
        const char* moduleValue,
        const char* instanceValue,
        const char* eventValue)
    {
        std::size_t moduleLength = 0;
        std::size_t instanceLength = 0;
        std::size_t eventLength = 0;
        if (!boundedCStringLength(moduleValue, kMaxIdentifierBytes, moduleLength)
            || moduleLength == 0
            || !boundedCStringLength(instanceValue, kMaxIdentifierBytes, instanceLength)
            || instanceLength == 0
            || !boundedCStringLength(eventValue, kMaxIdentifierBytes, eventLength)
            || eventLength == 0) {
            return false;
        }
        const std::string_view module(moduleValue, moduleLength);
        const std::string_view instanceId(instanceValue, instanceLength);
        const std::string_view event(eventValue, eventLength);
        if (!allowedModule(module)
            || module != "lez_indexer_module" || event != "nodeChanged") {
            return false;
        }

        std::lock_guard<std::mutex> creationLock(subscriptionCreationMutex_);
        bool existingSubscription = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (lifecycle_ != Lifecycle::open || !workerLive_ || ingestInstance_ == nullptr) {
                return false;
            }
            for (const auto& subscription : subscriptions_) {
                if (subscription->module == module && subscription->instanceId == instanceId
                    && subscription->event == event) {
                    existingSubscription = true;
                    break;
                }
            }
            if (!existingSubscription) {
                std::size_t scopedSubscriptions = 0;
                for (const auto& subscription : subscriptions_) {
                    scopedSubscriptions += !subscription->instanceId.empty() ? 1U : 0U;
                }
                if (scopedSubscriptions >= limits_.maxScopedSubscriptions) {
                    return false;
                }
            }
        }

        lp_client* const client = acquireScopedClient(module, instanceId);
        if (client == nullptr) {
            return false;
        }

        if (existingSubscription) {
            bool retained = false;
            {
                std::lock_guard<std::mutex> lock(mutex_);
                const auto found = std::find_if(
                    subscriptions_.begin(),
                    subscriptions_.end(),
                    [&module, &instanceId, &event](
                        const std::unique_ptr<SubscriptionRecord>& entry) {
                        return entry->module == module && entry->instanceId == instanceId
                            && entry->event == event;
                    });
                if (found != subscriptions_.end() && lifecycle_ == Lifecycle::open
                    && workerLive_) {
                    ++(*found)->references;
                    retained = true;
                }
            }
            if (retained) {
                return true;
            }
            releaseScopedClient(client);
            return false;
        }

        auto record = std::make_unique<SubscriptionRecord>();
        record->owner = this;
        record->module.assign(module);
        record->instanceId.assign(instanceId);
        record->event.assign(event);
        record->client = client;
        SubscriptionRecord* const rawRecord = record.get();
        bool retainRecord = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (lifecycle_ == Lifecycle::open && workerLive_ && ingestInstance_ != nullptr) {
                subscriptions_.push_back(std::move(record));
                retainRecord = true;
            }
        }
        if (!retainRecord) {
            releaseScopedClient(client);
            return false;
        }

        lp_subscription* handle = nullptr;
        try {
            handle = api_.subscribe(
                client,
                rawRecord->event.c_str(),
                &eventCallback,
                rawRecord);
        } catch (...) {
            handle = nullptr;
        }

        bool retained = false;
        std::unique_ptr<SubscriptionRecord> removedRecord;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            const auto found = std::find_if(
                subscriptions_.begin(),
                subscriptions_.end(),
                [rawRecord](const std::unique_ptr<SubscriptionRecord>& entry) {
                    return entry.get() == rawRecord;
                });
            if (found != subscriptions_.end() && lifecycle_ == Lifecycle::open && workerLive_
                && handle != nullptr) {
                rawRecord->handle = handle;
                retained = true;
            } else if (found != subscriptions_.end()) {
                removedRecord = std::move(*found);
                subscriptions_.erase(found);
            }
        }
        if (!retained && handle != nullptr) {
            try {
                api_.unsubscribe(handle);
            } catch (...) {
            }
        }
        if (!retained) {
            releaseScopedClient(client);
        }
        return retained;
    }

    bool unsubscribeInstance(
        const char* moduleValue,
        const char* instanceValue,
        const char* eventValue)
    {
        std::size_t moduleLength = 0;
        std::size_t instanceLength = 0;
        std::size_t eventLength = 0;
        if (!boundedCStringLength(moduleValue, kMaxIdentifierBytes, moduleLength)
            || moduleLength == 0
            || !boundedCStringLength(instanceValue, kMaxIdentifierBytes, instanceLength)
            || instanceLength == 0
            || !boundedCStringLength(eventValue, kMaxIdentifierBytes, eventLength)
            || eventLength == 0) {
            return false;
        }
        const std::string_view module(moduleValue, moduleLength);
        const std::string_view instanceId(instanceValue, instanceLength);
        const std::string_view event(eventValue, eventLength);
        if (!allowedModule(module)
            || module != "lez_indexer_module" || event != "nodeChanged") {
            return false;
        }

        std::lock_guard<std::mutex> creationLock(subscriptionCreationMutex_);
        std::unique_ptr<SubscriptionRecord> removedRecord;
        lp_client* client = nullptr;
        lp_subscription* handle = nullptr;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            const auto found = std::find_if(
                subscriptions_.begin(),
                subscriptions_.end(),
                [&module, &instanceId, &event](
                    const std::unique_ptr<SubscriptionRecord>& entry) {
                    return entry->module == module && entry->instanceId == instanceId
                        && entry->event == event;
                });
            if (found == subscriptions_.end()) {
                return true;
            }
            SubscriptionRecord& subscription = *(*found);
            client = subscription.client;
            if (subscription.references > 1) {
                --subscription.references;
            } else {
                handle = subscription.handle;
                removedRecord = std::move(*found);
                subscriptions_.erase(found);
            }
        }
        if (handle != nullptr) {
            try {
                api_.unsubscribe(handle);
            } catch (...) {
            }
        }
        // Keep the record alive until the native unsubscribe callback returns.
        removedRecord.reset();
        releaseScopedClient(client);
        return true;
    }

    int32_t dispatch(
        uint64_t requestId,
        const char* moduleValue,
        const char* instanceValue,
        const char* methodValue,
        const char* argsValue,
        LogosInspectorHostReplyFn reply,
        void* replyContext)
    {
        if (requestId == 0 || reply == nullptr) {
            return 0;
        }

        std::size_t moduleLength = 0;
        std::size_t instanceLength = 0;
        std::size_t methodLength = 0;
        std::size_t argsLength = 0;
        if (!boundedCStringLength(moduleValue, kMaxIdentifierBytes, moduleLength)
            || moduleLength == 0
            || (instanceValue != nullptr
                && (!boundedCStringLength(instanceValue, kMaxIdentifierBytes, instanceLength)
                    || instanceLength == 0))
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
        const std::string_view instanceView = instanceValue == nullptr
            ? std::string_view {}
            : std::string_view(instanceValue, instanceLength);

        std::size_t retainedBytes = 0;
        if (!checkedAdd(retainedBytes, moduleLength)
            || !checkedAdd(retainedBytes, instanceLength)
            || !checkedAdd(retainedBytes, methodLength)
            || !checkedAdd(retainedBytes, argsLength)
            || retainedBytes > limits_.maxSingleRequestBytes) {
            return 0;
        }

        lp_client* scopedClient = nullptr;
        if (!instanceView.empty()) {
            scopedClient = acquireScopedClient(moduleView, instanceView);
            if (scopedClient == nullptr) {
                return 0;
            }
        }

        std::unique_ptr<PendingRequest> request;
        try {
            request = std::make_unique<PendingRequest>();
            request->owner = this;
            request->requestId = requestId;
            request->reply = reply;
            request->replyContext = replyContext;
            request->module.assign(moduleValue, moduleLength);
            request->method.assign(methodValue, methodLength);
            request->argsJson.assign(argsValue, argsLength);
            request->retainedBytes = retainedBytes;
            request->scopedClientReference = !instanceView.empty();
        } catch (...) {
            releaseScopedClient(scopedClient);
            return 0;
        }
        PendingRequest* const rawRequest = request.get();

        bool admitted = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (lifecycle_ != Lifecycle::open || !workerLive_
                || pending_.size() >= limits_.maxPendingRequests
                || pending_.find(requestId) != pending_.end()
                || retainedBytes > limits_.maxRetainedRequestBytes - retainedRequestBytes_) {
                admitted = false;
            } else {
                request->client = instanceView.empty()
                    ? clientForModuleLocked(request->module, {})
                    : scopedClient;
                if (request->client != nullptr) {
                    pending_.emplace(requestId, std::move(request));
                    retainedRequestBytes_ += retainedBytes;
                    ++activeInvokes_;
                    admitted = true;
                }
            }
        }
        if (!admitted) {
            releaseScopedClient(scopedClient);
            return 0;
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
        lp_client* release = nullptr;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            const auto found = pending_.find(requestId);
            if (found == pending_.end()) {
                return;
            }
            found->second->cancelled = true;
            release = eraseFinishedRequestLocked(requestId, found->second.get());
        }
        releaseScopedClient(release);
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

        lp_client* release = nullptr;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            const auto found = pending_.find(request->requestId);
            if (found != pending_.end() && found->second.get() == request) {
                found->second->callbackFinished = true;
                release = eraseFinishedRequestLocked(request->requestId, request);
            }
            changed_.notify_all();
        }
        releaseScopedClient(release);
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
            lp_client* release = nullptr;
            {
                std::lock_guard<std::mutex> lock(mutex_);
                const auto found = pending_.find(requestId);
                if (found != pending_.end() && found->second.get() == request) {
                    found->second->invoking = false;
                }
                if (activeInvokes_ > 0) {
                    --activeInvokes_;
                }
                release = eraseFinishedRequestLocked(requestId, request);
                changed_.notify_all();
            }
            releaseScopedClient(release);
        } catch (...) {
        }
    }

    lp_client* eraseFinishedRequestLocked(uint64_t requestId, PendingRequest* expected)
    {
        const auto found = pending_.find(requestId);
        if (found == pending_.end() || found->second.get() != expected) {
            return nullptr;
        }
        const PendingRequest& request = *found->second;
        if (request.invoking || !request.callbackFinished
            || (!request.terminal && !request.cancelled)) {
            return nullptr;
        }
        lp_client* release = request.scopedClientReference ? request.client : nullptr;
        retainedRequestBytes_ = request.retainedBytes <= retainedRequestBytes_
            ? retainedRequestBytes_ - request.retainedBytes
            : 0;
        pending_.erase(found);
        return release;
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
        const bool validSize = valid
            && checkedAdd(retainedBytes, subscription->instanceId.size())
            && checkedAdd(retainedBytes, subscription->event.size())
            && checkedAdd(retainedBytes, dataLength)
            && retainedBytes <= limits_.maxSingleEventBytes;

        std::unique_lock<std::mutex> lock(mutex_);
        if ((lifecycle_ != Lifecycle::activating && lifecycle_ != Lifecycle::open)
            || (subscription->instanceId.empty() && !ownsEvents_)
            || (!subscription->instanceId.empty() && ingestInstance_ == nullptr)) {
            return;
        }
        if (!validSize) {
            requestFaultLocked();
            return;
        }

        QueuedEvent event;
        event.module = subscription->module;
        event.instanceId = subscription->instanceId;
        event.event = subscription->event;
        event.argsJson.assign(dataJson, dataLength);
        event.retainedBytes = retainedBytes;

        if (lifecycle_ == Lifecycle::activating || !eventQueue_.empty()) {
            enqueueEventLocked(std::move(event));
            return;
        }

        int32_t status = LOGOS_INSPECTOR_EVENT_REJECTED;
        try {
            status = ingestQueuedEvent(event);
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

    int32_t ingestQueuedEvent(const QueuedEvent& event) noexcept
    {
        try {
            if (!event.instanceId.empty()) {
                if (ingestInstance_ == nullptr) {
                    return LOGOS_INSPECTOR_EVENT_REJECTED;
                }
                return ingestInstance_(
                    core_,
                    event.module.c_str(),
                    event.instanceId.c_str(),
                    event.event.c_str(),
                    event.argsJson.c_str());
            }
            return ingest_(
                core_,
                event.module.c_str(),
                event.event.c_str(),
                event.argsJson.c_str());
        } catch (...) {
            return LOGOS_INSPECTOR_EVENT_REJECTED;
        }
    }

    void enqueueEventLocked(QueuedEvent event)
    {
        if (eventQueue_.size() >= limits_.maxQueuedEvents
            || event.retainedBytes > limits_.maxQueuedEventBytes - queuedEventBytes_) {
            if (coalesceQueuedBlockBurstLocked(event)) {
                return;
            }
            requestFaultLocked();
            return;
        }
        queuedEventBytes_ += event.retainedBytes;
        eventQueue_.push_back(std::move(event));
        changed_.notify_all();
    }

    bool coalesceQueuedBlockBurstLocked(QueuedEvent& event)
    {
        if (!event.instanceId.empty() || !isLatestBlockEvent(event.module, event.event)
            || eventQueue_.empty()) {
            return false;
        }
        if (queuedEventBytes_ > limits_.maxQueuedEventBytes) {
            return false;
        }
        for (const QueuedEvent& queued : eventQueue_) {
            if (!queued.instanceId.empty()
                || !isLatestBlockEvent(queued.module, queued.event)) {
                return false;
            }
        }

        // `newBlock` is a current-state observation rather than a terminal
        // operation result. Preserve the newest pending block while the Rust
        // ingress catches up, but never compact a mixed event backlog.
        eventQueue_.clear();
        queuedEventBytes_ = 0;
        try {
            eventQueue_.push_back(std::move(event));
        } catch (...) {
            requestFaultLocked();
            return true;
        }
        queuedEventBytes_ = eventQueue_.back().retainedBytes;
        changed_.notify_all();
        return true;
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
            static_cast<void>(publishEventHealth(false));
            changed_.notify_all();
        }
    }

    bool publishEventHealth(bool ready) noexcept
    {
        if (core_ == nullptr || setEventHealth_ == nullptr) {
            return false;
        }
        try {
            return setEventHealth_(core_, ready ? 1 : 0) == 1;
        } catch (...) {
            return false;
        }
    }

    bool publishActivatedEventHealth(bool ready) noexcept
    {
        const bool published = publishEventHealth(ready);
        bool stillOpen = false;
        try {
            std::lock_guard<std::mutex> lock(mutex_);
            stillOpen = lifecycle_ == Lifecycle::open && ownsEvents_ == ready && workerLive_;
        } catch (...) {
            stillOpen = false;
        }
        if (!published || !stillOpen) {
            static_cast<void>(publishEventHealth(false));
            return false;
        }
        return true;
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

                int32_t status = LOGOS_INSPECTOR_EVENT_REJECTED;
                {
                    const QueuedEvent& event = eventQueue_.front();
                    status = ingestQueuedEvent(event);
                }
                if (status == LOGOS_INSPECTOR_EVENT_ACCEPTED) {
                    const std::size_t retainedBytes = eventQueue_.front().retainedBytes;
                    queuedEventBytes_ = retainedBytes <= queuedEventBytes_
                        ? queuedEventBytes_ - retainedBytes
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
            bool waitForExistingTeardown = false;
            {
                // Scoped subscription creation takes these locks in this
                // order. Join in-flight creation before detaching records,
                // then release them before protocol teardown: unsubscribe and
                // destroy can synchronously marshal work back to the owner.
                std::lock_guard<std::mutex> subscriptionCreationLock(subscriptionCreationMutex_);
                std::lock_guard<std::mutex> clientCreationLock(clientCreationMutex_);
                std::lock_guard<std::mutex> lock(mutex_);
                if (lifecycle_ == Lifecycle::closed) {
                    return;
                }
                waitForExistingTeardown = teardownStarted_;
                teardownStarted_ = true;
            }
            if (waitForExistingTeardown) {
                std::unique_lock<std::mutex> lock(mutex_);
                waitWithOwnerEventPumpingLocked(lock, [this] {
                    return lifecycle_ == Lifecycle::closed;
                });
                return;
            }
            {
                std::unique_lock<std::mutex> lock(mutex_);
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
                static_cast<void>(publishEventHealth(false));
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
    std::mutex clientCreationMutex_;
    std::mutex subscriptionCreationMutex_;
    std::mutex joinMutex_;
    std::condition_variable changed_;
    Lifecycle lifecycle_ = Lifecycle::dormant;
    LogosInspectorCore* core_ = nullptr;
    IngestModuleEventFn ingest_ = nullptr;
    IngestModuleInstanceEventFn ingestInstance_ = nullptr;
    SetRuntimeModuleEventHealthFn setEventHealth_ = nullptr;
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
    IngestModuleEventFn ingest,
    SetRuntimeModuleEventHealthFn setEventHealth) noexcept
{
    return impl_->bindCore(core, ingest, nullptr, setEventHealth);
}

bool LogosProtocolHostTransport::bindCoreV2(
    LogosInspectorCore* core,
    IngestModuleEventFn ingest,
    IngestModuleInstanceEventFn ingestInstance,
    SetRuntimeModuleEventHealthFn setEventHealth) noexcept
{
    return impl_->bindCore(core, ingest, ingestInstance, setEventHealth);
}

bool LogosProtocolHostTransport::activate() noexcept
{
    return impl_->activate();
}

LogosInspectorHostTransportV1 LogosProtocolHostTransport::vtable() noexcept
{
    return impl_->vtable();
}

LogosInspectorHostTransportV2 LogosProtocolHostTransport::vtableV2() noexcept
{
    return impl_->vtableV2();
}

bool LogosProtocolHostTransport::ownsRuntimeModuleEvents() const noexcept
{
    return impl_->ownsRuntimeModuleEvents();
}

void LogosProtocolHostTransport::close() noexcept
{
    impl_->close();
}
