#ifndef LOGOS_TRANSPORT_FACTORY_H
#define LOGOS_TRANSPORT_FACTORY_H

#include "logos_transport_config.h"

#include <memory>
#include <QString>

class LogosTransportHost;
class LogosTransportConnection;

namespace LogosTransportFactory {

    /**
     * @brief Create a transport host for `cfg`, honoring the process-wide
     * LogosMode.
     *
     * Resolution rule:
     *   - LogosMode::Mock                 → MockTransportHost   (cfg ignored)
     *   - LogosMode::Local                → LocalTransportHost  (cfg ignored)
     *   - LogosMode::Remote + LocalSocket → RemoteTransportHost (QRO)
     *   - LogosMode::Remote + Tcp/TcpSsl  → PlainTransportHost(cfg)
     *
     * Mode wins over `cfg.protocol` so test fixtures that switch the
     * process into Mock/Local always get the test transport, regardless
     * of which overload (or which LogosAPIProvider constructor) was
     * used. In Remote mode, `cfg` chooses the wire protocol and
     * carries the bind/dial address + TLS material.
     */
    std::unique_ptr<LogosTransportHost>
        createHost(const LogosTransportConfig& cfg,
                   const QString& registryUrl);

    /**
     * @brief Convenience: createHost using the process-global default
     * LogosTransportConfig. Equivalent to
     * `createHost(LogosTransportConfigGlobal::getDefault(), registryUrl)`.
     */
    std::unique_ptr<LogosTransportHost> createHost(const QString& registryUrl);

    /**
     * @brief Create a transport connection for `cfg`, honoring the
     * process-wide LogosMode. Same resolution rule as createHost — see
     * its doc-comment for the full table.
     */
    std::unique_ptr<LogosTransportConnection>
        createConnection(const LogosTransportConfig& cfg,
                         const QString& registryUrl);

    /**
     * @brief Convenience: createConnection using the process-global default
     * LogosTransportConfig. Equivalent to
     * `createConnection(LogosTransportConfigGlobal::getDefault(), registryUrl)`.
     */
    std::unique_ptr<LogosTransportConnection> createConnection(const QString& registryUrl);

}

#endif // LOGOS_TRANSPORT_FACTORY_H
