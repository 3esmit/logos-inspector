use std::{pin::Pin, sync::OnceLock};

use cxx_qt::Threading;
use cxx_qt_lib::QString;
use logos_inspector::bridge::{INSPECTOR_MODULE, InspectorBridge};
use serde::Serialize;
use serde_json::Value;

#[derive(Default)]
pub struct LogosBridgeRust;

#[derive(Debug, Serialize)]
struct BridgeResponse {
    ok: bool,
    value: Value,
    text: String,
    error: String,
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[namespace = "logos_bridge"]
        type LogosBridge = super::LogosBridgeRust;
    }

    extern "RustQt" {
        #[qinvokable]
        #[cxx_name = "callModuleJson"]
        fn call_module_json(
            self: &LogosBridge,
            module: &QString,
            method: &QString,
            args_json: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "callModuleJsonAsync"]
        fn call_module_json_async(
            self: Pin<&mut LogosBridge>,
            request_id: i32,
            module: &QString,
            method: &QString,
            args_json: &QString,
        );

        #[qsignal]
        #[cxx_name = "moduleCallFinished"]
        fn module_call_finished(
            self: Pin<&mut LogosBridge>,
            request_id: i32,
            response_json: &QString,
        );
    }

    impl cxx_qt::Threading for LogosBridge {}
}

impl qobject::LogosBridge {
    pub fn call_module_json(
        &self,
        module: &QString,
        method: &QString,
        args_json: &QString,
    ) -> QString {
        QString::from(call_module_response_json(
            &module.to_string(),
            &method.to_string(),
            &args_json.to_string(),
        ))
    }

    pub fn call_module_json_async(
        self: Pin<&mut Self>,
        request_id: i32,
        module: &QString,
        method: &QString,
        args_json: &QString,
    ) {
        let qt_thread = self.qt_thread();
        let module = module.to_string();
        let method = method.to_string();
        let args_json = args_json.to_string();
        std::thread::spawn(move || {
            let response_json = call_module_response_json(&module, &method, &args_json);
            let _ = qt_thread.queue(move |mut qobject| {
                let response = QString::from(response_json);
                qobject.as_mut().module_call_finished(request_id, &response);
            });
        });
    }
}

fn call_module_response_json(module: &str, method: &str, args_json: &str) -> String {
    let response = match call_module(module, method, args_json) {
        Ok(value) => BridgeResponse {
            ok: true,
            text: format_value(&value),
            value,
            error: String::new(),
        },
        Err(error) => BridgeResponse {
            ok: false,
            value: Value::Null,
            text: String::new(),
            error: format!("{error:#}"),
        },
    };

    serde_json::to_string(&response).unwrap_or_else(|error| {
        format!(
            r#"{{"ok":false,"value":null,"text":"","error":"failed to serialize bridge response: {error}"}}"#
        )
    })
}

fn call_module(module: &str, method: &str, args_json: &str) -> anyhow::Result<Value> {
    let args = serde_json::from_str(args_json)?;
    bridge()?.call_module(module, method, args)
}

fn bridge() -> anyhow::Result<&'static InspectorBridge> {
    static BRIDGE: OnceLock<InspectorBridge> = OnceLock::new();
    if let Some(bridge) = BRIDGE.get() {
        return Ok(bridge);
    }

    let bridge = InspectorBridge::new()?;
    let _ = BRIDGE.set(bridge);
    BRIDGE
        .get()
        .ok_or_else(|| anyhow::anyhow!("failed to initialize {INSPECTOR_MODULE} bridge"))
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}
