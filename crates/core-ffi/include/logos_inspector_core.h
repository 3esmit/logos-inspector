#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct LogosInspectorCore LogosInspectorCore;

#define LOGOS_INSPECTOR_HOST_TRANSPORT_ABI_VERSION 1u

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
 * Returning 1 accepts a request and requires exactly one reply carrying the
 * same module_request_id. Returning 0 rejects it and forbids a reply. The
 * reply context is borrowed, must be passed back unchanged, and is valid until
 * host close returns. After optional cancel is invoked, the host may suppress
 * its reply or deliver at most one late reply; either outcome is safe.
 *
 * Reply callbacks may run on any thread and may overlap dispatch, cancel, and
 * host close. Before host close returns, every in-progress reply callback must
 * have returned and no future reply callback may begin. payload_json is a
 * borrowed NUL-terminated UTF-8 string for the reply call. ok == 1 requires
 * valid JSON, including literal null; other values report an error payload.
 * No C or C++ callback may throw or unwind across this ABI boundary.
 */
LogosInspectorCore* logos_inspector_core_new_with_host_transport(
    const LogosInspectorHostTransportV1* transport);

/*
 * Close is idempotent and may race asynchronous call/cancel entry points.
 * Do not call close or free reentrantly from a core reply or host transport
 * callback. Free must not race any ABI call or callback and invokes close
 * before releasing the handle.
 */
void logos_inspector_core_close(LogosInspectorCore* handle);
void logos_inspector_core_free(LogosInspectorCore* handle);

/*
 * Handles created with host transport reject these synchronous entry points
 * with an async-required error and never invoke host dispatch.
 */
char* logos_inspector_core_call(
    LogosInspectorCore* handle,
    const char* method,
    const char* args_json);

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
