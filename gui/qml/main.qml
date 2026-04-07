import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.xbelite2.gui

ApplicationWindow {
    id: root
    visible: true
    width: 960
    height: 700
    minimumWidth: 800
    minimumHeight: 600
    title: "Xbox Elite Series 2 - Profile Configurator"
    color: "#000000"

    ProfileModel {
        id: profileModel
        Component.onCompleted: connect_daemon()
    }

    // Poll controller status for live input display
    Timer {
        id: pollTimer
        interval: 16  // ~60fps
        running: false
        repeat: true
        onTriggered: profileModel.refresh_status()
    }

    Component.onCompleted: pollTimer.start()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 0

        // Header bar
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 50
            color: "#111111"

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 20
                anchors.rightMargin: 20

                ColumnLayout {
                    spacing: 2
                    Label {
                        text: profileModel.device_name
                        font.pixelSize: 15
                        font.bold: true
                        color: "#e0e0e0"
                    }
                    Label {
                        text: {
                            if (!profileModel.connected) return "Disconnected"
                            if (profileModel.hw_profile === 0) return "Connected — Default (Passthrough)"
                            return "Connected — Profile " + profileModel.hw_profile + " (Editable)"
                        }
                        font.pixelSize: 11
                        color: profileModel.connected ? "#107c10" : "#e74c3c"
                    }
                }

                Item { Layout.fillWidth: true }

                // HW Profile indicator
                Row {
                    spacing: 6
                    Repeater {
                        model: ["D", "1", "2", "3"]
                        Rectangle {
                            required property int index
                            required property string modelData
                            width: modelData === "D" ? 20 : 16; height: 16; radius: 8
                            color: index === profileModel.hw_profile ? (index === 0 ? "#555" : "#107c10") : "#222"
                            border.color: index === profileModel.hw_profile ? (index === 0 ? "#888" : "#1a8c1a") : "#444"
                            border.width: 1
                            Label {
                                anchors.centerIn: parent
                                text: modelData
                                font.pixelSize: 9; font.bold: true
                                color: index === profileModel.hw_profile ? "white" : "#666"
                            }
                        }
                    }
                }

                Item { width: 20 }

                Button {
                    text: "Save"
                    Layout.preferredWidth: 80
                    Layout.preferredHeight: 32
                    onClicked: profileModel.save_profile()
                    background: Rectangle {
                        color: parent.hovered ? "#1a8c1a" : "#107c10"
                        radius: 4
                    }
                    contentItem: Text {
                        text: parent.text
                        color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        font.bold: true
                    }
                }
            }
        }

        // Tab bar
        TabBar {
            id: tabBar
            Layout.fillWidth: true
            background: Rectangle { color: "#111111" }

            Repeater {
                model: ["Mapping", "Left stick", "Right stick", "Triggers", "Vibration"]
                TabButton {
                    text: modelData
                    width: implicitWidth + 20
                    background: Rectangle {
                        color: tabBar.currentIndex === index ? "#000" : "transparent"
                        Rectangle {
                            anchors.bottom: parent.bottom
                            width: parent.width; height: 2
                            color: tabBar.currentIndex === index ? "#0078d4" : "transparent"
                        }
                    }
                    contentItem: Text {
                        text: parent.text
                        color: tabBar.currentIndex === index ? "#fff" : "#888"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        font.pixelSize: 13
                    }
                }
            }
        }

        // Content
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabBar.currentIndex

            ButtonMapPage { profileModel: profileModel }
            StickCurvePage { profileModel: profileModel; axisIndex: 0 }
            StickCurvePage { profileModel: profileModel; axisIndex: 2 }
            TriggerPage { profileModel: profileModel }
            VibrationPage { profileModel: profileModel }
        }
    }
}
