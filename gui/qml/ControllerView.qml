import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Visual representation of the Elite 2 controller (simplified SVG-like)
Item {
    Rectangle {
        anchors.fill: parent
        color: "#16213e"
        radius: 12
        border.color: "#533483"

        ColumnLayout {
            anchors.centerIn: parent
            spacing: 15

            Label {
                text: "Controller Layout"
                color: "#888"
                font.pixelSize: 12
                Layout.alignment: Qt.AlignHCenter
            }

            // Front face buttons
            Grid {
                columns: 5
                spacing: 8
                Layout.alignment: Qt.AlignHCenter

                // Row 1: LB / empty / Xbox / empty / RB
                ControllerButton { label: "LB"; color: "#533483" }
                Item { width: 40; height: 30 }
                ControllerButton { label: "Xbox"; color: "#107c10"; width: 40 }
                Item { width: 40; height: 30 }
                ControllerButton { label: "RB"; color: "#533483" }

                // Row 2: View / L3 / empty / R3 / Menu
                ControllerButton { label: "View"; color: "#444"; width: 40 }
                ControllerButton { label: "L3"; color: "#444" }
                Item { width: 40; height: 30 }
                ControllerButton { label: "R3"; color: "#444" }
                ControllerButton { label: "Menu"; color: "#444"; width: 40 }

                // Row 3: D-pad / empty / A B X Y
                ControllerButton { label: "D-Pad"; color: "#444"; width: 40 }
                Item { width: 40; height: 30 }
                Item { width: 40; height: 30 }
                Item { width: 40; height: 30 }
                Grid {
                    columns: 2
                    spacing: 4
                    ControllerButton { label: "X"; color: "#0078d4" }
                    ControllerButton { label: "Y"; color: "#ffd800" }
                    ControllerButton { label: "A"; color: "#107c10" }
                    ControllerButton { label: "B"; color: "#e74c3c" }
                }
            }

            // Paddles (back)
            Label {
                text: "Back Paddles"
                color: "#888"
                font.pixelSize: 11
                Layout.alignment: Qt.AlignHCenter
            }

            Grid {
                columns: 4
                spacing: 8
                Layout.alignment: Qt.AlignHCenter

                ControllerButton { label: "P3\nUL"; color: "#e67e22"; width: 50; height: 40 }
                ControllerButton { label: "P4\nLL"; color: "#e67e22"; width: 50; height: 40 }
                ControllerButton { label: "P2\nLR"; color: "#e67e22"; width: 50; height: 40 }
                ControllerButton { label: "P1\nUR"; color: "#e67e22"; width: 50; height: 40 }
            }

            // Triggers
            Label {
                text: "Triggers"
                color: "#888"
                font.pixelSize: 11
                Layout.alignment: Qt.AlignHCenter
            }

            RowLayout {
                spacing: 40
                Layout.alignment: Qt.AlignHCenter
                ControllerButton { label: "LT"; color: "#533483"; width: 60; height: 30 }
                ControllerButton { label: "RT"; color: "#533483"; width: 60; height: 30 }
            }
        }
    }

    component ControllerButton: Rectangle {
        property string label: ""
        property color buttonColor: "#444"
        width: 35
        height: 30
        radius: 4
        color: buttonColor
        border.color: Qt.lighter(buttonColor, 1.3)

        Label {
            anchors.centerIn: parent
            text: label
            color: "white"
            font.pixelSize: 9
            font.bold: true
            horizontalAlignment: Text.AlignHCenter
        }
    }
}
