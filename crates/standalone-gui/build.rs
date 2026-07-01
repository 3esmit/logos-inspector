use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("LogosInspectorStandalone"))
        .qt_module("Network")
        .file("src/bridge.rs")
        .build();
}
