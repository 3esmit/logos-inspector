#ifndef LOGOS_CALL_ERROR_H
#define LOGOS_CALL_ERROR_H

// The canonical cross-module call error — the C++ face of the protocol's
// {code, message, origin} error JSON (see makeErrorJson / lp_invoke's
// out_error_json). Deliberately Qt-free: it crosses into Qt-free module
// code (generated typed wrappers expose it as an optional out-parameter,
// e.g. `calc.add(a, b, &err)`), so only std types appear here.

#include <string>

namespace logos {

// Error codes are lowercase snake_case strings, mirroring the C ABI's JSON
// contract rather than an enum so the set can grow (transport-level codes,
// provider dispatch errors) without an ABI break.
//
// Canonical infrastructure codes currently include object_unavailable,
// invoke_failed, transport_error, transport_closed, timeout, and
// capability_unavailable. Provider/domain error codes may extend this set.
struct CallError {
    std::string code;     // empty = no error
    std::string message;
    std::string origin;   // module the error originated from / was detected for

    bool ok() const { return code.empty(); }
    void clear() { code.clear(); message.clear(); origin.clear(); }
};

} // namespace logos

#endif // LOGOS_CALL_ERROR_H
