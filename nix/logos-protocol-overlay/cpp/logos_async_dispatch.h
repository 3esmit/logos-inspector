#ifndef LOGOS_ASYNC_DISPATCH_H
#define LOGOS_ASYNC_DISPATCH_H

#include <QString>

// Shared wire constants for "multi" (concurrent) dispatch. Concurrency is a
// MODULE-side concern handled entirely behind the ordinary callMethod entry
// point — no new provider/host vtable method, so the provider ABI is unchanged
// and an old host/daemon loads a "multi" module and forwards its traffic
// without even understanding these markers.
//
// A "multi" module's generated glue does NOT block in callMethod: it hands the
// handler to a worker and returns a PENDING SENTINEL immediately (a QVariantMap
// carrying the call id under pendingCallKey()). When the worker finishes, the
// module pushes the real result back as a COMPLETION event
// (callCompleteEvent(), data = [callId, result]) over the SAME event channel it
// already uses (setEventListener). The host (ModuleProxy / liblogos) is a pure
// forwarder — it returns whatever callMethod returned and forwards whatever
// events the module emits. The CONSUMER transport (RemoteLogosObject for QtRO,
// PlainLogosObject for the plain transport) detects the sentinel, waits for the
// matching completion keyed by callId, and returns the real result — so
// generated clients call transparently. Both transports use this path; the
// version that speaks it is logos-protocol 0.2 (additive minor — see
// logos_protocol.h).
namespace logos {

inline QString pendingCallKey()   { return QStringLiteral("__logos_pending_call__"); }
inline QString callCompleteEvent() { return QStringLiteral("__logos_call_complete__"); }

}  // namespace logos

#endif  // LOGOS_ASYNC_DISPATCH_H
