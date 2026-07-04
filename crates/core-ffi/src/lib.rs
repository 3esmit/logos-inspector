use std::{
    ffi::{CStr, CString, c_char},
    ptr,
};

use logos_inspector::bridge::{
    InspectorBridge, call_inspector_response_json, call_module_response_json,
};

pub struct LogosInspectorCore {
    bridge: InspectorBridge,
}

#[unsafe(no_mangle)]
pub extern "C" fn logos_inspector_core_new() -> *mut LogosInspectorCore {
    match InspectorBridge::new() {
        Ok(bridge) => Box::into_raw(Box::new(LogosInspectorCore { bridge })),
        Err(_) => ptr::null_mut(),
    }
}

/// Releases a bridge handle created by `logos_inspector_core_new`.
///
/// # Safety
///
/// `handle` must be null or a pointer returned by `logos_inspector_core_new`
/// that has not already been released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_free(handle: *mut LogosInspectorCore) {
    if handle.is_null() {
        return;
    }

    // SAFETY: `handle` was allocated by `logos_inspector_core_new`; this
    // function is the matching owner-releasing boundary.
    unsafe {
        drop(Box::from_raw(handle));
    }
}

/// Calls a method on the embedded `logos_inspector` bridge.
///
/// # Safety
///
/// `handle` must be null or a live pointer returned by
/// `logos_inspector_core_new`. `method` and `args_json` must be valid
/// NUL-terminated UTF-8 strings for the duration of the call. The returned
/// pointer must be released with `logos_inspector_core_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_call(
    handle: *mut LogosInspectorCore,
    method: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    let response = match call_inputs(handle, method, args_json) {
        Ok((core, method, args_json)) => {
            call_inspector_response_json(&core.bridge, &method, &args_json)
        }
        Err(error) => error_response(error),
    };
    into_c_string(response)
}

/// Calls any module through the embedded inspector bridge.
///
/// # Safety
///
/// `handle` must be null or a live pointer returned by
/// `logos_inspector_core_new`. `module`, `method`, and `args_json` must be valid
/// NUL-terminated UTF-8 strings for the duration of the call. The returned
/// pointer must be released with `logos_inspector_core_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_call_module(
    handle: *mut LogosInspectorCore,
    module: *const c_char,
    method: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    let response = match call_module_inputs(handle, module, method, args_json) {
        Ok((core, module, method, args_json)) => {
            call_module_response_json(&core.bridge, &module, &method, &args_json)
        }
        Err(error) => error_response(error),
    };
    into_c_string(response)
}

/// Releases a string returned by this library.
///
/// # Safety
///
/// `value` must be null or a pointer returned by `logos_inspector_core_call` or
/// `logos_inspector_core_call_module` that has not already been released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }

    // SAFETY: `value` must come from `CString::into_raw` in this library.
    unsafe {
        drop(CString::from_raw(value));
    }
}

fn call_inputs(
    handle: *mut LogosInspectorCore,
    method: *const c_char,
    args_json: *const c_char,
) -> Result<(&'static LogosInspectorCore, String, String), String> {
    Ok((
        core_ref(handle)?,
        c_string(method, "method")?,
        c_string(args_json, "args JSON")?,
    ))
}

fn call_module_inputs(
    handle: *mut LogosInspectorCore,
    module: *const c_char,
    method: *const c_char,
    args_json: *const c_char,
) -> Result<(&'static LogosInspectorCore, String, String, String), String> {
    Ok((
        core_ref(handle)?,
        c_string(module, "module")?,
        c_string(method, "method")?,
        c_string(args_json, "args JSON")?,
    ))
}

fn core_ref(handle: *mut LogosInspectorCore) -> Result<&'static LogosInspectorCore, String> {
    if handle.is_null() {
        return Err("logos inspector core is not initialized".to_owned());
    }

    // SAFETY: caller passes an opaque handle returned by
    // `logos_inspector_core_new`; lifetime is bounded by the host module.
    Ok(unsafe { &*handle })
}

fn c_string(value: *const c_char, label: &str) -> Result<String, String> {
    if value.is_null() {
        return Err(format!("{label} is required"));
    }

    // SAFETY: caller provides a valid NUL-terminated C string for the duration
    // of this call.
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map(ToOwned::to_owned)
        .map_err(|error| format!("{label} is not valid UTF-8: {error}"))
}

fn into_c_string(value: String) -> *mut c_char {
    let sanitized = value.replace('\0', "\\u0000");
    match CString::new(sanitized) {
        Ok(value) => value.into_raw(),
        Err(_) => match CString::new(error_response("failed to encode bridge response")) {
            Ok(value) => value.into_raw(),
            Err(_) => ptr::null_mut(),
        },
    }
}

fn error_response(error: impl std::fmt::Display) -> String {
    serde_json::json!({
        "ok": false,
        "value": null,
        "text": "",
        "error": error.to_string(),
    })
    .to_string()
}
