import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    required property var profileModel

    RowLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 20

        // Left: live trigger visualizations
        ColumnLayout {
            Layout.preferredWidth: 200
            Layout.fillHeight: true
            spacing: 15

            Label {
                text: "Live Triggers"
                font.pixelSize: 16; font.bold: true; color: "#e0e0e0"
                Layout.alignment: Qt.AlignHCenter
            }

            // LT bar
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: "#111"; radius: 8; border.color: "#333"

                ColumnLayout {
                    anchors.fill: parent; anchors.margins: 10; spacing: 5

                    Label { text: "Left Trigger"; color: "#e0e0e0"; font.pixelSize: 13; font.bold: true; Layout.alignment: Qt.AlignHCenter }

                    Rectangle {
                        Layout.fillWidth: true; Layout.fillHeight: true
                        color: "#0a0a0a"; radius: 4; border.color: "#333"
                        clip: true

                        // Saturation threshold line — output clips to max above this
                        Rectangle {
                            anchors.left: parent.left; anchors.right: parent.right
                            y: parent.height - (parent.height * profileModel.left_trigger_sat / 255)
                            height: 1; color: "#e74c3c"; opacity: 0.6
                        }
                        Rectangle {
                            anchors.left: parent.left; anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: parent.height * (profileModel.live_lt / 1023.0)
                            color: "#0078d4"; radius: 4; opacity: 0.7
                        }
                    }

                    Label {
                        text: profileModel.live_lt + " / 1023"
                        color: "#888"; font.pixelSize: 11; font.family: "monospace"
                        Layout.alignment: Qt.AlignHCenter
                    }
                }
            }

            // RT bar
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: "#111"; radius: 8; border.color: "#333"

                ColumnLayout {
                    anchors.fill: parent; anchors.margins: 10; spacing: 5

                    Label { text: "Right Trigger"; color: "#e0e0e0"; font.pixelSize: 13; font.bold: true; Layout.alignment: Qt.AlignHCenter }

                    Rectangle {
                        Layout.fillWidth: true; Layout.fillHeight: true
                        color: "#0a0a0a"; radius: 4; border.color: "#333"
                        clip: true

                        Rectangle {
                            anchors.left: parent.left; anchors.right: parent.right
                            y: parent.height - (parent.height * profileModel.right_trigger_sat / 255)
                            height: 1; color: "#e74c3c"; opacity: 0.6
                        }
                        Rectangle {
                            anchors.left: parent.left; anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: parent.height * (profileModel.live_rt / 1023.0)
                            color: "#0078d4"; radius: 4; opacity: 0.7
                        }
                    }

                    Label {
                        text: profileModel.live_rt + " / 1023"
                        color: "#888"; font.pixelSize: 11; font.family: "monospace"
                        Layout.alignment: Qt.AlignHCenter
                    }
                }
            }
        }

        // Right: trigger saturation sliders
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 20

            Label {
                text: "Trigger Saturation (Max)"
                font.pixelSize: 16; font.bold: true; color: "#e0e0e0"
            }
            Label {
                text: "Physical travel at which the output reaches its maximum. 255 = full analog, lower = hair-trigger, 0 = binary (on/off)."
                color: "#888"; font.pixelSize: 11
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            GroupBox {
                Layout.fillWidth: true
                title: "Left Trigger"
                label: Label { text: parent.title; color: "#e0e0e0"; font.pixelSize: 13; font.bold: true }
                background: Rectangle { color: "#111"; radius: 8; border.color: "#333"; y: 25 }

                GridLayout {
                    columns: 3; columnSpacing: 10; rowSpacing: 10

                    Label { text: "Max"; color: "#888" }
                    Slider {
                        id: ltSat; from: 0; to: 255; stepSize: 1
                        value: profileModel.left_trigger_sat
                        Layout.fillWidth: true
                        onMoved: profileModel.set_trigger_saturation(0, value)
                    }
                    Label { text: Math.round(ltSat.value).toString(); color: "#888"; Layout.preferredWidth: 35 }
                }
            }

            GroupBox {
                Layout.fillWidth: true
                title: "Right Trigger"
                label: Label { text: parent.title; color: "#e0e0e0"; font.pixelSize: 13; font.bold: true }
                background: Rectangle { color: "#111"; radius: 8; border.color: "#333"; y: 25 }

                GridLayout {
                    columns: 3; columnSpacing: 10; rowSpacing: 10

                    Label { text: "Max"; color: "#888" }
                    Slider {
                        id: rtSat; from: 0; to: 255; stepSize: 1
                        value: profileModel.right_trigger_sat
                        Layout.fillWidth: true
                        onMoved: profileModel.set_trigger_saturation(1, value)
                    }
                    Label { text: Math.round(rtSat.value).toString(); color: "#888"; Layout.preferredWidth: 35 }
                }
            }

            Item { Layout.fillHeight: true }
        }
    }
}
