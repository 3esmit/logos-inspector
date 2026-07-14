#ifndef LOGOS_REGISTRY_FACTORY_H
#define LOGOS_REGISTRY_FACTORY_H

#include <memory>
#include <QString>

class LogosRegistry;

namespace LogosRegistryFactory {

    /**
     * @brief Create a registry appropriate for the current LogosMode.
     *
     * In Remote mode a QtRemoteRegistry is created, binding a
     * QRemoteObjectRegistryHost to the given @p url.
     *
     * In Local (in-process) mode a NullRegistry is created — no IPC
     * endpoint is needed because all modules share the same process.
     *
     * @param url  The address to bind the registry to (ignored in Local mode).
     * @return     Owning pointer to the new registry.
     */
    std::unique_ptr<LogosRegistry> create(const QString& url);

}

#endif // LOGOS_REGISTRY_FACTORY_H
