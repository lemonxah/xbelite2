import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    required property var profileModel
    readonly property bool isPassthrough: profileModel.hw_profile === 0

    readonly property var buttonCodes: ({
        "A": 0x130, "B": 0x131, "X": 0x133, "Y": 0x134,
        "LB": 0x136, "RB": 0x137,
        "View": 0x13A, "Menu": 0x13B, "Xbox": 0x13C,
        "L Stick": 0x13D, "R Stick": 0x13E,
        "D Up": 0x220, "D Down": 0x221, "D Left": 0x222, "D Right": 0x223,
        "P1": 0x225, "P2": 0x227, "P3": 0x224, "P4": 0x226
    })

    readonly property var allButtonNames: [
        "A", "B", "X", "Y", "LB", "RB", "View", "Menu", "Xbox",
        "L Stick", "R Stick", "D Up", "D Down", "D Left", "D Right",
        "P1", "P2", "P3", "P4"
    ]

    // Bitmask: 0-10=buttons, 11=DUp, 12=DDown, 13=DLeft, 14=DRight
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
            case "D Up": return (b & (1<<11)) !== 0
            case "D Down": return (b & (1<<12)) !== 0
            case "D Left": return (b & (1<<13)) !== 0
            case "D Right": return (b & (1<<14)) !== 0
            case "P1": return (p & 0x01) !== 0
            case "P2": return (p & 0x02) !== 0
            case "P3": return (p & 0x04) !== 0
            case "P4": return (p & 0x08) !== 0
        }
        return false
    }

    function getRemapFor(btnName) {
        try {
            var remaps = JSON.parse(profileModel.get_remaps_json())
            var code = buttonCodes[btnName]
            for (var i = 0; i < remaps.length; i++) {
                if (remaps[i].src.code === code) {
                    for (var name in buttonCodes) {
                        if (buttonCodes[name] === remaps[i].dst.code) return name
                    }
                }
            }
        } catch(e) {}
        return ""
    }

    property string selectedButton: ""

    // Layout: left labels | controller image | right labels
    RowLayout {
        anchors.fill: parent
        anchors.margins: 5
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
                model: ["LB", "View", "L Stick", "D Up", "D Down", "D Left", "D Right", "P3", "P4"]
                SideBtn {
                    label: modelData; remap: isPassthrough ? "" : getRemapFor(modelData)
                    sel: !isPassthrough && selectedButton === modelData
                    pressed: isBtnPressed(modelData)
                    accent: (modelData === "P3" || modelData === "P4") ? "#e67e22" : "#ccc"
                    onClicked: if (!isPassthrough) selectedButton = modelData
                    clickable: !isPassthrough
                }
            }
        }

        // Center: controller image with connection lines
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
                    label: modelData; remap: isPassthrough ? "" : getRemapFor(modelData)
                    sel: !isPassthrough && selectedButton === modelData
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
                    onClicked: if (!isPassthrough) selectedButton = modelData
                    clickable: !isPassthrough
                }
            }
        }
    }

    // Remap bar at bottom
    Rectangle {
        anchors.bottom: parent.bottom; anchors.left: parent.left; anchors.right: parent.right
        height: (selectedButton !== "" && !isPassthrough) ? 50 : 0; color: "#111"; visible: selectedButton !== "" && !isPassthrough; z: 10

        RowLayout {
            anchors.fill: parent; anchors.margins: 8; spacing: 12

            Label { text: "Remap <b>" + selectedButton + "</b> →"; color: "#e0e0e0"; font.pixelSize: 13; textFormat: Text.RichText }

            ComboBox {
                id: remapTarget; Layout.preferredWidth: 160
                model: ["(None)"].concat(allButtonNames)
                background: Rectangle { color: "#222"; radius: 4; border.color: "#444" }
                contentItem: Text { text: remapTarget.displayText; color: "#e0e0e0"; leftPadding: 8; verticalAlignment: Text.AlignVCenter }
            }

            Button {
                text: "Apply"
                onClicked: {
                    if (remapTarget.currentIndex === 0) profileModel.remove_remap(buttonCodes[selectedButton])
                    else profileModel.set_remap(buttonCodes[selectedButton], buttonCodes[allButtonNames[remapTarget.currentIndex - 1]])
                }
                background: Rectangle { color: parent.hovered ? "#1a8c1a" : "#107c10"; radius: 4 }
                contentItem: Text { text: parent.text; color: "white"; font.bold: true; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
            }

            Button {
                text: "Clear"
                onClicked: profileModel.remove_remap(buttonCodes[selectedButton])
                background: Rectangle { color: parent.hovered ? "#e74c3c" : "#333"; radius: 4 }
                contentItem: Text { text: parent.text; color: "#ccc"; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
            }

            Item { Layout.fillWidth: true }
        }
    }

    // Side button component
    // Passthrough overlay
    Rectangle {
        anchors.centerIn: parent
        width: 300; height: 40; radius: 8
        color: "#222"
        border.color: "#555"
        visible: isPassthrough
        z: 20

        Label {
            anchors.centerIn: parent
            text: "Profile 0 — Passthrough (no remapping)"
            color: "#888"; font.pixelSize: 13
        }
    }

    component SideBtn: Rectangle {
        property string label: ""
        property string remap: ""
        property bool sel: false
        property bool pressed: false
        property bool clickable: true
        property color accent: "#ccc"
        signal clicked()

        width: 105; height: 32; radius: 4
        color: pressed ? accent : (sel ? "#533483" : (ma.containsMouse && clickable ? "#222" : "#111"))
        border.color: pressed ? accent : (sel ? "#7b4fbf" : (remap !== "" ? "#0078d4" : "#333"))
        border.width: (sel || pressed) ? 2 : 1
        opacity: clickable ? 1.0 : 0.6

        Row {
            anchors.centerIn: parent; spacing: 3
            Label {
                text: parent.parent.label
                color: parent.parent.pressed ? "#000" : parent.parent.accent
                font.pixelSize: 11; font.bold: true
            }
            Label {
                text: parent.parent.remap !== "" ? "→" + parent.parent.remap : ""
                color: parent.parent.pressed ? "#000" : "#4da6ff"
                font.pixelSize: 9
                visible: parent.parent.remap !== ""
            }
        }

        MouseArea { id: ma; anchors.fill: parent; hoverEnabled: true; cursorShape: clickable ? Qt.PointingHandCursor : Qt.ArrowCursor; onClicked: if (clickable) parent.clicked() }
    }

    // Trigger bar component - shows fill level
    component TriggerBar: Rectangle {
        property string label: ""
        property real fillLevel: 0.0  // 0.0 to 1.0

        width: 105; height: 32; radius: 4
        color: "#111"
        border.color: "#333"
        clip: true

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
            color: "#e0e0e0"
            font.pixelSize: 10; font.bold: true
        }
    }
}
