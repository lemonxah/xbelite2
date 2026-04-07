use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.xbelite2.gui")
            .qml_file("qml/main.qml")
            .qml_file("qml/ProfilePage.qml")
            .qml_file("qml/ButtonMapPage.qml")
            .qml_file("qml/StickCurvePage.qml")
            .qml_file("qml/TriggerPage.qml")
            .qml_file("qml/VibrationPage.qml")
            .qml_file("qml/ControllerView.qml"),
    )
    .file("src/profile_model.rs")
    .qrc("assets/resources.qrc")
    .build();
}
