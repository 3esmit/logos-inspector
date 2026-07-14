#ifndef LOGOS_TRANSPORT_H
#define LOGOS_TRANSPORT_H

#include <QString>
#include <QVariant>
#include <QVariantList>

class QObject;
class LogosObject;

/**
 * @brief Abstract interface for the provider/server side of module transport.
 *
 * Implementations handle how a module object is made available to consumers
 * (e.g. via in-process registry, Qt Remote Objects, or other mechanisms).
 */
class LogosTransportHost {
public:
    virtual ~LogosTransportHost() = default;

    /**
     * @brief Publish an object so consumers can discover and invoke it
     * @param name The name to publish the object under
     * @param object The QObject to publish (provider side remains Qt-based)
     * @return true if publishing succeeded
     */
    virtual bool publishObject(const QString& name, QObject* object) = 0;

    /**
     * @brief Remove a previously published object
     * @param name The name the object was published under
     */
    virtual void unpublishObject(const QString& name) = 0;

    /**
     * @brief URL this backend will (or does) listen on for the named object.
     *
     * Used by LogosAPIProvider so it doesn't need to bake the URL scheme
     * into LogosInstance — each backend owns its own addressing.
     *
     * Default implementation returns the deterministic local-socket URL
     * ("local:logos_<mod>_<instance>") for back-compat with existing
     * qt_remote code; network-transport backends override.
     */
    virtual QString bindUrl(const QString& instanceId,
                            const QString& moduleName);
};

/**
 * @brief Abstract interface for the consumer/client side of module transport.
 *
 * Implementations handle how a consumer connects to a host and acquires
 * LogosObject handles.  Method invocation, event subscription, and lifecycle
 * management are handled by LogosObject itself.
 */
class LogosTransportConnection {
public:
    virtual ~LogosTransportConnection() = default;

    /**
     * @brief Establish connection to the host/registry
     * @return true if connection succeeded
     */
    virtual bool connectToHost() = 0;

    /**
     * @brief Check if currently connected
     */
    virtual bool isConnected() const = 0;

    /**
     * @brief Tear down and re-establish the connection
     * @return true if reconnection succeeded
     */
    virtual bool reconnect() = 0;

    /**
     * @brief Acquire a LogosObject handle to a named object from the host.
     *
     * The returned LogosObject encapsulates method invocation, event
     * handling and lifecycle.  Call LogosObject::release() when done.
     *
     * @param objectName The published name of the object
     * @param timeoutMs Maximum time to wait for the object to become available
     * @return LogosObject* handle, or nullptr on failure.
     */
    virtual LogosObject* requestObject(const QString& objectName, int timeoutMs) = 0;

    /**
     * @brief URL this backend expects to connect to for the named object.
     *
     * Mirror of LogosTransportHost::bindUrl on the consumer side. Default
     * returns the deterministic local-socket URL; network-transport
     * backends override.
     */
    virtual QString endpointUrl(const QString& instanceId,
                                const QString& moduleName);
};

#endif // LOGOS_TRANSPORT_H
