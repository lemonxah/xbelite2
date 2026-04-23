pub mod profile_model;
pub mod device_monitor;
pub mod evdev_reader;
pub mod hw_config;
pub mod writer;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    // Set default style for QtQuick.Controls
    std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Fusion");

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        let url = QUrl::from("qrc:/qt/qml/com/xbelite2/gui/qml/main.qml");
        eprintln!("Loading QML from: {}", url.to_string());
        engine.load(&url);
    }

    if let Some(app) = app.as_mut() {
        eprintln!("Starting event loop...");
        app.exec();
    }
}
