#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef struct LogosInspectorCore LogosInspectorCore;

LogosInspectorCore* logos_inspector_core_new(void);
void logos_inspector_core_free(LogosInspectorCore* handle);

char* logos_inspector_core_call(
    LogosInspectorCore* handle,
    const char* method,
    const char* args_json);

char* logos_inspector_core_call_module(
    LogosInspectorCore* handle,
    const char* module,
    const char* method,
    const char* args_json);

void logos_inspector_core_string_free(char* value);

#ifdef __cplusplus
}
#endif
