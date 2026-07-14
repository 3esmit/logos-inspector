#ifndef LOGOS_TRANSPORT_CONFIG_H
#define LOGOS_TRANSPORT_CONFIG_H

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

// -----------------------------------------------------------------------------
// LogosTransportConfig — plain C++ value describing one transport endpoint.
//
// Intentionally Qt-free: the SDK is being de-Qt'd and new config types should
// not pull QtCore into consumers. See the transport backend implementations
// in cpp/implementations/ for how each protocol uses these fields.
// -----------------------------------------------------------------------------

enum class LogosProtocol {
    LocalSocket,   // QLocalSocket via QRemoteObjects (existing code path)
    Tcp,           // Plain TCP (Boost.Asio + JSON framing)
    TcpSsl,        // TCP + TLS (Boost.Asio + OpenSSL + JSON framing)
    // Noise, Quic — future work
};

enum class LogosWireCodec {
    Json,   // nlohmann::json::dump / parse   (default for now)
    Cbor,   // nlohmann::json::to_cbor / from_cbor (future)
};

struct LogosTransportConfig {
    LogosProtocol protocol = LogosProtocol::LocalSocket;

    // Tcp / TcpSsl bind address on the daemon side.
    // On the client side this is the address to connect to; clients may
    // override via --tcp-host to, e.g., reach a container bound to 0.0.0.0
    // as "localhost" from the host.
    std::string host = "127.0.0.1";

    // 0 = let the daemon pick; the chosen port is written into the endpoint
    // file so clients can find it.
    uint16_t port = 0;

    // TcpSsl only.
    std::string caFile;
    std::string certFile;
    std::string keyFile;
    bool verifyPeer = true;

    // Wire-format codec used for RPC framing on this transport. Only
    // meaningful for plain-C++ transports (Tcp / TcpSsl); LocalSocket
    // ignores it and uses QRemoteObjects' own wire format.
    LogosWireCodec codec = LogosWireCodec::Json;
};

// Field-wise equality. Used as the equality predicate for hashed
// containers keyed by LogosTransportConfig (e.g. the explicit-transport
// LogosAPIClient cache in logos_api.h). Every field that can plausibly
// distinguish one transport-attached client from another belongs here —
// missing one would let two callers with different security or codec
// settings alias onto the same cached client.
inline bool operator==(const LogosTransportConfig& a,
                       const LogosTransportConfig& b) noexcept
{
    return a.protocol   == b.protocol
        && a.port       == b.port
        && a.verifyPeer == b.verifyPeer
        && a.codec      == b.codec
        && a.host       == b.host
        && a.caFile     == b.caFile
        && a.certFile   == b.certFile
        && a.keyFile    == b.keyFile;
}

inline bool operator!=(const LogosTransportConfig& a,
                       const LogosTransportConfig& b) noexcept
{
    return !(a == b);
}

using LogosTransportSet = std::vector<LogosTransportConfig>;

// -----------------------------------------------------------------------------
// Process-global default.
//
// Set once at startup (before constructing any LogosAPI). LogosAPI /
// LogosAPIClient consult this when no per-instance override is passed.
// Modules launched by a daemon inherit the daemon's default.
// -----------------------------------------------------------------------------
namespace LogosTransportConfigGlobal {

    inline LogosTransportConfig& defaultStorage() {
        static LogosTransportConfig cfg{};   // LocalSocket
        return cfg;
    }

    inline const LogosTransportConfig& getDefault() {
        return defaultStorage();
    }

    inline void setDefault(LogosTransportConfig cfg) {
        defaultStorage() = std::move(cfg);
    }

}

#endif // LOGOS_TRANSPORT_CONFIG_H
