mod bridge;

use std::{env, path::PathBuf};

use anyhow::{Context as _, Result};
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() -> Result<()> {
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let entry = qml_entry()?;

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(&format!("file://{}", entry.display())));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }

    Ok(())
}

fn qml_entry() -> Result<PathBuf> {
    let qml_dir = env::var_os("LOGOS_INSPECTOR_QML_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../qml"));
    let entry = qml_dir.join("StandaloneMain.qml");
    entry
        .canonicalize()
        .with_context(|| format!("failed to locate QML entry at {}", entry.display()))
}
