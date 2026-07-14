#ifndef MODULE_PROXY_H
#define MODULE_PROXY_H

#include <QObject>
#include <QVariant>
#include <QVariantList>
#include <QHash>
#include <QString>
#include <QJsonArray>

#include "logos_async_outcome.h"

class LogosProviderObject;

/**
 * @brief ModuleProxy wraps a LogosProviderObject and exposes it as a QObject
 *        so that Qt Remote Objects can publish it.
 *
 * All method dispatch, introspection, and event forwarding is delegated
 * to the underlying LogosProviderObject*.  For legacy QObject-based plugins,
 * that provider is a QtProviderObject adapter; for new-API plugins it is
 * the plugin's own LogosProviderObject subclass.
 */
class ModuleProxy : public QObject
{
    Q_OBJECT

public:
    explicit ModuleProxy(LogosProviderObject* provider, QObject* parent = nullptr);
    ~ModuleProxy();

    Q_INVOKABLE QVariant callRemoteMethod(const QString& authToken, const QString& methodName, const QVariantList& args = QVariantList());
    logos::AsyncCallOutcome callRemoteMethodOutcome(
        const QString& authToken,
        const QString& methodName,
        const QVariantList& args = QVariantList());
    Q_INVOKABLE bool informModuleToken(const QString& authToken, const QString& moduleName, const QString& token);
    bool saveToken(const QString& from_module_name, const QString& token);
    // getPluginInterface() returns the module's whole interface (methods AND
    // events, each tagged with a "type"); getPluginMethods()/getPluginEvents()
    // are the type-filtered views. All three derive from the provider's single
    // getMethods() call — there is no separate getEvents() vtable method, which
    // is what keeps the provider ABI stable across SDK versions.
    Q_INVOKABLE QJsonArray getPluginMethods();
    Q_INVOKABLE QJsonArray getPluginEvents();
    Q_INVOKABLE QJsonArray getPluginInterface();

signals:
    void eventResponse(const QString& eventName, const QVariantList& data);

private:
    // Returns true only when authToken matches a token THIS module has issued
    // (via saveToken / informModuleToken). Empty/unknown tokens are rejected.
    // Comparison is constant-time and never early-outs, so neither a correct
    // prefix nor the number of issued tokens leaks through timing.
    bool isAuthorized(const QString& authToken) const;

    LogosProviderObject* m_provider;
    QHash<QString, QString> m_tokens;
};

#endif // MODULE_PROXY_H
