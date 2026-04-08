import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    required property var profileModel
    readonly property bool isPassthrough: profileModel.hw_profile === 0
    readonly property bool canEdit: profileModel.is_usb && !isPassthrough

    // GIP button codes (hardware-level)
    readonly property var gipCodes: ({
            "A": 0x04,
            "B": 0x05,
            "X": 0x06,
            "Y": 0x07,
            "LB": 0x08,
            "RB": 0x09,
            "LT": 0x0A,
            "RT": 0x0B,
            "DUp": 0x0C,
            "DDown": 0x0D,
            "DLeft": 0x0E,
            "DRight": 0x0F,
            "L Stick": 0x10,
            "R Stick": 0x11,
            "P1": 0x12,
            "P2": 0x13,
            "P3": 0x14,
            "P4": 0x15
        })

    // Buttons that can be remapped — face, bumpers, triggers, dpad, sticks
    // Paddles are hardware-linked to face buttons in the profile, not individually remappable
    // View, Menu, Xbox are not remappable
    readonly property var remappableButtons: ({
            "A": true,
            "B": true,
            "X": true,
            "Y": true,
            "LB": true,
            "RB": true,
            "LT": true,
            "RT": true,
            "DUp": true,
            "DDown": true,
            "DLeft": true,
            "DRight": true,
            "L Stick": true,
            "R Stick": true
        })

    // Target buttons for remapping (what a button can be remapped TO)
    // Paddles map to existing controller buttons, not independent actions
    readonly property var remapTargets: ["A", "B", "X", "Y", "LB", "RB", "LT", "RT", "DUp", "DDown", "DLeft", "DRight", "L Stick", "R Stick"]

    // All buttons for display purposes
    readonly property var allButtonNames: ["A", "B", "X", "Y", "LB", "RB", "View", "Menu", "Xbox", "L Stick", "R Stick", "DUp", "DDown", "DLeft", "DRight", "P1", "P2", "P3", "P4"]

    function isBtnPressed(name) {
        var b = profileModel.live_buttons;
        var p = profileModel.live_paddles;
        switch (name) {
        case "A":
            return (b & (1 << 0)) !== 0;
        case "B":
            return (b & (1 << 1)) !== 0;
        case "X":
            return (b & (1 << 2)) !== 0;
        case "Y":
            return (b & (1 << 3)) !== 0;
        case "LB":
            return (b & (1 << 4)) !== 0;
        case "RB":
            return (b & (1 << 5)) !== 0;
        case "View":
            return (b & (1 << 6)) !== 0;
        case "Menu":
            return (b & (1 << 7)) !== 0;
        case "Xbox":
            return (b & (1 << 8)) !== 0;
        case "L Stick":
            return (b & (1 << 9)) !== 0;
        case "R Stick":
            return (b & (1 << 10)) !== 0;
        case "DUp":
            return (b & (1 << 11)) !== 0;
        case "DDown":
            return (b & (1 << 12)) !== 0;
        case "DLeft":
            return (b & (1 << 13)) !== 0;
        case "DRight":
            return (b & (1 << 14)) !== 0;
        case "P1":
            return (p & 0x01) !== 0;
        case "P2":
            return (p & 0x02) !== 0;
        case "P3":
            return (p & 0x04) !== 0;
        case "P4":
            return (p & 0x08) !== 0;
        }
        return false;
    }

    property string selectedButton: ""
    property bool shiftMode: false
    property int remapVersion: 0 // bumped to force UI rebind

    property string shiftButton: "" // Which button is the shift modifier (from hw_profile)

    onSelectedButtonChanged: {
        updateDropdowns();
        // Sync shift modifier checkbox with actual shift button
        shiftModifierCheck.checked = (selectedButton !== "" && selectedButton === shiftButton);
    }
    property var hwRemaps: ({})  // parsed from get_hw_profile_info()

    // Refresh remap display when profile changes or on load
    function refreshRemaps() {
        try {
            var info = JSON.parse(profileModel.get_hw_profile_info());
            hwRemaps = info;
            shiftButton = info.shift_button || "";
        } catch (e) {
            hwRemaps = {};
            shiftButton = "";
        }
        remapVersion++;
        updateDropdowns();
    }

    // Get the normal remap for a button (empty string if default)
    function getNormalRemap(btnName) {
        var _v = remapVersion; // force dependency
        if (hwRemaps && hwRemaps.normal && hwRemaps.normal[btnName])
            return hwRemaps.normal[btnName];
        return "";
    }

    // Get the shift remap for a button
    function getShiftRemap(btnName) {
        var _v = remapVersion; // force dependency
        if (hwRemaps && hwRemaps.shift && hwRemaps.shift[btnName])
            return hwRemaps.shift[btnName];
        return "";
    }

    Connections {
        target: profileModel
        function onHw_profileChanged() {
            refreshRemaps();
        }
        function onIs_usbChanged() {
            refreshRemaps();
        }
        function onConnectedChanged() {
            refreshRemaps();
        }
    }

    // Set dropdowns to current remap values when a button is selected
    function updateDropdowns() {
        if (selectedButton === "")
            return;
        var normalVal = getNormalRemap(selectedButton);
        var shiftVal = getShiftRemap(selectedButton);

        // Find index in remapTargets (0 = Default, 1+ = button name)
        normalTarget.currentIndex = normalVal === "" ? 0 : remapTargets.indexOf(normalVal) + 1;
        shiftTarget.currentIndex = shiftVal === "" ? 0 : remapTargets.indexOf(shiftVal) + 1;
    }

    // Apply remap immediately from current dropdown state
    function applyCurrentRemap() {
        if (selectedButton === "" || !selectedIsRemappable) return
        var src = selectedButton
        var normalBtn = normalTarget.currentIndex === 0 ? src : remapTargets[normalTarget.currentIndex - 1]
        var shiftBtn = shiftTarget.currentIndex === 0 ? src : remapTargets[shiftTarget.currentIndex - 1]
        profileModel.set_hw_remap(src, normalBtn, shiftBtn)
        // Optimistic update
        if (!hwRemaps.normal) hwRemaps.normal = {}
        if (!hwRemaps.shift) hwRemaps.shift = {}
        if (normalBtn === src) delete hwRemaps.normal[src]
        else hwRemaps.normal[src] = normalBtn
        if (shiftBtn === src) delete hwRemaps.shift[src]
        else hwRemaps.shift[src] = shiftBtn
        remapVersion++
    }

    Component.onCompleted: refreshRemaps()

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
                model: ["LB", "View", "DUp", "DDown", "DLeft", "DRight", "L Stick", "P3", "P4"]
                SideBtn {
                    label: modelData
                    sel: canEdit && selectedButton === modelData
                    pressed: isBtnPressed(modelData)
                    accent: (modelData === "P3" || modelData === "P4") ? "#e67e22" : "#ccc"
                    onClicked: if (canEdit)
                        selectedButton = modelData
                    clickable: canEdit
                    remappable: modelData in remappableButtons
                    normalRemap: getNormalRemap(modelData)
                    shiftRemap: getShiftRemap(modelData)
                }
            }
        }

        // Center: Xbox button + controller image
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ColumnLayout {
                anchors.fill: parent
                spacing: 4

                // Xbox button at top center
                SideBtn {
                    Layout.alignment: Qt.AlignHCenter
                    label: "Xbox"
                    sel: canEdit && selectedButton === "Xbox"
                    pressed: isBtnPressed("Xbox")
                    accent: "#107c10"
                    onClicked: if (canEdit)
                        selectedButton = "Xbox"
                    clickable: canEdit
                    remappable: false
                    normalRemap: getNormalRemap("Xbox")
                    shiftRemap: getShiftRemap("Xbox")
                }

                Image {
                    id: ctrlImg
                    source: "qrc:/assets/elite2.png"
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    fillMode: Image.PreserveAspectFit
                    smooth: true
                    opacity: 0.9
                }
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
                model: ["RB", "Menu", "A", "B", "X", "Y", "R Stick", "P1", "P2"]
                SideBtn {
                    label: modelData
                    sel: canEdit && selectedButton === modelData
                    pressed: isBtnPressed(modelData)
                    accent: {
                        switch (modelData) {
                        case "Y":
                            return "#c8b517";
                        case "B":
                            return "#e74c3c";
                        case "A":
                            return "#107c10";
                        case "X":
                            return "#0078d4";
                        case "P1":
                        case "P2":
                            return "#e67e22";
                        default:
                            return "#ccc";
                        }
                    }
                    onClicked: if (canEdit)
                        selectedButton = modelData
                    clickable: canEdit
                    remappable: modelData in remappableButtons
                    normalRemap: getNormalRemap(modelData)
                    shiftRemap: getShiftRemap(modelData)
                }
            }
        }
    }

    // Remap bar at bottom
    readonly property bool selectedIsRemappable: selectedButton in remappableButtons
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        height: (selectedButton !== "" && canEdit) ? 60 : 0
        color: "#111"
        visible: height > 0
        z: 10

        RowLayout {
            anchors.fill: parent
            anchors.margins: 8
            spacing: 12

            Label {
                text: {
                    if (!selectedIsRemappable)
                        return "<b>" + selectedButton + "</b> — not remappable";
                    var isPaddle = selectedButton.startsWith("P");
                    if (isPaddle)
                        return "Bind <b>" + selectedButton + "</b> to →";
                    return "Remap <b>" + selectedButton + "</b> →";
                }
                color: selectedIsRemappable ? "#e0e0e0" : "#888"
                font.pixelSize: 13
                textFormat: Text.RichText
            }

            // Shift modifier checkbox
            CheckBox {
                id: shiftModifierCheck
                visible: selectedIsRemappable
                text: "SHIFT MODIFIER"
                checked: false
                contentItem: Text {
                    text: parent.text
                    color: parent.checked ? "#e67e22" : "#888"
                    font.pixelSize: 11
                    font.bold: parent.checked
                    leftPadding: parent.indicator.width + 6
                    verticalAlignment: Text.AlignVCenter
                }
                indicator: Rectangle {
                    width: 16
                    height: 16
                    radius: 3
                    color: parent.checked ? "#e67e22" : "#333"
                    border.color: parent.checked ? "#f39c12" : "#555"
                    x: 0
                    anchors.verticalCenter: parent.verticalCenter
                    Text {
                        anchors.centerIn: parent
                        text: parent.parent.checked ? "S" : ""
                        color: "white"
                        font.pixelSize: 10
                        font.bold: true
                    }
                }
            }

            // Normal mode remap
            ColumnLayout {
                spacing: 2
                visible: selectedIsRemappable && !shiftModifierCheck.checked
                Label {
                    text: "Normal"
                    color: "#888"
                    font.pixelSize: 9
                }
                ComboBox {
                    id: normalTarget
                    Layout.preferredWidth: 120
                    model: ["(Default)"].concat(remapTargets)
                    enabled: !shiftModifierCheck.checked
                    onActivated: applyCurrentRemap()
                    background: Rectangle {
                        color: "#222"
                        radius: 4
                        border.color: "#444"
                    }
                    contentItem: Text {
                        text: normalTarget.displayText
                        color: "#e0e0e0"
                        leftPadding: 8
                        verticalAlignment: Text.AlignVCenter
                        font.pixelSize: 12
                    }
                }
            }

            // Shift mode remap
            ColumnLayout {
                spacing: 2
                visible: selectedIsRemappable && !shiftModifierCheck.checked
                Label {
                    text: "Shift"
                    color: "#9b59b6"
                    font.pixelSize: 9
                }
                ComboBox {
                    id: shiftTarget
                    Layout.preferredWidth: 120
                    model: ["(Default)"].concat(remapTargets)
                    enabled: !shiftModifierCheck.checked
                    onActivated: applyCurrentRemap()
                    background: Rectangle {
                        color: "#1a1028"
                        radius: 4
                        border.color: "#6c3483"
                    }
                    contentItem: Text {
                        text: shiftTarget.displayText
                        color: "#d2b4de"
                        leftPadding: 8
                        verticalAlignment: Text.AlignVCenter
                        font.pixelSize: 12
                    }
                }
            }

            // Shift modifier info
            Label {
                visible: selectedIsRemappable && shiftModifierCheck.checked
                text: "Hold <b>" + selectedButton + "</b> to activate shift remaps"
                color: "#e67e22"
                font.pixelSize: 12
                textFormat: Text.RichText
            }

            Button {
                visible: selectedIsRemappable && shiftModifierCheck.checked
                text: "Set Shift"
                onClicked: profileModel.set_shift_button(selectedButton)
                background: Rectangle {
                    color: parent.hovered ? "#f39c12" : "#e67e22"
                    radius: 4
                }
                contentItem: Text {
                    text: parent.text; color: "white"; font.bold: true
                    horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
                }
            }

            Button {
                visible: selectedIsRemappable && !shiftModifierCheck.checked
                text: "Reset"
                onClicked: {
                    normalTarget.currentIndex = 0
                    shiftTarget.currentIndex = 0
                    applyCurrentRemap()
                }
                background: Rectangle {
                    color: parent.hovered ? "#e74c3c" : "#333"
                    radius: 4
                }
                contentItem: Text {
                    text: parent.text; color: "#ccc"
                    horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
                }
            }

            Item {
                Layout.fillWidth: true
            }
        }
    }

    // Overlay messages
    Rectangle {
        anchors.centerIn: parent
        width: 320
        height: 40
        radius: 8
        color: "#222"
        border.color: "#555"
        visible: isPassthrough
        z: 20

        Label {
            anchors.centerIn: parent
            text: "Profile 0 - Passthrough (paddles as independent buttons)"
            color: "#888"
            font.pixelSize: 13
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: 360
        height: 40
        radius: 8
        color: "#1a1028"
        border.color: "#6c3483"
        visible: !isPassthrough && !profileModel.is_usb
        z: 20

        Label {
            anchors.centerIn: parent
            text: "Connect via USB to edit hardware remaps"
            color: "#9b59b6"
            font.pixelSize: 13
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
        property string normalRemap: ""
        property string shiftRemap: ""
        signal clicked

        width: 105
        height: normalRemap !== "" || shiftRemap !== "" ? 44 : 32
        radius: 4
        color: pressed ? accent : (sel ? "#533483" : (ma.containsMouse && clickable ? "#222" : "#111"))
        border.color: pressed ? accent : (sel ? "#7b4fbf" : (normalRemap !== "" || shiftRemap !== "" ? "#0078d4" : "#333"))
        border.width: (sel || pressed || normalRemap !== "" || shiftRemap !== "") ? 2 : 1
        opacity: clickable ? 1.0 : 0.6

        Column {
            anchors.centerIn: parent
            spacing: 1

            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 3
                Label {
                    text: label
                    color: pressed ? "#000" : accent
                    font.pixelSize: 11
                    font.bold: true
                }
            }

            // Show remap info below the label
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 4
                visible: normalRemap !== "" || shiftRemap !== ""

                Label {
                    text: normalRemap !== "" ? "→" + normalRemap : ""
                    color: pressed ? "#000" : "#4da6ff"
                    font.pixelSize: 8
                    visible: normalRemap !== ""
                }
                Label {
                    text: shiftRemap !== "" ? "S→" + shiftRemap : ""
                    color: pressed ? "#000" : "#9b59b6"
                    font.pixelSize: 8
                    visible: shiftRemap !== ""
                }
            }
        }

        MouseArea {
            id: ma
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: if (clickable)
                parent.clicked()
        }
    }

    // Trigger bar component
    component TriggerBar: Rectangle {
        property string label: ""
        property real fillLevel: 0.0

        width: 105
        height: 32
        radius: 4
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
            font.pixelSize: 10
            font.bold: true
        }
    }
}
