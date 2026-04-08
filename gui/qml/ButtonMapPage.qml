import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    required property var profileModel
    readonly property bool isPassthrough: profileModel.hw_profile === 0
    readonly property bool canEdit: profileModel.is_usb && !isPassthrough

    // GIP button codes (hardware-level)
    readonly property var gipCodes: ({
        "A": 0x04, "B": 0x05, "X": 0x06, "Y": 0x07,
        "LB": 0x08, "RB": 0x09, "LT": 0x0A, "RT": 0x0B,
        "DUp": 0x0C, "DDown": 0x0D, "DLeft": 0x0E, "DRight": 0x0F
    })

    readonly property var gipNames: [
        "A", "B", "X", "Y", "LB", "RB", "LT", "RT",
        "DUp", "DDown", "DLeft", "DRight"
    ]

    readonly property var allButtonNames: [
        "A", "B", "X", "Y", "LB", "RB",
        "View", "Menu", "Xbox",
        "L Stick", "R Stick",
        "DUp", "DDown", "DLeft", "DRight",
        "P1", "P2", "P3", "P4"
    ]

    function isBtnPressed(name) {
        var b = profileModel.live_buttons
        var p = profileModel.live_paddles
        switch(name) {
            case "A": return (b & (1<<0)) !== 0
            case "B": return (b & (1<<1)) !== 0
            case "X": return (b & (1<<2)) !== 0
            case "Y": return (b & (1<<3)) !== 0
            case "LB": return (b & (1<<4)) !== 0
            case "RB": return (b & (1<<5)) !== 0
            case "View": return (b & (1<<6)) !== 0
            case "Menu": return (b & (1<<7)) !== 0
            case "Xbox": return (b & (1<<8)) !== 0
            case "L Stick": return (b & (1<<9)) !== 0
            case "R Stick": return (b & (1<<10)) !== 0
            case "DUp": return (b & (1<<11)) !== 0
            case "DDown": return (b & (1<<12)) !== 0
            case "DLeft": return (b & (1<<13)) !== 0
            case "DRight": return (b & (1<<14)) !== 0
            case "P1": return (p & 0x01) !== 0
            case "P2": return (p & 0x02) !== 0
            case "P3": return (p & 0x04) !== 0
            case "P4": return (p & 0x08) !== 0
        }
        return false
    }

    property string selectedButton: ""
    property bool shiftMode: false

    // Layout: left labels | controller image | right labels
    RowLayout {
        anchors.fill: parent
        anchors.margins: 5
        anchors.bottomMargin: (selectedButton !== "" && canEdit) ? 60 : 5
        spacing: 0

        // Left side labels
        Column {
            Layout.preferredWidth: 110
            Layout.alignment: Qt.AlignVCenter
            spacing: 6

            TriggerBar {
                label: "LT"
                fillLevel: profileModel.live_lt / 1023.0
            }

            Repeater {
                model: ["LB", "View", "L Stick", "DUp", "DDown", "DLeft", "DRight", "P3", "P4"]
                SideBtn {
                    label: modelData
                    sel: canEdit && selectedButton === modelData
                    pressed: isBtnPressed(modelData)
                    accent: (modelData === "P3" || modelData === "P4") ? "#e67e22" : "#ccc"
                    onClicked: if (canEdit) selectedButton = modelData
                    clickable: canEdit
                    remappable: modelData in gipCodes
                }
            }
        }

        // Center: controller image
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Image {
                id: ctrlImg
                source: "qrc:/assets/elite2.png"
                anchors.centerIn: parent
                width: parent.width
                height: parent.height
                fillMode: Image.PreserveAspectFit
                smooth: true
                opacity: 0.9
            }
        }

        // Right side labels
        Column {
            Layout.preferredWidth: 110
            Layout.alignment: Qt.AlignVCenter
            spacing: 6

            TriggerBar {
                label: "RT"
                fillLevel: profileModel.live_rt / 1023.0
            }

            Repeater {
                model: ["RB", "Menu", "Xbox", "Y", "B", "A", "X", "R Stick", "P1", "P2"]
                SideBtn {
                    label: modelData
                    sel: canEdit && selectedButton === modelData
                    pressed: isBtnPressed(modelData)
                    accent: {
                        switch(modelData) {
                            case "Y": return "#c8b517"
                            case "B": return "#e74c3c"
                            case "A": return "#107c10"
                            case "X": return "#0078d4"
                            case "Xbox": return "#107c10"
                            case "P1": case "P2": return "#e67e22"
                            default: return "#ccc"
                        }
                    }
                    onClicked: if (canEdit) selectedButton = modelData
                    clickable: canEdit
                    remappable: modelData in gipCodes
                }
            }
        }
    }

    // Remap bar at bottom
    Rectangle {
        anchors.bottom: parent.bottom; anchors.left: parent.left; anchors.right: parent.right
        height: (selectedButton !== "" && canEdit && selectedButton in gipCodes) ? 55 : 0
        color: "#111"; visible: height > 0; z: 10

        RowLayout {
            anchors.fill: parent; anchors.margins: 8; spacing: 12

            Label {
                text: "Remap <b>" + selectedButton + "</b> →"
                color: "#e0e0e0"; font.pixelSize: 13; textFormat: Text.RichText
            }

            // Normal mode remap
            ColumnLayout {
                spacing: 2
                Label { text: "Normal"; color: "#888"; font.pixelSize: 9 }
                ComboBox {
                    id: normalTarget; Layout.preferredWidth: 120
                    model: ["(Default)"].concat(gipNames)
                    background: Rectangle { color: "#222"; radius: 4; border.color: "#444" }
                    contentItem: Text {
                        text: normalTarget.displayText; color: "#e0e0e0"
                        leftPadding: 8; verticalAlignment: Text.AlignVCenter; font.pixelSize: 12
                    }
                }
            }

            // Shift mode remap
            ColumnLayout {
                spacing: 2
                Label { text: "Shift"; color: "#9b59b6"; font.pixelSize: 9 }
                ComboBox {
                    id: shiftTarget; Layout.preferredWidth: 120
                    model: ["(Default)"].concat(gipNames)
                    background: Rectangle { color: "#1a1028"; radius: 4; border.color: "#6c3483" }
                    contentItem: Text {
                        text: shiftTarget.displayText; color: "#d2b4de"
                        leftPadding: 8; verticalAlignment: Text.AlignVCenter; font.pixelSize: 12
                    }
                }
            }

            Button {
                text: "Apply"
                onClicked: {
                    var src = selectedButton
                    var normalBtn = normalTarget.currentIndex === 0 ? src : gipNames[normalTarget.currentIndex - 1]
                    var shiftBtn = shiftTarget.currentIndex === 0 ? src : gipNames[shiftTarget.currentIndex - 1]
                    profileModel.set_hw_remap(src, normalBtn, shiftBtn)
                }
                background: Rectangle { color: parent.hovered ? "#1a8c1a" : "#107c10"; radius: 4 }
                contentItem: Text { text: parent.text; color: "white"; font.bold: true; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
            }

            Button {
                text: "Reset"
                onClicked: profileModel.set_hw_remap(selectedButton, selectedButton, selectedButton)
                background: Rectangle { color: parent.hovered ? "#e74c3c" : "#333"; radius: 4 }
                contentItem: Text { text: parent.text; color: "#ccc"; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
            }

            Item { Layout.fillWidth: true }
        }
    }

    // Overlay messages
    Rectangle {
        anchors.centerIn: parent
        width: 320; height: 40; radius: 8
        color: "#222"; border.color: "#555"
        visible: isPassthrough; z: 20

        Label {
            anchors.centerIn: parent
            text: "Profile 0 - Passthrough (no remapping)"
            color: "#888"; font.pixelSize: 13
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: 360; height: 40; radius: 8
        color: "#1a1028"; border.color: "#6c3483"
        visible: !isPassthrough && !profileModel.is_usb; z: 20

        Label {
            anchors.centerIn: parent
            text: "Connect via USB to edit hardware remaps"
            color: "#9b59b6"; font.pixelSize: 13
        }
    }

    // Side button component
    component SideBtn: Rectangle {
        property string label: ""
        property bool sel: false
        property bool pressed: false
        property bool clickable: true
        property bool remappable: true
        property color accent: "#ccc"
        signal clicked()

        width: 105; height: 32; radius: 4
        color: pressed ? accent : (sel ? "#533483" : (ma.containsMouse && clickable ? "#222" : "#111"))
        border.color: pressed ? accent : (sel ? "#7b4fbf" : "#333")
        border.width: (sel || pressed) ? 2 : 1
        opacity: clickable ? 1.0 : 0.6

        Row {
            anchors.centerIn: parent; spacing: 3
            Label {
                text: parent.parent.label
                color: parent.parent.pressed ? "#000" : parent.parent.accent
                font.pixelSize: 11; font.bold: true
            }
            // Show a small indicator if button is remappable (GIP) vs display-only
            Rectangle {
                width: 4; height: 4; radius: 2
                color: "#0078d4"
                visible: parent.parent.remappable && parent.parent.clickable
                anchors.verticalCenter: parent.verticalCenter
            }
        }

        MouseArea {
            id: ma; anchors.fill: parent; hoverEnabled: true
            cursorShape: clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: if (clickable) parent.clicked()
        }
    }

    // Trigger bar component
    component TriggerBar: Rectangle {
        property string label: ""
        property real fillLevel: 0.0

        width: 105; height: 32; radius: 4
        color: "#111"; border.color: "#333"; clip: true

        Rectangle {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: parent.width * fillLevel
            radius: 4
            color: fillLevel > 0.8 ? "#e74c3c" : (fillLevel > 0.3 ? "#c8b517" : "#107c10")
            opacity: 0.6
        }

        Label {
            anchors.centerIn: parent
            text: label + " " + Math.round(fillLevel * 100) + "%"
            color: "#e0e0e0"; font.pixelSize: 10; font.bold: true
        }
    }
}
