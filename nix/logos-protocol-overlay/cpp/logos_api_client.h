#ifndef LOGOS_API_CLIENT_H
#define LOGOS_API_CLIENT_H

#include <QObject>
#include <QString>
#include <QVariant>
#include <QVariantList>
#include <QMap>
#include <functional>
#include <string>
#include <vector>

#include "logos_call_error.h"
#include "logos_async_outcome.h"
#include "logos_mode.h"
#include "logos_transport_config.h"
#include <nlohmann/json.hpp>

class LogosAPI;
class LogosAPIConsumer;
class LogosObject;
class TokenManager;

/**
 * @brief LogosAPIClient provides a high-level interface for remote method calls
 * 
 * This class serves as a facade over LogosAPIConsumer, providing a clean interface
 * for applications that need to call remote methods and handle events.
 */
class LogosAPIClient : public QObject
{
    Q_OBJECT

public:
    /**
     * @brief Construct a client with explicit transports for both the
     * target module *and* `capability_module`.
     *
     * Two transports because the SDK's auto-`requestModule` flow inside
     * invokeRemoteMethod{,Async} dials `capability_module` to fetch a
     * per-target token. When the daemon advertises capability_module on
     * a different transport from the target (e.g. CLI on host →
     * core_service over TCP, but capability_module also over TCP on a
     * sibling port), the auto-dial must use the right one. Pre-building
     * the consumer once in the constructor (see m_capability_consumer)
     * keeps the hot path free of per-call lookups.
     */
    LogosAPIClient(const QString& module_to_talk_to,
                   const QString& origin_module,
                   TokenManager* token_manager,
                   const LogosTransportConfig& target_transport,
                   const LogosTransportConfig& capability_transport,
                   QObject *parent = nullptr);

    /**
     * @brief No-transport constructor — both target and
     * capability_module use the process-global default
     * (LocalSocket) via LogosTransportConfigGlobal::getDefault().
     */
    explicit LogosAPIClient(const QString& module_to_talk_to,
                            const QString& origin_module,
                            TokenManager* token_manager,
                            QObject *parent = nullptr);
    ~LogosAPIClient();

    /**
     * @brief Request a LogosObject handle by name
     * @return LogosObject* handle, or nullptr if failed
     */
    LogosObject* requestObject(const QString& objectName, Timeout timeout = Timeout());

    bool isConnected() const;
    QString registryUrl() const;
    bool reconnect();

    QVariant invokeRemoteMethod(const QString& objectName, const QString& methodName,
                             const QVariantList& args = QVariantList(), Timeout timeout = Timeout());

    /**
     * @brief invokeRemoteMethod with an explicit error out-channel.
     *
     * Fills *err with the canonical {code, message, origin} call error when
     * the failure is detectable (today: "object_unavailable" when the target
     * object cannot be acquired); cleared on success. Generated typed client
     * wrappers call this overload and throw logos::LogosCallError so callers
     * can distinguish a failed call from a legitimately default-valued
     * result.
     */
    QVariant invokeRemoteMethod(const QString& objectName, const QString& methodName,
                             const QVariantList& args, Timeout timeout, logos::CallError* err);

    QVariant invokeRemoteMethod(const QString& objectName, const QString& methodName,
                             const QVariant& arg, Timeout timeout = Timeout());

    QVariant invokeRemoteMethod(const QString& objectName, const QString& methodName, 
                             const QVariant& arg1, const QVariant& arg2, Timeout timeout = Timeout());

    QVariant invokeRemoteMethod(const QString& objectName, const QString& methodName, 
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3, Timeout timeout = Timeout());

    QVariant invokeRemoteMethod(const QString& objectName, const QString& methodName, 
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3, 
                             const QVariant& arg4, Timeout timeout = Timeout());

    QVariant invokeRemoteMethod(const QString& objectName, const QString& methodName,
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3,
                             const QVariant& arg4, const QVariant& arg5, Timeout timeout = Timeout());

    // const char* overloads — resolve ambiguity for string literals
    QVariant invokeRemoteMethod(const char* objectName, const char* methodName,
                             const QVariantList& args = QVariantList(), Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString(objectName), QString(methodName), args, timeout); }

    QVariant invokeRemoteMethod(const char* objectName, const char* methodName,
                             const QVariant& arg, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString(objectName), QString(methodName), arg, timeout); }

    QVariant invokeRemoteMethod(const char* objectName, const char* methodName,
                             const QVariant& arg1, const QVariant& arg2, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString(objectName), QString(methodName), arg1, arg2, timeout); }

    QVariant invokeRemoteMethod(const char* objectName, const char* methodName,
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString(objectName), QString(methodName), arg1, arg2, arg3, timeout); }

    QVariant invokeRemoteMethod(const char* objectName, const char* methodName,
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3,
                             const QVariant& arg4, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString(objectName), QString(methodName), arg1, arg2, arg3, arg4, timeout); }

    QVariant invokeRemoteMethod(const char* objectName, const char* methodName,
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3,
                             const QVariant& arg4, const QVariant& arg5, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString(objectName), QString(methodName), arg1, arg2, arg3, arg4, arg5, timeout); }

    // std::string overloads — thin wrappers that convert internally
    QVariant invokeRemoteMethod(const std::string& objectName, const std::string& methodName,
                             const QVariantList& args = QVariantList(), Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString::fromStdString(objectName), QString::fromStdString(methodName), args, timeout); }

    QVariant invokeRemoteMethod(const std::string& objectName, const std::string& methodName,
                             const QVariant& arg, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString::fromStdString(objectName), QString::fromStdString(methodName), arg, timeout); }

    QVariant invokeRemoteMethod(const std::string& objectName, const std::string& methodName,
                             const QVariant& arg1, const QVariant& arg2, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString::fromStdString(objectName), QString::fromStdString(methodName), arg1, arg2, timeout); }

    QVariant invokeRemoteMethod(const std::string& objectName, const std::string& methodName,
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString::fromStdString(objectName), QString::fromStdString(methodName), arg1, arg2, arg3, timeout); }

    QVariant invokeRemoteMethod(const std::string& objectName, const std::string& methodName,
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3,
                             const QVariant& arg4, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString::fromStdString(objectName), QString::fromStdString(methodName), arg1, arg2, arg3, arg4, timeout); }

    QVariant invokeRemoteMethod(const std::string& objectName, const std::string& methodName,
                             const QVariant& arg1, const QVariant& arg2, const QVariant& arg3,
                             const QVariant& arg4, const QVariant& arg5, Timeout timeout = Timeout())
    { return invokeRemoteMethod(QString::fromStdString(objectName), QString::fromStdString(methodName), arg1, arg2, arg3, arg4, arg5, timeout); }

    // const char* onEvent overload
    void onEvent(LogosObject* originObject, const char* eventName,
                std::function<void(const QString&, const QVariantList&)> callback)
    { onEvent(originObject, QString(eventName), callback); }

    // std::string onEvent overloads
    void onEvent(LogosObject* originObject, const std::string& eventName,
                std::function<void(const QString&, const QVariantList&)> callback)
    { onEvent(originObject, QString::fromStdString(eventName), callback); }

    void onEvent(LogosObject* originObject, const std::string& eventName,
                std::function<void(const std::string&, const QVariantList&)> callback)
    {
        onEvent(originObject, QString::fromStdString(eventName),
            [cb = std::move(callback)](const QString& name, const QVariantList& args) {
                cb(name.toStdString(), args);
            });
    }

    void onEvent(LogosObject* originObject, const char* eventName,
                std::function<void(const std::string&, const QVariantList&)> callback)
    {
        onEvent(originObject, QString(eventName),
            [cb = std::move(callback)](const QString& name, const QVariantList& args) {
                cb(name.toStdString(), args);
            });
    }

    using AsyncResultCallback = std::function<void(QVariant)>;
    using AsyncOutcomeCallback = std::function<void(logos::AsyncCallOutcome)>;

    void invokeRemoteMethodAsyncOutcome(const QString& objectName,
                                        const QString& methodName,
                                        const QVariantList& args,
                                        AsyncOutcomeCallback callback,
                                        Timeout timeout = Timeout());

    void invokeRemoteMethodAsync(const QString& objectName, const QString& methodName,
                                 const QVariantList& args, AsyncResultCallback callback,
                                 Timeout timeout = Timeout());

    void invokeRemoteMethodAsync(const QString& objectName, const QString& methodName,
                                 const QVariant& arg, AsyncResultCallback callback,
                                 Timeout timeout = Timeout());

    void invokeRemoteMethodAsync(const QString& objectName, const QString& methodName,
                                 const QVariant& arg1, const QVariant& arg2,
                                 AsyncResultCallback callback, Timeout timeout = Timeout());

    void invokeRemoteMethodAsync(const QString& objectName, const QString& methodName,
                                 const QVariant& arg1, const QVariant& arg2, const QVariant& arg3,
                                 AsyncResultCallback callback, Timeout timeout = Timeout());

    void invokeRemoteMethodAsync(const QString& objectName, const QString& methodName,
                                 const QVariant& arg1, const QVariant& arg2, const QVariant& arg3,
                                 const QVariant& arg4, AsyncResultCallback callback,
                                 Timeout timeout = Timeout());

    void invokeRemoteMethodAsync(const QString& objectName, const QString& methodName,
                                 const QVariant& arg1, const QVariant& arg2, const QVariant& arg3,
                                 const QVariant& arg4, const QVariant& arg5,
                                 AsyncResultCallback callback, Timeout timeout = Timeout());

    /**
     * @brief Register an event listener via LogosObject's callback mechanism
     * @param originObject The LogosObject that will emit the event
     * @param eventName The name of the event to listen for
     * @param callback Function to call when the event is triggered
     */
    void onEvent(LogosObject* originObject, const QString& eventName, 
                std::function<void(const QString&, const QVariantList&)> callback);

    /**
     * @brief Emit an event on a LogosObject (for plugins that act as event sources)
     * @param object The LogosObject to emit the event on
     * @param eventName The name of the event
     * @param data The event data
     */
    void onEventResponse(LogosObject* object, const QString& eventName, const QVariantList& data);

    /**
     * @brief Backward-compatible overload for QObject-based plugins.
     *
     * Old-API plugins call onEventResponse(this, ...) where `this` is a QObject*.
     * This overload invokes the eventResponse signal on the QObject via QMetaObject.
     */
    void onEventResponse(QObject* object, const QString& eventName, const QVariantList& data);

    bool informModuleToken(const QString& authToken, const QString& moduleName, const QString& token);
    bool informModuleToken(const char* authToken, const char* moduleName, const char* token)
        { return informModuleToken(QString(authToken), QString(moduleName), QString(token)); }
    bool informModuleToken(const std::string& authToken, const std::string& moduleName, const std::string& token);
    bool informModuleToken_module(const QString& authToken, const QString& originModule, const QString& moduleName, const QString& token);

    TokenManager* getTokenManager() const;
    QString getToken(const QString& module_name);

    // nlohmann::json overloads — args is a JSON array, result is a JSON value.
    // These convert between nlohmann::json and QVariant internally so callers
    // never need to touch Qt JSON types.
    nlohmann::json invokeRemoteMethod(const std::string& objectName,
                                      const std::string& methodName,
                                      const nlohmann::json& args,
                                      Timeout timeout = Timeout());

    // nlohmann::json event callback overload — data arrives as a json array.
    void onEvent(LogosObject* originObject, const std::string& eventName,
                 std::function<void(const std::string&, const nlohmann::json&)> callback);

private:
    // ABI note: this private layout is consumed by every plugin that
    // statically links libsdk. Adding a new field in the middle of
    // this section shifts the offsets of subsequent fields and
    // SILENTLY breaks any plugin compiled before the change — it
    // reads m_token_manager at the wrong offset and segfaults on
    // the first cross-process call. New private members MUST be
    // appended to the end. (Long-term cure: pimpl this class so
    // sizeof / offsets become opaque to consumers.)
    LogosAPIConsumer* m_consumer;
    QMap<QString, QString> m_tokens;
    TokenManager* m_token_manager;
    QString m_origin_module;
    // Pre-built consumer for the auto-`requestModule` token-fetch path
    // in invokeRemoteMethod{,Async}. Constructed once with the right
    // transport (see the two-transport ctor) so the hot path doesn't
    // chase a back-pointer to LogosAPI just to look up the transport
    // registry. Null only when `m_consumer` itself is for
    // capability_module (no recursion). In-class default to nullptr
    // so any old constructor that doesn't list this field still
    // leaves a defined value.
    LogosAPIConsumer* m_capability_consumer = nullptr;

    // Per-target queue of continuations waiting on an in-flight async
    // requestModule handshake. The FIRST async call to an un-tokened target
    // starts exactly one handshake; concurrent calls to the same target queue
    // here and all drain with the single minted token when it resolves. Without
    // this coalescing a fan-out of N first-calls fires N racing handshakes whose
    // distinct tokens overwrite each other on the target — so already-dispatched
    // calls carry a superseded token and get rejected. Touched only on the
    // owner thread (invokeRemoteMethodAsync marshals there), so it needs no
    // lock. Appended last per the ABI note above; defaults to empty.
    using PendingHandshakeCall =
        std::function<void(const QString&, const logos::CallError&)>;
    QMap<QString, std::vector<PendingHandshakeCall>> m_pendingHandshakes;
};

#endif // LOGOS_API_CLIENT_H
