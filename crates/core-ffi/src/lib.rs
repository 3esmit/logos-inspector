use std::{
    ffi::{CStr, CString, c_char},
    ptr,
};

use logos_inspector::bridge::InspectorBridge;

pub struct LogosInspectorCore {
    bridge: InspectorBridge,
}

#[unsafe(no_mangle)]
pub extern "C" fn logos_inspector_core_new() -> *mut LogosInspectorCore {
    match InspectorBridge::basecamp_unavailable() {
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
        Ok((core, method, args_json)) => core.bridge.call_inspector_json(&method, &args_json),
        Err(error) => InspectorBridge::error_json(error),
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
            core.bridge.call_module_json(&module, &method, &args_json)
        }
        Err(error) => InspectorBridge::error_json(error),
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
        Err(_) => match CString::new(InspectorBridge::error_json(
            "failed to encode bridge response",
        )) {
            Ok(value) => value.into_raw(),
            Err(_) => ptr::null_mut(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn call_returns_error_for_null_handle() -> TestResult {
        let method = CString::new("moduleVersion")?;
        let args = CString::new("[]")?;

        // SAFETY: null handle is an accepted error path for this FFI call.
        let ptr =
            unsafe { logos_inspector_core_call(ptr::null_mut(), method.as_ptr(), args.as_ptr()) };
        let value = response_value(ptr)?;

        if value.get("ok").and_then(Value::as_bool) != Some(false) {
            return err("expected error response");
        }
        expect_error_envelope_shape(&value)?;
        if value
            .get("error")
            .and_then(Value::as_str)
            .is_none_or(|error| !error.contains("not initialized"))
        {
            return err("expected initialization error");
        }
        Ok(())
    }

    #[test]
    fn call_rejects_null_method() -> TestResult {
        let handle = logos_inspector_core_new();
        if handle.is_null() {
            return err("failed to create core handle");
        }
        let args = CString::new("[]")?;

        // SAFETY: handle was created by this library; null method is an
        // accepted error path for this FFI call.
        let ptr = unsafe { logos_inspector_core_call(handle, ptr::null(), args.as_ptr()) };
        let value = response_value(ptr)?;

        // SAFETY: handle was created by this library and not yet released.
        unsafe {
            logos_inspector_core_free(handle);
        }

        if value.get("ok").and_then(Value::as_bool) != Some(false) {
            return err("expected error response");
        }
        expect_error_envelope_shape(&value)?;
        if value
            .get("error")
            .and_then(Value::as_str)
            .is_none_or(|error| !error.contains("method is required"))
        {
            return err("expected method error");
        }
        Ok(())
    }

    #[test]
    fn returned_strings_escape_interior_nul() -> TestResult {
        let ptr = into_c_string("a\0b".to_owned());
        let text = c_string_from_owned_ptr(ptr)?;

        if text != "a\\u0000b" {
            return err("expected escaped interior nul");
        }
        Ok(())
    }

    #[test]
    fn handles_keep_independent_command_surfaces() -> TestResult {
        let module = CString::new("logos_inspector")?;
        let method = CString::new("sourcePolicy")?;
        let args = CString::new("[]")?;
        let first = logos_inspector_core_new();
        let second = logos_inspector_core_new();
        if first.is_null() || second.is_null() || first == second {
            // SAFETY: null is accepted; non-null handles were created above.
            unsafe {
                logos_inspector_core_free(first);
                logos_inspector_core_free(second);
            }
            return err("failed to create independent core handles");
        }

        // SAFETY: both handles and C strings remain live for these calls.
        let first_response = unsafe {
            logos_inspector_core_call_module(first, module.as_ptr(), method.as_ptr(), args.as_ptr())
        };
        // SAFETY: both handles and C strings remain live for these calls.
        let second_response = unsafe {
            logos_inspector_core_call_module(
                second,
                module.as_ptr(),
                method.as_ptr(),
                args.as_ptr(),
            )
        };
        let first_value = response_value(first_response)?;
        let second_value = response_value(second_response)?;

        // SAFETY: first was created above and has not been released.
        unsafe {
            logos_inspector_core_free(first);
        }
        // SAFETY: second remains live after first is released.
        let surviving_response = unsafe {
            logos_inspector_core_call_module(
                second,
                module.as_ptr(),
                method.as_ptr(),
                args.as_ptr(),
            )
        };
        let surviving_value = response_value(surviving_response)?;
        // SAFETY: second was created above and has not been released.
        unsafe {
            logos_inspector_core_free(second);
        }

        if first_value.get("ok").and_then(Value::as_bool) != Some(true)
            || second_value.get("ok").and_then(Value::as_bool) != Some(true)
            || surviving_value.get("ok").and_then(Value::as_bool) != Some(true)
        {
            return Err(std::io::Error::other(format!(
                "independent core handle call failed: first={first_value}, second={second_value}, surviving={surviving_value}"
            ))
            .into());
        }
        if second_value.get("value") != surviving_value.get("value") {
            return err("surviving core handle changed after sibling release");
        }
        Ok(())
    }

    #[test]
    fn new_handle_fails_external_module_calls_closed_without_cli_fallback() -> TestResult {
        let module = CString::new("logos_blockchain")?;
        let method = CString::new("getCryptarchiaInfo")?;
        let args = CString::new("[]")?;
        let handle = logos_inspector_core_new();
        if handle.is_null() {
            return err("failed to create core handle");
        }

        // SAFETY: handle and C strings remain live for this call.
        let response = unsafe {
            logos_inspector_core_call_module(
                handle,
                module.as_ptr(),
                method.as_ptr(),
                args.as_ptr(),
            )
        };
        let value = response_value(response);
        // SAFETY: handle was created above and has not been released.
        unsafe {
            logos_inspector_core_free(handle);
        }
        let value = value?;

        if value.get("ok").and_then(Value::as_bool) != Some(false) {
            return err("expected fail-closed error response");
        }
        expect_error_envelope_shape(&value)?;
        if value.get("error").and_then(Value::as_str)
            != Some(
                "Basecamp host module transport is unavailable: the pinned protocol does not provide safe async error and close semantics",
            )
        {
            return Err(std::io::Error::other(format!(
                "unexpected Basecamp transport error: {value}"
            ))
            .into());
        }
        if value.get("error_details").is_some() {
            return err("unexpected structured details in transport error");
        }
        Ok(())
    }

    fn expect_error_envelope_shape(value: &Value) -> TestResult {
        if !value.get("value").is_some_and(Value::is_null) {
            return err("expected null envelope value");
        }
        if value.get("text").and_then(Value::as_str) != Some("") {
            return err("expected empty envelope text");
        }
        Ok(())
    }

    fn response_value(ptr: *mut c_char) -> Result<Value, Box<dyn std::error::Error>> {
        let text = c_string_from_owned_ptr(ptr)?;
        Ok(serde_json::from_str(&text)?)
    }

    fn c_string_from_owned_ptr(ptr: *mut c_char) -> Result<String, Box<dyn std::error::Error>> {
        if ptr.is_null() {
            return err("FFI returned null string");
        }
        // SAFETY: pointer is returned by this library and remains valid until
        // the matching free below.
        let text = unsafe { CStr::from_ptr(ptr) }.to_str()?.to_owned();
        // SAFETY: pointer was returned by this library and is released once.
        unsafe {
            logos_inspector_core_string_free(ptr);
        }
        Ok(text)
    }

    fn err<T>(message: &str) -> Result<T, Box<dyn std::error::Error>> {
        Err(std::io::Error::other(message).into())
    }
}
