#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct LogosInspectorCore LogosInspectorCore;

#define LOGOS_INSPECTOR_HOST_TRANSPORT_ABI_VERSION 1u
#define LOGOS_INSPECTOR_EVENT_REJECTED 0
#define LOGOS_INSPECTOR_EVENT_ACCEPTED 1
#define LOGOS_INSPECTOR_EVENT_BACKPRESSURE -1

typedef void (*LogosInspectorCoreReplyFn)(
    void* context,
    uint64_t bridge_request_id,
    const char* response_json);

typedef void (*LogosInspectorHostReplyFn)(
    void* context,
    uint64_t module_request_id,
    int32_t ok,
    const char* payload_json);

typedef int32_t (*LogosInspectorHostDispatchFn)(
    void* host_context,
    uint64_t module_request_id,
    const char* module,
    const char* method,
    const char* args_json,
    LogosInspectorHostReplyFn reply,
    void* reply_context);

typedef void (*LogosInspectorHostCancelFn)(
    void* host_context,
    uint64_t module_request_id);

typedef void (*LogosInspectorHostCloseFn)(void* host_context);

typedef struct LogosInspectorHostTransportV1 {
    uint32_t abi_version;
    uint32_t struct_size;
    void* context;
    LogosInspectorHostDispatchFn dispatch;
    LogosInspectorHostCancelFn cancel;
    LogosInspectorHostCloseFn close;
} LogosInspectorHostTransportV1;

/*
 * Runtime event-health signaling is an additive function, not a field in
 * LogosInspectorHostTransportV1. Version 1 size and layout remain unchanged.
 */

LogosInspectorCore* logos_inspector_core_new(void);

/*
 * Creates a provider-neutral asynchronous host bridge. The vtable is copied;
 * its context remains caller-owned and must stay valid until close returns.
 * dispatch and close are required; cancel is optional. Constructor failure
 * retains caller ownership and does not invoke close.
 * If cancel is provided, dispatch and cancel may run on concurrent threads and
 * the context must support both. cancel may run when an accepted module-call
 * future is abandoned before reply; bridge ingress cancellation remains local
 * and uncorrelated.
 * Dispatch borrows its strings only for the call; queued hosts must copy them.
 *
 * Returning 1 accepts a request. Unless cancel is invoked or host close begins,
 * the host must issue exactly one reply carrying the same module_request_id.
 * Returning 0 rejects it and forbids a reply. The reply context is borrowed,
 * must be passed back unchanged, and is valid until host close returns. After
 * cancel or close begins, the host may suppress its reply or deliver at most
 * one late reply; either outcome is safe.
 *
 * Reply callbacks may run on any thread and may overlap dispatch, cancel, and
 * host close. Before host close returns, every in-progress reply callback and
 * host-initiated module-event ingress call must have returned, and neither may
 * begin later. payload_json is a borrowed NUL-terminated UTF-8 string for the
 * reply call. ok == 1 requires valid JSON, including literal null; other values
 * report an error payload. No C or C++ callback may throw or unwind across this
 * ABI boundary.
 */
LogosInspectorCore* logos_inspector_core_new_with_host_transport(
    const LogosInspectorHostTransportV1* transport);

/*
 * Publishes native runtime module-event ownership to the Rust transport.
 * Health starts false. Set ready to 1 only while the native adapter owns the
 * complete subscription catalog, bounded retry policy, and quiescent shutdown
 * contract. Set 0 as soon as that ownership faults. Local Rust subscription
 * registration is not upstream event-delivery evidence.
 *
 * Returns 1 when an open asynchronous host-backed core accepts the update.
 * Returns 0 for invalid values, null/synchronous handles, or once close begins.
 * This function may race ordinary calls and event ingress while the allocation
 * remains live. Join any race with close before free.
 */
int32_t logos_inspector_core_set_runtime_module_event_health(
    LogosInspectorCore* handle,
    int32_t ready);

/*
 * Close is idempotent and may race asynchronous call/cancel entry points and
 * logos_inspector_core_call on a host-transport handle. The allocation must
 * remain live until every racing call and close has returned. Do not call
 * close or free reentrantly from a core reply or host transport callback.
 * Free must not race any ABI call or callback and invokes close before
 * releasing the handle. The caller must join every racing call and close
 * before invoking free. A core reply callback already selected by a concurrent
 * cancel may still be running after close returns; join that call before free.
 */
void logos_inspector_core_close(LogosInspectorCore* handle);
void logos_inspector_core_free(LogosInspectorCore* handle);

/*
 * Handles created with host transport accept this entry point only for
 * explicitly catalogued synchronous logos_inspector methods. Accepted calls
 * copy their inputs, enter the same bounded worker and bridge instance as
 * asynchronous calls, and block until that worker returns the response. They
 * never invoke host dispatch. Operation commands, Tokio- or
 * module-transport-backed runtime methods, Zone Catalog/L2 commands,
 * callModule, and unknown methods return an async-required error before
 * enqueue. A full worker queue returns a backpressure error. Closing handles
 * return a closed error. This call may race close while the allocation remains
 * live; join both calls before free.
 *
 * A call made reentrantly on the bridge worker from a core or host callback
 * returns a reentrant-call error instead of blocking that worker.
 */
char* logos_inspector_core_call(
    LogosInspectorCore* handle,
    const char* method,
    const char* args_json);

/*
 * Handles created with host transport reject every synchronous module call
 * with an async-required error and never invoke host dispatch.
 */
char* logos_inspector_core_call_module(
    LogosInspectorCore* handle,
    const char* module,
    const char* method,
    const char* args_json);

/*
 * Copies all inputs. Return 1 transfers reply_context and guarantees exactly
 * one callback, including cancellation or close. The callback may race this
 * function's return and may run on any thread. Return 0 leaves reply_context
 * caller-owned and guarantees no callback. response_json is borrowed for the
 * duration of the callback. A callback may reenter non-closing ABI functions.
 * bridge_request_id must be nonzero and unique while pending; it may be reused
 * after its terminal callback returns.
 */
int32_t logos_inspector_core_call_module_async(
    LogosInspectorCore* handle,
    uint64_t bridge_request_id,
    const char* module,
    const char* method,
    const char* args_json,
    LogosInspectorCoreReplyFn reply,
    void* reply_context);

/*
 * Copies and queues one host module event without waiting for Rust processing.
 * args_json must be a valid JSON array representing the callback arguments.
 * Returns EVENT_ACCEPTED when validated and queued before close began,
 * EVENT_REJECTED for invalid input or a closing handle, and
 * EVENT_BACKPRESSURE when the bounded ingress queue is full. Accepted events
 * share FIFO worker ordering with bridge calls. This function may run on any
 * thread and may race call, cancel, or close. The host must quiesce these calls
 * before its close callback returns.
 *
 * The core does not retry. On BACKPRESSURE, a native event owner must copy and
 * retry the event without blocking the host callback, or begin host shutdown.
 * It may compact an unaccepted homogeneous backlog of
 * blockchain_module:newBlock events to its newest event; that event is a
 * current-state observation, not a Runtime Operation completion. All other
 * events must be retained because dropping one could strand an accepted Runtime
 * Operation. A host must not advertise native event ownership to QML until it
 * implements that policy.
 */
int32_t logos_inspector_core_ingest_module_event(
    LogosInspectorCore* handle,
    const char* module,
    const char* event,
    const char* args_json);

/*
 * Return 1 claims a pending bridge request and selects cancellation as its one
 * terminal callback. Return 0 means unknown or already terminal. Cancellation
 * is cooperative: underlying host work may finish later and is then ignored.
 */
int32_t logos_inspector_core_cancel(
    LogosInspectorCore* handle,
    uint64_t bridge_request_id);

void logos_inspector_core_string_free(char* value);

#ifdef __cplusplus
}
#endif
