#ifndef LOGOS_REGISTRY_H
#define LOGOS_REGISTRY_H

/**
 * @brief Abstract interface for a module registry endpoint.
 *
 * A registry is a well-known rendezvous point that modules advertise
 * themselves on (host/provider side) and that consumers discover them
 * through (client side).
 *
 * The interface is intentionally minimal: lifecycle management
 * (creation / destruction) is handled entirely by the implementation's
 * constructor and destructor.  Callers only need to check whether the
 * registry is up and running.
 *
 * Current implementations:
 *   - QtRemoteRegistry  — backed by QRemoteObjectRegistryHost (Remote mode)
 *   - NullRegistry      — no-op used in Local (in-process) mode
 */
class LogosRegistry {
public:
    virtual ~LogosRegistry() = default;

    /**
     * @brief Returns true if the registry endpoint has been successfully
     *        created and is ready to accept registrations.
     */
    virtual bool isInitialized() const = 0;
};

#endif // LOGOS_REGISTRY_H
