#ifndef LOGOS_PROTOCOL_H
#define LOGOS_PROTOCOL_H

/* ===========================================================================
 * logos_protocol.h — the public, language-neutral C ABI of logos-protocol.
 *
 * This is the ONE seam every Logos SDK builds on. The data model is
 * JSON-in-strings: method arguments are a JSON array, results are a JSON
 * value, event payloads are a JSON array — all UTF-8 `const char*`.
 *
 * Ownership:
 *   - Every `char*` RETURNED by this library is heap-allocated and owned by
 *     the caller; free it with lp_string_free() (safe on NULL).
 *   - Every `const char*` PASSED IN is borrowed for the duration of the call.
 *
 * Bytes encoding: binary data crossing this ABI is encoded inside JSON as
 *   {"_bytes": "<base64url>"}
 * (a single-key object). This is lossless for arbitrary bytes, including
 * embedded NUL. It matches the plain-wire encoding (json_mapping.cpp) and is
 * the canonical representation at this boundary.
 *
 * Error shape: structural failures report one canonical JSON object through
 * `out_error_json` / error callbacks:
 *   {"code": "<machine_code>", "message": "<human text>", "origin": "<module>"}
 *
 * Threading / event-loop contract:
 *   - Callbacks may arrive on an internal protocol thread — never assume
 *     they run on your own thread.
 *   - Handles are thread-safe per-handle: calls on one handle may be made
 *     from any thread; the library marshals to the handle's owner thread
 *     internally where required.
 *   - Qt-free transports (plain tcp/tcp_ssl, mock) are serviced by the
 *     library's own workers — no caller event loop is needed.
 *   - The Qt Remote Objects transport (the current default inside module
 *     processes) ADDITIONALLY requires a running Qt event loop in the
 *     process. Every Logos module process has one (logos_host runs it).
 *     Standalone non-Qt consumers must use the plain transport.
 *   - lp_invoke() blocks the calling thread until the result arrives or the
 *     timeout elapses (timeout_ms <= 0 selects the default, currently 20s).
 *
 * Cancellation / lifetime:
 *   - After lp_client_destroy() / lp_unsubscribe() RETURNS, no further
 *     callbacks fire for that handle; pending async results are dropped.
 *     `user_data` may be freed only after that point, never before.
 *
 * Versioning: this library carries the logos-protocol semantic version —
 * the single number that governs Logos load/call compatibility. Two
 * participants interoperate iff they share the same MAJOR. MINOR is
 * additive/back-compatible; PATCH never affects compatibility.
 * =========================================================================== */

#define LOGOS_PROTOCOL_VERSION_MAJOR 0
// 0.2: per-module concurrent dispatch ("multi"). Additive/back-compatible — a
// multi module returns a deferred-completion sentinel from callMethod and pushes
// the result as a __logos_call_complete__ event (see logos_async_dispatch.h);
// the provider/host ABI is UNCHANGED, so same-MAJOR hosts (incl. 0.1 daemons)
// load and forward multi modules without modification. A pre-0.2 *consumer*
// would see the raw sentinel rather than awaiting it — graceful, not a crash.
#define LOGOS_PROTOCOL_VERSION_MINOR 2
#define LOGOS_PROTOCOL_VERSION_PATCH 1
#define LOGOS_PROTOCOL_VERSION_STRING "0.2.1"

#ifdef __cplusplus
extern "C" {
#endif

/* ---------------------------------------------------------------------------
 * Return codes (negative = failure). Functions returning int use these.
 * ------------------------------------------------------------------------- */
#define LP_OK 0
#define LP_ERR_INVALID_ARG (-1)
#define LP_ERR_UNSUPPORTED (-2) /* provider surface: exercised in a later phase */
#define LP_ERR_INTERNAL (-3)
#define LP_ERR_UNAVAILABLE (-4) /* target module/object could not be acquired */

/* ---------------------------------------------------------------------------
 * Version
 * ------------------------------------------------------------------------- */

/** Version string "MAJOR.MINOR.PATCH" of the linked logos-protocol.
 *  Returns a static string — do NOT free. */
const char* lp_protocol_version(void);

/** MAJOR component of the linked logos-protocol version. Equal majors are
 *  compatible; unequal majors are not. */
int lp_protocol_abi_major(void);

/* ---------------------------------------------------------------------------
 * Memory
 * ------------------------------------------------------------------------- */

/** Free a string returned by this library. Safe to call with NULL. */
void lp_string_free(char* s);

/* ---------------------------------------------------------------------------
 * Process-global mode / transport defaults
 * ------------------------------------------------------------------------- */

/** Set the process-wide communication mode: "remote" (IPC, default),
 *  "local" (in-process registry) or "mock" (in-memory, for tests).
 *  Returns LP_OK or LP_ERR_INVALID_ARG. */
int lp_set_mode(const char* mode);

/** Current mode as "remote" | "local" | "mock". Static string — do not free. */
const char* lp_get_mode(void);

/** Set the process-global default transport from a JSON object, e.g.
 *    {"protocol":"local"}
 *    {"protocol":"tcp","host":"127.0.0.1","port":6001,"codec":"json"}
 *    {"protocol":"tcp_ssl","host":"...","port":6443,"codec":"cbor",
 *     "ca_file":"...","cert_file":"...","key_file":"...","verify_peer":true}
 *  Returns LP_OK or LP_ERR_INVALID_ARG on parse failure. */
int lp_set_default_transport(const char* transport_json);

/* ---------------------------------------------------------------------------
 * Consumer: clients, invoke, subscribe
 * ------------------------------------------------------------------------- */

typedef struct lp_client lp_client;
typedef struct lp_subscription lp_subscription;

/** Result callback for lp_invoke_async.
 *  ok != 0 → `json` is the result JSON value; ok == 0 → `json` is the
 *  canonical error object. `json` is only valid for the duration of the
 *  callback — copy it if you need it longer. */
typedef void (*lp_result_cb)(int ok, const char* json, void* user_data);

/** Event callback for lp_subscribe. `data_json` is a JSON array (the event
 *  payload), valid only for the duration of the callback. */
typedef void (*lp_event_cb)(const char* event_name, const char* data_json,
                            void* user_data);

/**
 * Create a client for calling `target_module` on behalf of `origin_module`.
 *
 * `target_transport_json` / `capability_transport_json`: JSON object as for
 * lp_set_default_transport(), or NULL to use the process default. The
 * capability transport is used by the automatic `requestModule` token-fetch
 * flow (this library dials `capability_module` transparently the first time
 * a target requires a token — every language gets that flow for free).
 *
 * The calling thread becomes the client's owner thread; with the Qt Remote
 * Objects transport it must run a Qt event loop. Returns NULL on invalid
 * arguments.
 */
lp_client* lp_client_create(const char* target_module,
                            const char* origin_module,
                            const char* target_transport_json,
                            const char* capability_transport_json);

/** Destroy a client. After this returns, no further callbacks fire for the
 *  client or its subscriptions. Logical close is synchronous; QObject-backed
 *  transport resources may be released later on the owner event loop. A
 *  foreign caller does not wait for owner-loop progress, but does wait for an
 *  already-running user callback to finish. */
void lp_client_destroy(lp_client* client);

/**
 * Call `method` on the client's target module, blocking until the result
 * arrives or the timeout elapses.
 *
 * `args_json`: JSON array of arguments (NULL means "[]").
 * `timeout_ms <= 0` selects the default timeout.
 *
 * On LP_OK: *out_result_json (if non-NULL) receives the result JSON value
 * (which may be the successful value "null").
 * On failure: *out_error_json (if non-NULL) receives the canonical error
 * object. Both out-strings are owned by the caller (lp_string_free).
 */
int lp_invoke(lp_client* client,
              const char* method,
              const char* args_json,
              int timeout_ms,
              char** out_result_json,
              char** out_error_json);

/**
 * Asynchronous variant of lp_invoke. Returns LP_OK once the call is accepted
 * for asynchronous admission; a foreign caller does not wait for owner-loop
 * progress. `cb` then fires exactly once unless client destruction cancels
 * queued admission or the pending callback. Successful JSON `null` is
 * `ok != 0` with `json == "null"`; unavailability, transport failure, and
 * timeout are `ok == 0` with a canonical error object. Safe to call from any
 * thread.
 */
int lp_invoke_async(lp_client* client,
                    const char* method,
                    const char* args_json,
                    int timeout_ms,
                    lp_result_cb cb,
                    void* user_data);

/**
 * Subscribe to `event_name` emitted by the client's target module.
 * `cb` fires once per event with the payload as a JSON array.
 * Returns NULL on failure (e.g. the target object cannot be acquired).
 */
lp_subscription* lp_subscribe(lp_client* client,
                              const char* event_name,
                              lp_event_cb cb,
                              void* user_data);

/** Cancel a subscription. After this returns the callback will not fire
 *  again (already-running invocations are allowed to finish first). Logical
 *  cancellation does not wait for owner-loop progress; QObject-backed
 *  resource release may finish later on that loop. */
void lp_unsubscribe(lp_subscription* sub);

/** Introspect the target module's methods/events as a JSON array (the
 *  same shape `lm` prints). Caller frees via lp_string_free. NULL on
 *  failure. */
char* lp_get_methods(lp_client* client);

/* ---------------------------------------------------------------------------
 * Tokens
 * ------------------------------------------------------------------------- */

/** Get the stored token for `module_name`. Returns NULL when absent;
 *  caller frees via lp_string_free. */
char* lp_token_get(const char* module_name);

/** Store a token for `module_name`. */
int lp_token_save(const char* module_name, const char* token);

/** Deliver a module token to the client's target (the consumer-side
 *  `informModuleToken`). Returns LP_OK when the target accepted it. */
int lp_inform_module_token(lp_client* client,
                           const char* auth_token,
                           const char* module_name,
                           const char* token);

/* ---------------------------------------------------------------------------
 * Provider (GROUNDWORK — defined and compiled in this version, fully
 * exercised when module authoring lands on the common cdylib module-impl
 * C ABI. Until then lp_provider_register/emit return LP_ERR_UNSUPPORTED.)
 * ------------------------------------------------------------------------- */

typedef struct lp_provider lp_provider;

/** Dispatch a method call. Return a heap string (result JSON value) that the
 *  library frees with lp_string_free; return NULL to signal failure. */
typedef char* (*lp_dispatch_cb)(const char* method, const char* args_json,
                                void* user_data);

/** Return the module's method/event metadata as a JSON array (heap string,
 *  freed by the library via lp_string_free). */
typedef char* (*lp_getmethods_cb)(void* user_data);

/** Accept a token delivered by another module. Return LP_OK to accept. */
typedef int (*lp_token_cb)(const char* module_name, const char* token,
                           void* user_data);

lp_provider* lp_provider_create(const char* module_name,
                                const char* transport_set_json);
void lp_provider_destroy(lp_provider* provider);
int lp_provider_register(lp_provider* provider,
                         lp_dispatch_cb dispatch,
                         lp_getmethods_cb get_methods,
                         lp_token_cb on_token,
                         void* user_data);
int lp_provider_emit_event(lp_provider* provider,
                           const char* event_name,
                           const char* data_json);
int lp_provider_save_token(lp_provider* provider,
                           const char* module_name,
                           const char* token);

#ifdef __cplusplus
}
#endif

#endif /* LOGOS_PROTOCOL_H */
