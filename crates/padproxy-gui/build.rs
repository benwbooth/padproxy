use cxx_qt_build::{CppFile, CxxQtBuilder, QmlModule};

fn main() {
    println!("cargo:rerun-if-changed=qml/main.qml");
    println!("cargo:rerun-if-changed=src/gui_bridge.rs");
    println!("cargo:rerun-if-changed=src/qml_registration.cpp");

    CxxQtBuilder::new_qml_module(QmlModule::new("com.benwbooth.padproxy").qml_file("qml/main.qml"))
        .qt_module("Network")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .files(["src/gui_bridge.rs"])
        .cpp_file(CppFile::from("src/qml_registration.cpp").moc(false))
        .build();
}
