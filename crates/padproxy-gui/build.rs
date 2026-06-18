use cxx_qt_build::{CxxQtBuilder, QmlModule};

// The GUI is pure Rust: the QObject is registered with QML via `#[qml_element]`
// (see `gui_bridge.rs`), so no hand-written C++ registration shim is needed. For
// that to work, moc must find QtQml headers; the flake's qmake wrapper points
// `QT_INSTALL_HEADERS` at the merged Qt env so `qqmlregistration.h` is visible.
fn main() {
    println!("cargo:rerun-if-changed=qml/main.qml");
    println!("cargo:rerun-if-changed=qml/crosshair.qml");
    println!("cargo:rerun-if-changed=qml/radial.qml");
    println!("cargo:rerun-if-changed=qml/layer.qml");
    println!("cargo:rerun-if-changed=qml/desktop.qml");
    println!("cargo:rerun-if-changed=qml/magnifier.qml");
    println!("cargo:rerun-if-changed=src/gui_bridge.rs");

    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.benwbooth.padproxy")
            .qml_file("qml/main.qml")
            .qml_file("qml/crosshair.qml")
            .qml_file("qml/radial.qml")
            .qml_file("qml/layer.qml")
            .qml_file("qml/desktop.qml")
            .qml_file("qml/magnifier.qml"),
    )
    .qrc_resources([
        "qml/images/controller-generic.svg",
        "qml/images/controller-playstation.svg",
        "qml/images/controller-xbox.svg",
    ])
    .qt_module("Network")
    .qt_module("Multimedia")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Svg")
    .files(["src/gui_bridge.rs"])
    .build();
}
