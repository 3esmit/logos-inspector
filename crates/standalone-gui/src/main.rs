mod bridge;
mod runtime;

use anyhow::Result;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() -> Result<()> {
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let entry = runtime::qml_entry()?;

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(&format!("file://{}", entry.display())));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }

    Ok(())
}
