use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QUrl};
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const QML_BASE: &str = "qrc:/qt/qml/com/benwbooth/padproxy/qml";

/// Pick the root QML file from CLI args. `--overlay <name>` loads an overlay
/// window (e.g. `crosshair`); otherwise the main window is shown.
fn select_qml_file() -> String {
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|arg| arg == "--overlay") {
        if let Some(name) = args.get(index + 1) {
            match name.as_str() {
                "crosshair" => return format!("{QML_BASE}/crosshair.qml"),
                "radial" => return format!("{QML_BASE}/radial.qml"),
                "layer" => return format!("{QML_BASE}/layer.qml"),
                "desktop" => return format!("{QML_BASE}/desktop.qml"),
                other => eprintln!("PadProxy: unknown overlay '{other}', showing main window"),
            }
        } else {
            eprintln!("PadProxy: --overlay requires a name (e.g. crosshair)");
        }
    }
    format!("{QML_BASE}/main.qml")
}

fn main() {
    padproxy_gui::gui_bridge::init_qt_static_modules();

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let qml_url = QUrl::from(select_qml_file().as_str());
    let object_created = Arc::new(AtomicBool::new(false));

    if let Some(engine) = engine.as_mut() {
        let object_created = Arc::clone(&object_created);
        engine
            .on_object_created(move |_, qobject, url| {
                if qobject.is_null() {
                    eprintln!("PadProxy failed to create QML root object from {url}");
                } else {
                    object_created.store(true, Ordering::Relaxed);
                    eprintln!("PadProxy loaded QML root object from {url}");
                }
            })
            .release();
    }

    if let Some(engine) = engine.as_mut() {
        engine
            .on_object_creation_failed(|engine, url| {
                eprintln!("PadProxy failed to load QML root object from {url}");
                let engine: Pin<&mut QQmlEngine> = engine.upcast_pin();
                engine.quit();
            })
            .release();
    }

    if let Some(engine) = engine.as_mut() {
        eprintln!("PadProxy loading QML from {qml_url}");
        engine.load(&qml_url);
    }

    if let Some(engine) = engine.as_mut() {
        let engine: Pin<&mut QQmlEngine> = engine.upcast_pin();
        engine.on_quit(|_| {}).release();
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }

    if !object_created.load(Ordering::Relaxed) {
        std::process::exit(1);
    }
}
