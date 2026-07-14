#ifndef LOGOS_TRANSPORT_CONFIG_JSON_H
#define LOGOS_TRANSPORT_CONFIG_JSON_H

#include "logos_transport_config.h"

#include <string>

// JSON (de)serialization for LogosTransportSet — used to pass per-module
// transport configuration across the parent → child subprocess boundary
// when the daemon launches `logos_host_qt` for capability_module (and
// any other module that needs a non-default transport set). Kept
// separate from logos_transport_config.h so consumers that only want
// the value type don't pay for the JSON parse / nlohmann include.
//
// Wire shape mirrors daemon.json's per-module transport list:
//   [
//     {"protocol": "local"},
//     {"protocol": "tcp", "host": "127.0.0.1", "port": 6001, "codec": "json"},
//     {"protocol": "tcp_ssl", "host": "0.0.0.0", "port": 6443,
//      "codec": "cbor", "ca_file": "/p/ca", "cert_file": "/p/c",
//      "key_file": "/p/k", "verify_peer": true}
//   ]
//
// Cert/key paths *are* included on the wire here (unlike the on-disk
// daemon.json connection file): this serialization is for the parent
// telling the child "here's the cert you should bind your TLS
// listener with", which is the one place those paths legitimately
// need to flow over IPC.

namespace logos {

// Serialize a LogosTransportSet to a single-line JSON string suitable
// for passing as a CLI argument or environment variable to a child
// process. Returns "[]" for an empty set.
std::string transportSetToJsonString(const LogosTransportSet& set);

// Inverse of transportSetToJsonString. Returns an empty set on parse
// error (and leaves diagnostics to the caller — pass through the
// command-line parser's existing error machinery).
LogosTransportSet transportSetFromJsonString(const std::string& json);

} // namespace logos

#endif // LOGOS_TRANSPORT_CONFIG_JSON_H
