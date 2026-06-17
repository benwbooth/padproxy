use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    println!("cargo:rerun-if-changed=qml/main.qml");
    println!("cargo:rerun-if-changed=src/gui_bridge.rs");

    CxxQtBuilder::new_qml_module(QmlModule::new("com.benwbooth.padproxy").qml_file("qml/main.qml"))
        .qt_module("Network")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .files(["src/gui_bridge.rs"])
        .build();
}
