use cxx_qt_build::{CppFile, CxxQtBuilder, QmlModule};

fn main() {
    println!("cargo:rerun-if-changed=qml/main.qml");
    println!("cargo:rerun-if-changed=qml/crosshair.qml");
    println!("cargo:rerun-if-changed=qml/radial.qml");
    println!("cargo:rerun-if-changed=qml/layer.qml");
    println!("cargo:rerun-if-changed=qml/desktop.qml");
    println!("cargo:rerun-if-changed=src/gui_bridge.rs");
    println!("cargo:rerun-if-changed=src/qml_registration.cpp");

    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.benwbooth.padproxy")
            .qml_file("qml/main.qml")
            .qml_file("qml/crosshair.qml")
            .qml_file("qml/radial.qml")
            .qml_file("qml/layer.qml")
            .qml_file("qml/desktop.qml"),
    )
    .qrc_resources([
        "qml/images/controller-generic.svg",
        "qml/images/controller-playstation.svg",
        "qml/images/controller-xbox.svg",
    ])
    .qt_module("Network")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Svg")
    .files(["src/gui_bridge.rs"])
    .cpp_file(CppFile::from("src/qml_registration.cpp").moc(false))
    .build();
}
