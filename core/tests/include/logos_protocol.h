#pragma once

// Unit-test-only C ABI declaration. Composed-host integration links against
// the pinned production logos-protocol header through its CMake target.

#ifdef __cplusplus
extern "C" {
#endif

#define LP_OK 0
#define LP_ERR_INVALID_ARG (-1)
#define LP_ERR_UNSUPPORTED (-2)
#define LP_ERR_INTERNAL (-3)
#define LP_ERR_UNAVAILABLE (-4)

typedef struct lp_client lp_client;
typedef struct lp_subscription lp_subscription;

typedef void (*lp_result_cb)(int ok, const char* json, void* user_data);
typedef void (*lp_event_cb)(const char* event_name, const char* data_json, void* user_data);

lp_client* lp_client_create(
    const char* target_module,
    const char* origin_module,
    const char* target_transport_json,
    const char* capability_transport_json);
lp_client* lp_client_create_instance(
    const char* target_module,
    const char* target_instance_id,
    const char* origin_module,
    const char* target_transport_json,
    const char* capability_transport_json);
void lp_client_destroy(lp_client* client);
int lp_invoke_async(
    lp_client* client,
    const char* method,
    const char* args_json,
    int timeout_ms,
    lp_result_cb callback,
    void* user_data);
lp_subscription* lp_subscribe(
    lp_client* client,
    const char* event_name,
    lp_event_cb callback,
    void* user_data);
void lp_unsubscribe(lp_subscription* subscription);

#ifdef __cplusplus
}
#endif
