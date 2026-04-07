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

                    // Vertical bar
                    Rectangle {
                        Layout.fillWidth: true; Layout.fillHeight: true
                        color: "#0a0a0a"; radius: 4; border.color: "#333"
                        clip: true

                        // Dead zone min marker
                        Rectangle {
                            anchors.left: parent.left; anchors.right: parent.right
                            y: parent.height - (parent.height * profileModel.left_trigger_min / 1023)
                            height: 1; color: "#e74c3c"; opacity: 0.5
                        }
                        // Dead zone max marker
                        Rectangle {
                            anchors.left: parent.left; anchors.right: parent.right
                            y: parent.height - (parent.height * profileModel.left_trigger_max / 1023)
                            height: 1; color: "#e74c3c"; opacity: 0.5
                        }
                        // Fill
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
                            y: parent.height - (parent.height * profileModel.right_trigger_min / 1023)
                            height: 1; color: "#e74c3c"; opacity: 0.5
                        }
                        Rectangle {
                            anchors.left: parent.left; anchors.right: parent.right
                            y: parent.height - (parent.height * profileModel.right_trigger_max / 1023)
                            height: 1; color: "#e74c3c"; opacity: 0.5
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

        // Right: dead zone sliders
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 20

            Label {
                text: "Trigger Dead Zones"
                font.pixelSize: 16; font.bold: true; color: "#e0e0e0"
            }

            GroupBox {
                Layout.fillWidth: true
                title: "Left Trigger"
                label: Label { text: parent.title; color: "#e0e0e0"; font.pixelSize: 13; font.bold: true }
                background: Rectangle { color: "#111"; radius: 8; border.color: "#333"; y: 25 }

                GridLayout {
                    columns: 3; columnSpacing: 10; rowSpacing: 10

                    Label { text: "Min"; color: "#888" }
                    Slider { id: ltMin; from: 0; to: 512; stepSize: 1; value: profileModel.left_trigger_min; Layout.fillWidth: true; onMoved: profileModel.set_trigger_zone(0, value, ltMax.value) }
                    Label { text: Math.round(ltMin.value).toString(); color: "#888"; Layout.preferredWidth: 35 }

                    Label { text: "Max"; color: "#888" }
                    Slider { id: ltMax; from: 512; to: 1023; stepSize: 1; value: profileModel.left_trigger_max; Layout.fillWidth: true; onMoved: profileModel.set_trigger_zone(0, ltMin.value, value) }
                    Label { text: Math.round(ltMax.value).toString(); color: "#888"; Layout.preferredWidth: 35 }
                }
            }

            GroupBox {
                Layout.fillWidth: true
                title: "Right Trigger"
                label: Label { text: parent.title; color: "#e0e0e0"; font.pixelSize: 13; font.bold: true }
                background: Rectangle { color: "#111"; radius: 8; border.color: "#333"; y: 25 }

                GridLayout {
                    columns: 3; columnSpacing: 10; rowSpacing: 10

                    Label { text: "Min"; color: "#888" }
                    Slider { id: rtMin; from: 0; to: 512; stepSize: 1; value: profileModel.right_trigger_min; Layout.fillWidth: true; onMoved: profileModel.set_trigger_zone(1, value, rtMax.value) }
                    Label { text: Math.round(rtMin.value).toString(); color: "#888"; Layout.preferredWidth: 35 }

                    Label { text: "Max"; color: "#888" }
                    Slider { id: rtMax; from: 512; to: 1023; stepSize: 1; value: profileModel.right_trigger_max; Layout.fillWidth: true; onMoved: profileModel.set_trigger_zone(1, rtMin.value, value) }
                    Label { text: Math.round(rtMax.value).toString(); color: "#888"; Layout.preferredWidth: 35 }
                }
            }

            Item { Layout.fillHeight: true }
        }
    }
}
