#ifndef LOGOS_OBJECT_H
#define LOGOS_OBJECT_H

#include <QString>
#include <QVariant>
#include <QVariantList>
#include <QJsonArray>
#include <functional>
#include <cstdint>

#include "logos_async_outcome.h"

/**
 * @brief Abstract interface for a module object handle.
 *
 * LogosObject decouples callers from the underlying transport mechanism.
 * Each transport (local/Qt Remote Objects/mock/JSON-RPC/...) provides its
 * own concrete subclass.  Callers interact exclusively through this
 * interface and never need to know the implementation type.
 */
class LogosObject {
public:
    virtual ~LogosObject() = default;

    /**
     * @brief Invoke a method on the remote/local module.
     * @param authToken Authentication token for the operation
     * @param methodName Method to call on the underlying module
     * @param args Arguments for the method
     * @param timeoutMs Maximum time to wait for the result
     * @return The method result, or an invalid QVariant on failure
     */
    virtual QVariant callMethod(const QString& authToken,
                                const QString& methodName,
                                const QVariantList& args,
                                int timeoutMs) = 0;

    using AsyncResultCallback = std::function<void(QVariant)>;
    using AsyncOutcomeCallback = std::function<void(logos::AsyncCallOutcome)>;

    /**
     * @brief Invoke a method asynchronously; result is delivered via callback.
     *
     * Returns immediately. The callback is never invoked synchronously inside
     * this call. Qt-backed transports use a subsequent event-loop iteration;
     * Qt-free transports may use an internal protocol worker thread.
     *
     * @param authToken Authentication token for the operation
     * @param methodName Method to call on the underlying module
     * @param args Arguments for the method
     * @param timeoutMs Maximum time to wait for the result
     * @param callback Called with the result (invalid QVariant on failure/timeout)
     */
    virtual void callMethodAsync(const QString& authToken,
                                 const QString& methodName,
                                 const QVariantList& args,
                                 int timeoutMs,
                                 AsyncResultCallback callback)
    {
        if (!callback) return;
        callMethodAsyncOutcome(authToken, methodName, args, timeoutMs,
            [callback = std::move(callback)](logos::AsyncCallOutcome outcome) mutable {
                callback(outcome.ok() ? std::move(outcome.value) : QVariant());
            });
    }

    /**
     * @brief Deliver a module token to the underlying module.
     * @param authToken Authentication token for the operation
     * @param moduleName Target module name
     * @param token The token to deliver
     * @param timeoutMs Maximum time to wait for the result
     * @return true if the token was delivered successfully
     */
    virtual bool informModuleToken(const QString& authToken,
                                   const QString& moduleName,
                                   const QString& token,
                                   int timeoutMs) = 0;

    using EventCallback = std::function<void(const QString&, const QVariantList&)>;

    /**
     * @brief Subscribe to events from this object.
     *
     * Qt-based implementations use QObject::connect internally;
     * other implementations may use a different mechanism.
     *
     * @param eventName The event name to listen for
     * @param callback  Called when the event fires
     */
    virtual void onEvent(const QString& eventName, EventCallback callback) = 0;

    /**
     * @brief Remove all event subscriptions made via onEvent().
     */
    virtual void disconnectEvents() = 0;

    /**
     * @brief Emit an event on this object.
     *
     * For Qt-based implementations this triggers the underlying
     * QObject signal so that Qt Remote Objects can replicate it.
     *
     * @param eventName The event name
     * @param data      Event payload
     */
    virtual void emitEvent(const QString& eventName, const QVariantList& data) = 0;

    /**
     * @brief Return introspection data for the methods exposed by
     *        the underlying module.
     */
    virtual QJsonArray getMethods() = 0;

    /**
     * @brief Release resources associated with this handle.
     *
     * After calling release() the object must not be used again.
     * Implementations that own the underlying resource (e.g. a
     * QRemoteObjectReplica) will delete it here.
     */
    virtual void release() = 0;

    /**
     * @brief Stable identity value suitable for use as a hash key.
     */
    virtual quintptr id() const = 0;

    /**
     * @brief Typed asynchronous invocation preserving success-null vs failure.
     *
     * Appended after the original virtual surface so existing method slots stay
     * stable. Legacy subclasses that only override callMethodAsync remain source
     * compatible; their QVariant result is treated as a successful outcome.
     */
    virtual void callMethodAsyncOutcome(const QString& authToken,
                                        const QString& methodName,
                                        const QVariantList& args,
                                        int timeoutMs,
                                        AsyncOutcomeCallback callback)
    {
        if (!callback) return;
        callMethodAsync(authToken, methodName, args, timeoutMs,
            [callback = std::move(callback)](QVariant value) mutable {
                callback(logos::AsyncCallOutcome::success(std::move(value)));
            });
    }
};

#endif // LOGOS_OBJECT_H
