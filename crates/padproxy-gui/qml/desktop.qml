import QtQuick

// A desktop status overlay (HUD): an always-on-top, click-through panel showing
// the remap on/off state and active profile. Launched with
// `padproxy --overlay desktop`. The `remapActive` and `profileName` properties
// can be driven from the backend.
Window {
    id: overlay
    flags: Qt.FramelessWindowHint
        | Qt.WindowStaysOnTopHint
        | Qt.WindowTransparentForInput
    color: "transparent"
    visible: true
    width: panel.width
    height: panel.height
    x: 40
    y: 40

    property bool remapActive: true
    property string profileName: "No profile"

    Rectangle {
        id: panel
        width: column.implicitWidth + 32
        height: column.implicitHeight + 24
        radius: 10
        color: "#cc12151c"
        border.color: overlay.remapActive ? "#00ff66" : "#ff5a5a"
        border.width: 2

        Column {
            id: column
            anchors.centerIn: parent
            spacing: 6

            Row {
                spacing: 10
                Rectangle {
                    width: 14
                    height: 14
                    radius: 7
                    anchors.verticalCenter: parent.verticalCenter
                    color: overlay.remapActive ? "#00ff66" : "#ff5a5a"
                }
                Text {
                    text: "PadProxy — Remap " + (overlay.remapActive ? "ON" : "OFF")
                    color: "white"
                    font.pixelSize: 18
                    font.bold: true
                }
            }

            Text {
                text: "Profile: " + overlay.profileName
                color: "#c8cdd6"
                font.pixelSize: 14
            }
        }
    }
}
