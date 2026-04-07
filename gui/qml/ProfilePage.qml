import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    required property var profileModel

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 20

        Label {
            text: "Hardware Profile Mapping"
            font.pixelSize: 20
            font.bold: true
            color: "#e0e0e0"
        }

        Label {
            text: "Map the controller's physical profile switch (0-3) to your software profiles.\n" +
                  "Profile 0 = default (no LED), 1-3 = LED profiles."
            color: "#888"
            font.pixelSize: 12
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }

        GroupBox {
            Layout.fillWidth: true
            title: "Profile Switch Mapping"
            label: Label {
                text: parent.title
                color: "#e0e0e0"
                font.pixelSize: 14
                font.bold: true
            }
            background: Rectangle {
                color: "#0f3460"
                radius: 8
                border.color: "#533483"
                y: 30
            }

            GridLayout {
                columns: 3
                columnSpacing: 20
                rowSpacing: 15

                Repeater {
                    model: 4

                    delegate: RowLayout {
                        Layout.columnSpan: 3
                        Layout.fillWidth: true

                        Rectangle {
                            Layout.preferredWidth: 20
                            Layout.preferredHeight: 20
                            radius: 10
                            color: index === profileModel.hw_profile ? "#107c10" : "#333"
                        }

                        Label {
                            text: "HW Profile " + index + (index === 0 ? " (Default)" : "")
                            color: "#e0e0e0"
                            font.pixelSize: 14
                            Layout.preferredWidth: 200
                        }

                        Label {
                            text: " -> "
                            color: "#533483"
                            font.bold: true
                        }

                        ComboBox {
                            Layout.fillWidth: true
                            model: {
                                try {
                                    return JSON.parse(profileModel.get_profile_names())
                                } catch(e) {
                                    return ["Default"]
                                }
                            }
                            onCurrentIndexChanged: {
                                profileModel.set_hw_profile_mapping(index, currentIndex)
                            }
                            background: Rectangle {
                                color: "#16213e"
                                radius: 4
                                border.color: "#533483"
                            }
                            contentItem: Text {
                                text: parent.displayText
                                color: "#e0e0e0"
                                leftPadding: 8
                                verticalAlignment: Text.AlignVCenter
                            }
                        }
                    }
                }
            }
        }

        // Profile list management
        GroupBox {
            Layout.fillWidth: true
            Layout.fillHeight: true
            title: "Software Profiles"
            label: Label {
                text: parent.title
                color: "#e0e0e0"
                font.pixelSize: 14
                font.bold: true
            }
            background: Rectangle {
                color: "#0f3460"
                radius: 8
                border.color: "#533483"
                y: 30
            }

            ListView {
                anchors.fill: parent
                clip: true
                model: {
                    try {
                        return JSON.parse(profileModel.get_profile_names())
                    } catch(e) {
                        return []
                    }
                }

                delegate: Rectangle {
                    width: parent ? parent.width : 200
                    height: 44
                    color: index === profileModel.active_profile ? "#533483" : (index % 2 === 0 ? "#16213e" : "#0f3460")
                    radius: 4

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 8

                        Label {
                            text: (index + 1) + ". " + modelData
                            color: "#e0e0e0"
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }

                        Button {
                            text: "Select"
                            visible: index !== profileModel.active_profile
                            onClicked: profileModel.select_profile(index)
                            background: Rectangle {
                                color: parent.hovered ? "#533483" : "transparent"
                                radius: 4
                                border.color: "#533483"
                            }
                            contentItem: Text {
                                text: parent.text
                                color: "#e0e0e0"
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                                font.pixelSize: 12
                            }
                        }

                        Button {
                            text: "Delete"
                            visible: profileModel.profile_count > 1
                            onClicked: {
                                profileModel.select_profile(index)
                                profileModel.delete_profile()
                            }
                            background: Rectangle {
                                color: parent.hovered ? "#e74c3c" : "transparent"
                                radius: 4
                                border.color: "#e74c3c"
                            }
                            contentItem: Text {
                                text: parent.text
                                color: "#e74c3c"
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                                font.pixelSize: 12
                            }
                        }
                    }
                }
            }
        }
    }
}
