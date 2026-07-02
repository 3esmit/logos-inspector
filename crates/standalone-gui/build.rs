use std::{env, process::Command};

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    emit_qt_rpath();

    CxxQtBuilder::new_qml_module(QmlModule::new("LogosInspectorStandalone"))
        .qt_module("Network")
        .file("src/bridge.rs")
        .build();
}

fn emit_qt_rpath() {
    println!("cargo:rerun-if-env-changed=QMAKE");
    println!("cargo:rerun-if-env-changed=PATH");
    println!("cargo:rerun-if-env-changed=NIX_BUILD_TOP");

    if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() != Ok("unix")
        || env::var_os("NIX_BUILD_TOP").is_some()
    {
        return;
    }

    let qmake = env::var("QMAKE").unwrap_or_else(|_| "qmake6".to_owned());
    let Some(qt_lib_dir) = qmake_query(&qmake, "QT_INSTALL_LIBS") else {
        return;
    };

    println!("cargo:rustc-link-arg-bin=logos-inspector-standalone-gui=-Wl,-rpath,{qt_lib_dir}");
}

fn qmake_query(qmake: &str, key: &str) -> Option<String> {
    let output = Command::new(qmake).args(["-query", key]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_owned())
}
