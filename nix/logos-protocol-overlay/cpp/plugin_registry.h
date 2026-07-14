#ifndef PLUGIN_REGISTRY_H
#define PLUGIN_REGISTRY_H

#include <QMap>
#include <QString>
#include <QObject>
#include <QCoreApplication>
#include <QVariant>
#include <QDebug>
#include <QMutex>
#include <QMutexLocker>

/**
 * @brief PluginRegistry provides in-process plugin registration for Local mode
 * 
 * This namespace provides functions to register, unregister, and retrieve plugins
 * using QCoreApplication properties. This is used as an alternative to QRemoteObjects
 * for mobile apps that run in a single process.
 */
namespace PluginRegistry {

    // Mutex for thread-safe access to the registry
    inline QMutex& registryMutex() {
        static QMutex mutex;
        return mutex;
    }

    // Prefix for plugin keys to avoid conflicts with other properties
    inline QString pluginKeyPrefix() {
        return QStringLiteral("logos_plugin_");
    }

    /**
     * @brief Convert a plugin name to a standardized key
     * @param name The plugin name
     * @return QString The standardized key
     */
    inline QString toPluginKey(const QString& name) {
        return pluginKeyPrefix() + name.toLower().replace(" ", "_");
    }

    /**
     * @brief Register a plugin in the application properties
     * @param plugin The plugin object to register
     * @param name The name to register the plugin under
     */
    inline void registerPlugin(QObject* plugin, const QString& name) {
        if (!plugin) {
            qWarning() << "PluginRegistry: Cannot register null plugin";
            return;
        }
        if (name.isEmpty()) {
            qWarning() << "PluginRegistry: Cannot register plugin with empty name";
            return;
        }

        QMutexLocker locker(&registryMutex());
        QString pluginKey = toPluginKey(name);
        QCoreApplication::instance()->setProperty(pluginKey.toUtf8().constData(), QVariant::fromValue(plugin));
        qDebug() << "PluginRegistry: Registered plugin with key:" << pluginKey;
    }

    /**
     * @brief Unregister a plugin from the application properties
     * @param name The name of the plugin to unregister
     * @return bool true if successful
     */
    inline bool unregisterPlugin(const QString& name) {
        if (name.isEmpty()) {
            qWarning() << "PluginRegistry: Cannot unregister plugin with empty name";
            return false;
        }

        QMutexLocker locker(&registryMutex());
        QString pluginKey = toPluginKey(name);
        QCoreApplication::instance()->setProperty(pluginKey.toUtf8().constData(), QVariant());
        qDebug() << "PluginRegistry: Unregistered plugin with key:" << pluginKey;
        return true;
    }

    /**
     * @brief Get a plugin by name with automatic casting to the requested type
     * @param name The name of the plugin to retrieve
     * @return T* Pointer to the plugin, or nullptr if not found
     */
    template<typename T>
    inline T* getPlugin(const QString& name) {
        if (name.isEmpty()) {
            qWarning() << "PluginRegistry: Cannot get plugin with empty name";
            return nullptr;
        }

        QMutexLocker locker(&registryMutex());
        QString pluginKey = toPluginKey(name);
        QVariant pluginVariant = QCoreApplication::instance()->property(pluginKey.toUtf8().constData());
        
        if (pluginVariant.isValid()) {
            QObject* obj = pluginVariant.value<QObject*>();
            if (obj) {
                T* result = qobject_cast<T*>(obj);
                if (result) {
                    qDebug() << "PluginRegistry: Found plugin:" << name;
                    return result;
                }
            }
        }
        
        qDebug() << "PluginRegistry: Plugin not found:" << name;
        return nullptr;
    }

    /**
     * @brief Check if a plugin is registered
     * @param name The name of the plugin to check
     * @return bool true if the plugin is registered
     */
    inline bool hasPlugin(const QString& name) {
        if (name.isEmpty()) {
            return false;
        }

        QMutexLocker locker(&registryMutex());
        QString pluginKey = toPluginKey(name);
        QVariant pluginVariant = QCoreApplication::instance()->property(pluginKey.toUtf8().constData());
        return pluginVariant.isValid() && pluginVariant.value<QObject*>() != nullptr;
    }

}

#endif // PLUGIN_REGISTRY_H
