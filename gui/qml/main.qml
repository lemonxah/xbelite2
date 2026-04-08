import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
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

    ColorDialog {
        id: colorDialog
        title: "Profile LED Color"
        onAccepted: {
            var c = selectedColor.toString()
            profileModel.set_profile_color_hex(c)
        }
    }

    Dialog {
        id: nameDialog
        title: "Device Name"
        standardButtons: Dialog.Ok | Dialog.Cancel
        anchors.centerIn: parent
        modal: true

        ColumnLayout {
            spacing: 10
            Label {
                text: "Enter new device name (max 15 characters):"
                color: "#e0e0e0"
            }
            TextField {
                id: nameField
                text: profileModel.device_name
                maximumLength: 15
                Layout.preferredWidth: 300
                color: "#e0e0e0"
                background: Rectangle { color: "#333"; radius: 4 }
            }
        }

        onAccepted: profileModel.set_device_name_text(nameField.text)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 0

        // Header bar
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 60
            color: "#111111"

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 20
                anchors.rightMargin: 20

                ColumnLayout {
                    spacing: 2

                    RowLayout {
                        spacing: 8
                        Label {
                            text: profileModel.device_name
                            font.pixelSize: 15
                            font.bold: true
                            color: "#e0e0e0"
                        }
                        Label {
                            text: profileModel.is_usb ? "[USB]" : "[BT]"
                            font.pixelSize: 11
                            font.bold: true
                            color: profileModel.is_usb ? "#0078d4" : "#9b59b6"
                        }
                        Button {
                            text: "Rename"
                            visible: profileModel.is_usb
                            implicitWidth: 60
                            implicitHeight: 22
                            font.pixelSize: 10
                            onClicked: {
                                nameField.text = profileModel.device_name
                                nameDialog.open()
                            }
                            background: Rectangle {
                                color: parent.hovered ? "#444" : "#333"
                                radius: 3
                            }
                            contentItem: Text {
                                text: parent.text; color: "#aaa"
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                                font.pixelSize: 10
                            }
                        }
                    }

                    Label {
                        text: {
                            if (!profileModel.connected) return "Disconnected"
                            if (profileModel.hw_profile === 0) return "Profile Default (Passthrough)"
                            var mode = profileModel.is_usb ? "Hardware Editable" : "Read Only"
                            return "Profile " + profileModel.hw_profile + " (" + mode + ")"
                        }
                        font.pixelSize: 11
                        color: profileModel.connected ? "#107c10" : "#e74c3c"
                    }
                }

                Item { Layout.fillWidth: true }

                // Profile color indicator + picker
                RowLayout {
                    spacing: 8
                    visible: profileModel.hw_profile > 0

                    Rectangle {
                        width: 24; height: 24; radius: 12
                        color: profileModel.profile_color === "default" ? "white" : profileModel.profile_color
                        border.color: "#666"
                        border.width: 1

                        MouseArea {
                            anchors.fill: parent
                            enabled: profileModel.is_usb
                            cursorShape: profileModel.is_usb ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onClicked: {
                                if (profileModel.profile_color !== "default")
                                    colorDialog.selectedColor = profileModel.profile_color
                                colorDialog.open()
                            }
                        }
                    }

                    Label {
                        text: profileModel.profile_color === "default" ? "Default" : profileModel.profile_color
                        font.pixelSize: 10
                        color: "#888"
                    }

                    Slider {
                        id: brightnessSlider
                        from: 0; to: 100; stepSize: 1
                        value: profileModel.profile_brightness
                        Layout.preferredWidth: 80
                        enabled: profileModel.is_usb
                        onMoved: profileModel.set_profile_brightness_value(Math.round(value))
                    }

                    Label {
                        text: Math.round(brightnessSlider.value) + "%"
                        font.pixelSize: 9
                        color: "#888"
                    }
                }

                Item { width: 12 }

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

        // BT mode banner
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? 30 : 0
            visible: profileModel.connected && !profileModel.is_usb && profileModel.hw_profile > 0
            color: "#2c1a3d"

            Label {
                anchors.centerIn: parent
                text: "Bluetooth mode - hardware profile editing requires USB connection"
                font.pixelSize: 11
                color: "#9b59b6"
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
