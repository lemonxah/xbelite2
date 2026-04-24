import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    required property var profileModel

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 30
        spacing: 15

        Label {
            text: "Rumble Intensity"
            font.pixelSize: 20; font.bold: true; color: "#e0e0e0"
        }
        Label {
            text: "Per-motor scale for rumble events received while this profile is active. 0 silences the motor."
            color: "#888"; font.pixelSize: 11
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        VibRow { label: "Strong Motor (Main)"; val: profileModel.vibration_main
            onSliderMoved: (newVal) => profileModel.set_vibration(0, newVal)
            onTest: profileModel.test_vibration(0, profileModel.vibration_main) }

        VibRow { label: "Weak Motor"; val: profileModel.vibration_weak
            onSliderMoved: (newVal) => profileModel.set_vibration(1, newVal)
            onTest: profileModel.test_vibration(1, profileModel.vibration_weak) }

        VibRow { label: "Left Trigger Impulse"; val: profileModel.vibration_lt
            onSliderMoved: (newVal) => profileModel.set_vibration(2, newVal)
            onTest: profileModel.test_vibration(2, profileModel.vibration_lt) }

        VibRow { label: "Right Trigger Impulse"; val: profileModel.vibration_rt
            onSliderMoved: (newVal) => profileModel.set_vibration(3, newVal)
            onTest: profileModel.test_vibration(3, profileModel.vibration_rt) }

        Button {
            text: "Test All Motors"
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: 200; Layout.preferredHeight: 40
            onClicked: profileModel.test_all_vibration()
            background: Rectangle { color: parent.hovered ? "#0078d4" : "#222"; radius: 6; border.color: "#0078d4"; border.width: 2 }
            contentItem: Text { text: parent.text; color: "white"; font.bold: true; font.pixelSize: 14; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
        }

        Item { Layout.fillHeight: true }
    }

    component VibRow: Rectangle {
        property string label: ""
        property int val: 100
        signal sliderMoved(int newVal)
        signal test()

        Layout.fillWidth: true; Layout.preferredHeight: 65
        color: "#111"; radius: 8; border.color: "#333"

        RowLayout {
            anchors.fill: parent; anchors.margins: 12; spacing: 12

            Label { text: label; color: "#e0e0e0"; font.pixelSize: 14; Layout.preferredWidth: 160 }

            Slider {
                id: sl; from: 0; to: 100; stepSize: 1; value: val
                Layout.fillWidth: true
                onMoved: sliderMoved(Math.round(value))
            }

            Label { text: Math.round(sl.value) + "%"; color: "#888"; font.pixelSize: 13; Layout.preferredWidth: 45; horizontalAlignment: Text.AlignRight }

            Button {
                text: "Test"; Layout.preferredWidth: 60; Layout.preferredHeight: 30
                onClicked: test()
                background: Rectangle { color: parent.hovered ? "#0078d4" : "#333"; radius: 4; border.color: "#0078d4" }
                contentItem: Text { text: parent.text; color: "#e0e0e0"; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter; font.pixelSize: 12 }
            }
        }
    }
}
