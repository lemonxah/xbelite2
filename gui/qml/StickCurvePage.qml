import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    required property var profileModel
    property int axisIndex: 0  // 0=left stick, 2=right stick

    readonly property var axisNames: ["X Axis", "Y Axis"]
    property int selectedAxis: axisIndex
    property var curvePoints: []

    // Live stick values (signed -32768..32767, normalize to -1.0..1.0)
    property real liveX: (axisIndex === 0 ? profileModel.live_lx : profileModel.live_rx) / 32768.0
    property real liveY: (axisIndex === 0 ? profileModel.live_ly : profileModel.live_ry) / 32768.0

    // Stick inversion state (refreshed on profile change)
    property var stickInversion: ({})
    property int invVersion: 0

    function refreshInversion() {
        try {
            stickInversion = JSON.parse(profileModel.get_stick_inversion())
        } catch(e) {
            stickInversion = {}
        }
    }

    Connections {
        target: profileModel
        function onHw_profileChanged() { refreshInversion(); loadCurve() }
    }

    Component.onCompleted: { refreshInversion(); loadCurve() }

    function loadCurve() {
        var json = [
            profileModel.left_stick_x_curve,
            profileModel.left_stick_y_curve,
            profileModel.right_stick_x_curve,
            profileModel.right_stick_y_curve
        ][selectedAxis]

        try {
            var parsed = JSON.parse(json)
            if (parsed && parsed.length === 16) { curvePoints = parsed }
            else { resetToLinear() }
        } catch(e) { resetToLinear() }
        canvas.requestPaint()
    }

    function resetToLinear() {
        var pts = []
        for (var i = 0; i < 16; i++) pts.push(Math.round(i * 32767 / 15))
        curvePoints = pts
        canvas.requestPaint()
    }

    function saveCurve() {
        profileModel.set_stick_curve(selectedAxis, JSON.stringify(curvePoints))
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: 15
        spacing: 15

        // Left: stick visualizer circle
        Rectangle {
            Layout.preferredWidth: 220
            Layout.fillHeight: true
            color: "#111"
            radius: 8
            border.color: "#333"

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 10
                spacing: 8

                Label {
                    text: (axisIndex === 0 ? "Left" : "Right") + " Stick"
                    color: "#e0e0e0"; font.pixelSize: 14; font.bold: true
                    Layout.alignment: Qt.AlignHCenter
                }

                // Stick position visualizer
                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: width

                    Canvas {
                        id: stickViz
                        anchors.fill: parent

                        property real sx: liveX
                        property real sy: liveY

                        onPaint: {
                            var ctx = getContext("2d")
                            var w = width, h = height
                            var cx = w/2, cy = h/2
                            var r = Math.min(cx, cy) - 5

                            ctx.clearRect(0, 0, w, h)

                            // Outer circle
                            ctx.strokeStyle = "#444"
                            ctx.lineWidth = 2
                            ctx.beginPath()
                            ctx.arc(cx, cy, r, 0, Math.PI * 2)
                            ctx.stroke()

                            // Dead zone circle (from profile setting)
                            var dzPct = (axisIndex === 0 ? profileModel.left_stick_deadzone : profileModel.right_stick_deadzone) / 100.0
                            if (dzPct > 0) {
                                ctx.fillStyle = "#2a1515"
                                ctx.strokeStyle = "#e74c3c"
                                ctx.lineWidth = 1
                                ctx.globalAlpha = 0.5
                                ctx.beginPath()
                                ctx.arc(cx, cy, r * dzPct, 0, Math.PI * 2)
                                ctx.fill()
                                ctx.stroke()
                                ctx.globalAlpha = 1.0
                            }

                            // Crosshairs
                            ctx.strokeStyle = "#222"
                            ctx.lineWidth = 1
                            ctx.beginPath()
                            ctx.moveTo(cx - r, cy); ctx.lineTo(cx + r, cy)
                            ctx.moveTo(cx, cy - r); ctx.lineTo(cx, cy + r)
                            ctx.stroke()

                            // Live position dot
                            var px = cx + sx * r
                            var py = cy + sy * r
                            ctx.fillStyle = "#0078d4"
                            ctx.beginPath()
                            ctx.arc(px, py, 8, 0, Math.PI * 2)
                            ctx.fill()

                            // Trail line from center
                            ctx.strokeStyle = "#0078d4"
                            ctx.lineWidth = 2
                            ctx.globalAlpha = 0.4
                            ctx.beginPath()
                            ctx.moveTo(cx, cy)
                            ctx.lineTo(px, py)
                            ctx.stroke()
                            ctx.globalAlpha = 1.0
                        }

                        Timer {
                            interval: 33  // ~30fps
                            running: true; repeat: true
                            onTriggered: stickViz.requestPaint()
                        }
                    }
                }

                // Numeric values
                Label {
                    text: "X: " + Math.round(liveX * 32768) + "  Y: " + Math.round(liveY * 32768)
                    color: "#888"; font.pixelSize: 10; font.family: "monospace"
                    Layout.alignment: Qt.AlignHCenter
                }

                // Deadzone slider
                Label {
                    text: "Dead Zone"
                    color: "#888"; font.pixelSize: 11
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 8
                }
                RowLayout {
                    Layout.fillWidth: true; spacing: 5
                    Slider {
                        id: dzSlider
                        from: 0; to: 50; stepSize: 1
                        value: axisIndex === 0 ? profileModel.left_stick_deadzone : profileModel.right_stick_deadzone
                        Layout.fillWidth: true
                        onMoved: profileModel.set_stick_deadzone(axisIndex === 0 ? 0 : 1, Math.round(value))
                    }
                    Label {
                        text: Math.round(dzSlider.value) + "%"
                        color: "#888"; font.pixelSize: 11
                        Layout.preferredWidth: 35
                    }
                }

                // Axis inversion toggles
                Label {
                    text: "Inversion"
                    color: "#888"; font.pixelSize: 11
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 8
                }

                CheckBox {
                    id: invertX
                    text: "Invert X"
                    enabled: profileModel.is_usb && profileModel.hw_profile > 0
                    checked: {
                        var _v = invVersion
                        return axisIndex === 0 ? (stickInversion.lx || false) : (stickInversion.rx || false)
                    }
                    onClicked: {
                        if (axisIndex === 0) stickInversion.lx = checked
                        else stickInversion.rx = checked
                        invVersion++
                        profileModel.set_stick_invert(axisIndex === 0 ? 0 : 1, 0, checked)
                    }
                    contentItem: Text {
                        text: parent.text; color: parent.enabled ? "#e0e0e0" : "#555"
                        font.pixelSize: 11
                        leftPadding: parent.indicator.width + 6
                        verticalAlignment: Text.AlignVCenter
                    }
                }

                CheckBox {
                    id: invertY
                    text: "Invert Y"
                    enabled: profileModel.is_usb && profileModel.hw_profile > 0
                    checked: {
                        var _v = invVersion
                        return axisIndex === 0 ? (stickInversion.ly || false) : (stickInversion.ry || false)
                    }
                    onClicked: {
                        if (axisIndex === 0) stickInversion.ly = checked
                        else stickInversion.ry = checked
                        invVersion++
                        profileModel.set_stick_invert(axisIndex === 0 ? 0 : 1, 1, checked)
                    }
                    contentItem: Text {
                        text: parent.text; color: parent.enabled ? "#e0e0e0" : "#555"
                        font.pixelSize: 11
                        leftPadding: parent.indicator.width + 6
                        verticalAlignment: Text.AlignVCenter
                    }
                }
            }
        }

        // Right: curve editor
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 10

            Label {
                text: (axisIndex === 0 ? "Left" : "Right") + " Stick Response Curves"
                font.pixelSize: 18; font.bold: true; color: "#e0e0e0"
            }

            RowLayout {
                spacing: 8
                Repeater {
                    model: axisNames
                    Button {
                        required property int index
                        required property string modelData
                        text: modelData
                        onClicked: { selectedAxis = axisIndex + index; loadCurve() }
                        background: Rectangle {
                            color: selectedAxis === (axisIndex + parent.index) ? "#533483" : "#111"
                            radius: 4
                        }
                        contentItem: Text {
                            text: parent.text
                            color: selectedAxis === (axisIndex + parent.index) ? "#fff" : "#888"
                            horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
                        }
                    }
                }
                Item { Layout.fillWidth: true }
                Button {
                    text: "Reset Linear"
                    onClicked: { resetToLinear(); saveCurve() }
                    background: Rectangle { color: parent.hovered ? "#e74c3c" : "#222"; radius: 4 }
                    contentItem: Text { text: parent.text; color: "#e0e0e0"; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                }
            }

            // Curve canvas
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: "#0a0a0a"
                radius: 8
                border.color: "#333"

                Canvas {
                    id: canvas
                    anchors.fill: parent
                    anchors.margins: 20

                    onPaint: {
                        var ctx = getContext("2d")
                        var w = width, h = height

                        ctx.clearRect(0, 0, w, h)

                        // Grid
                        ctx.strokeStyle = "#1a1a1a"; ctx.lineWidth = 1
                        for (var i = 0; i <= 4; i++) {
                            ctx.beginPath(); ctx.moveTo(i*w/4, 0); ctx.lineTo(i*w/4, h); ctx.stroke()
                            ctx.beginPath(); ctx.moveTo(0, i*h/4); ctx.lineTo(w, i*h/4); ctx.stroke()
                        }

                        // Linear ref
                        ctx.strokeStyle = "#333"; ctx.lineWidth = 1; ctx.setLineDash([5,5])
                        ctx.beginPath(); ctx.moveTo(0, h); ctx.lineTo(w, 0); ctx.stroke()
                        ctx.setLineDash([])

                        // Curve
                        if (curvePoints.length === 16) {
                            ctx.strokeStyle = "#107c10"; ctx.lineWidth = 3
                            ctx.beginPath()
                            for (var j = 0; j < 16; j++) {
                                var px = j * w / 15
                                var py = h - (curvePoints[j] / 32767) * h
                                if (j === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py)
                            }
                            ctx.stroke()

                            // Points
                            for (var k = 0; k < 16; k++) {
                                var cpx = k * w / 15
                                var cpy = h - (curvePoints[k] / 32767) * h
                                ctx.fillStyle = "#533483"
                                ctx.beginPath(); ctx.arc(cpx, cpy, 5, 0, Math.PI*2); ctx.fill()
                            }

                            // Live input indicator
                            var rawAxis = selectedAxis === axisIndex ? liveX : liveY
                            var absInput = Math.abs(rawAxis)
                            var ix = absInput * w
                            ctx.strokeStyle = "#0078d4"; ctx.lineWidth = 1
                            ctx.setLineDash([3,3])
                            ctx.beginPath(); ctx.moveTo(ix, 0); ctx.lineTo(ix, h); ctx.stroke()
                            ctx.setLineDash([])
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        property int dragIndex: -1
                        onPressed: function(mouse) {
                            var closest = -1, minDist = 20
                            for (var i = 0; i < 16; i++) {
                                var px = i * width / 15
                                var py = height - (curvePoints[i] / 32767) * height
                                var d = Math.sqrt(Math.pow(mouse.x-px,2) + Math.pow(mouse.y-py,2))
                                if (d < minDist) { minDist = d; closest = i }
                            }
                            dragIndex = closest
                        }
                        onPositionChanged: function(mouse) {
                            if (dragIndex >= 0 && dragIndex < 16) {
                                var val = Math.round((1 - mouse.y / height) * 32767)
                                val = Math.max(0, Math.min(32767, val))
                                var pts = curvePoints.slice(); pts[dragIndex] = val; curvePoints = pts
                                canvas.requestPaint()
                            }
                        }
                        onReleased: { if (dragIndex >= 0) saveCurve(); dragIndex = -1 }
                    }
                }

                Label { anchors.bottom: parent.bottom; anchors.horizontalCenter: parent.horizontalCenter; text: "Input"; color: "#555"; font.pixelSize: 10 }
                Label { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: "Output"; color: "#555"; font.pixelSize: 10; rotation: -90 }
            }
        }
    }
}
