import QtQuick

// A compact, always-on-top overlay showing the currently active layer. Launched
// with `padproxy --overlay layer`. The `layerName` property holds the displayed
// layer (default "Main") and can be driven from the backend's active layer.
Window {
    id: overlay
    flags: Qt.FramelessWindowHint
        | Qt.WindowStaysOnTopHint
        | Qt.WindowTransparentForInput
    color: "transparent"
    visible: true
    width: badge.width
    height: badge.height
    x: 40
    y: 40

    property string layerName: "Main"

    Rectangle {
        id: badge
        width: label.implicitWidth + 28
        height: label.implicitHeight + 16
        radius: 8
        color: "#cc1b1e26"
        border.color: "#00ff66"
        border.width: 2

        Row {
            anchors.centerIn: parent
            spacing: 8

            Rectangle {
                width: 10
                height: 10
                radius: 5
                anchors.verticalCenter: parent.verticalCenter
                color: "#00ff66"
            }

            Text {
                id: label
                text: "Layer: " + overlay.layerName
                color: "white"
                font.pixelSize: 18
                font.bold: true
            }
        }
    }
}
