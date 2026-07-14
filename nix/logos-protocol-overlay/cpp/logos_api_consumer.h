#ifndef LOGOS_API_CONSUMER_H
#define LOGOS_API_CONSUMER_H

#include <QObject>
#include <QString>
#include <QVariant>
#include <QVariantList>
#include <QHash>
#include <QMap>
#include <functional>
#include <memory>
#include <string>

#include "logos_call_error.h"
#include "logos_async_outcome.h"
#include "logos_mode.h"
#include "logos_transport_config.h"

class LogosTransportConnection;
class LogosObject;
class TokenManager;

/**
 * @brief LogosAPIConsumer handles connecting to module objects and invoking their methods
 * 
 * This class is responsible for the consumer/client side functionality:
 * - Connecting to module registries via the transport layer
 * - Requesting LogosObject handles
 * - Invoking methods on objects
 * - Handling events from objects
 */
class LogosAPIConsumer : public QObject
{
    Q_OBJECT

public:
    /**
     * @brief Construct a consumer connected via `transport`, honoring the
     * process-wide LogosMode.
     *
     * Transport resolution is done in one place — LogosTransportFactory —
     * by combining LogosMode + the supplied LogosTransportConfig:
     *   - LogosMode::Mock  → MockTransportConnection  (transport ignored)
     *   - LogosMode::Local → LocalTransportConnection (transport ignored)
     *   - LogosMode::Remote → wire protocol picked by `transport.protocol`
     *
     * Use this overload when the caller wants a specific transport for
     * this consumer without side-effecting the rest of the process
     * (e.g. the logoscore CLI dialing `core_service` over tcp_ssl
     * without also flipping the in-process LogosAPIProvider into
     * binding TLS).
     */
    LogosAPIConsumer(const QString& module_to_talk_to,
                     const QString& origin_module,
                     TokenManager* token_manager,
                     const LogosTransportConfig& transport,
                     QObject *parent = nullptr);

    /**
     * @brief Convenience constructor that uses the process-global default
     * LogosTransportConfig. Equivalent to passing
     * `LogosTransportConfigGlobal::getDefault()` to the explicit-transport
     * constructor above.
     */
    explicit LogosAPIConsumer(const QString& module_to_talk_to,
                              const QString& origin_module,
                              TokenManager* token_manager,
                              QObject *parent = nullptr);
    ~LogosAPIConsumer();

    /**
     * @brief Request a LogosObject handle by name
     * @return LogosObject* handle, or nullptr if failed. Caller must call release() when done.
     */
    LogosObject* requestObject(const QString& objectName, Timeout timeout = Timeout());

    bool isConnected() const;
    QString registryUrl() const;
    bool reconnect();

    QVariant invokeRemoteMethod(const QString& authToken, const QString& objectName, const QString& methodName,
                             const QVariantList& args = QVariantList(), Timeout timeout = Timeout());

    /**
     * @brief invokeRemoteMethod with an explicit error out-channel.
     *
     * Fills *err with the canonical {code, message, origin} call error when
     * the failure is detectable on this side (today: "object_unavailable"
     * when the target object/replica cannot be acquired). On success *err is
     * cleared. Failures the transport cannot yet distinguish from a void
     * result (per-dispatch errors) leave *err clear — the struct is the
     * extension point for surfacing transport-level statuses later.
     */
    QVariant invokeRemoteMethod(const QString& authToken, const QString& objectName, const QString& methodName,
                             const QVariantList& args, Timeout timeout, logos::CallError* err);

    using AsyncResultCallback = std::function<void(QVariant)>;

    /**
     * @brief Invoke a remote method asynchronously; result is delivered via callback
     * @param authToken Authentication token for the operation
     * @param objectName The name of the remote object
     * @param methodName The name of the method to call
     * @param args Arguments to pass to the method
     * @param callback Called when the call completes (on the caller's thread via QueuedConnection)
     * @param timeout Timeout for replica acquisition and for the remote call
     */
    void invokeRemoteMethodAsync(const QString& authToken, const QString& objectName, const QString& methodName,
                                 const QVariantList& args,
                                 AsyncResultCallback callback,
                                 Timeout timeout = Timeout());

    using AsyncOutcomeCallback = std::function<void(logos::AsyncCallOutcome)>;

    void invokeRemoteMethodAsyncOutcome(const QString& authToken,
                                        const QString& objectName,
                                        const QString& methodName,
                                        const QVariantList& args,
                                        AsyncOutcomeCallback callback,
                                        Timeout timeout = Timeout());

    /**
     * @brief Register an event listener via LogosObject's callback mechanism
     * @param originObject The LogosObject that will emit the event
     * @param eventName The name of the event to listen for
     * @param callback Function to call when the event is triggered
     */
    void onEvent(LogosObject* originObject, const QString& eventName, 
                std::function<void(const QString&, const QVariantList&)> callback);

public slots:
    bool informModuleToken(const QString& authToken, const QString& moduleName, const QString& token);
    bool informModuleToken_module(const QString& authToken, const QString& originModule, const QString& moduleName, const QString& token);
    std::string requestModule(const std::string& authToken, const std::string& originModule, const std::string& targetModule);

private:
    std::unique_ptr<LogosTransportConnection> m_transport;
    QString m_registryUrl;
    QMap<QString, QString> m_tokens;
    TokenManager* m_token_manager;
};

#endif // LOGOS_API_CONSUMER_H
