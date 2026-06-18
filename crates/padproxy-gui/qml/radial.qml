import QtQuick

// A radial command menu overlay: a ring of labelled segments around a center
// hub. Launched with `padproxy --overlay radial`. The `items` property holds
// the segment labels (an eight-way set by default) and can be bound to a
// profile's commands when wired to the backend.
Window {
    id: overlay
    visible: true
    visibility: Window.FullScreen
    color: "#88000000"
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    property var items: defaultItems()
    function defaultItems() {
        return ["Up", "Up-Right", "Right", "Down-Right",
                "Down", "Down-Left", "Left", "Up-Left"];
    }

    property real radius: Math.min(width, height) * 0.22
    property real hubRadius: radius * 0.42

    Item {
        anchors.centerIn: parent
        width: overlay.radius * 3
        height: overlay.radius * 3

        // Outer ring segments.
        Repeater {
            model: overlay.items.length
            delegate: Item {
                property real angle: (index / overlay.items.length) * 2 * Math.PI - Math.PI / 2
                x: parent.width / 2 + Math.cos(angle) * overlay.radius - width / 2
                y: parent.height / 2 + Math.sin(angle) * overlay.radius - height / 2
                width: overlay.radius * 0.9
                height: overlay.radius * 0.55

                Rectangle {
                    anchors.fill: parent
                    radius: height / 2
                    color: "#2a2f3a"
                    border.color: "#00ff66"
                    border.width: 2

                    Text {
                        anchors.centerIn: parent
                        text: overlay.items[index]
                        color: "white"
                        font.pixelSize: Math.max(14, overlay.radius * 0.12)
                    }
                }
            }
        }

        // Center hub.
        Rectangle {
            anchors.centerIn: parent
            width: overlay.hubRadius * 2
            height: overlay.hubRadius * 2
            radius: overlay.hubRadius
            color: "#1b1e26"
            border.color: "#00ff66"
            border.width: 3

            Text {
                anchors.centerIn: parent
                text: "PadProxy"
                color: "#00ff66"
                font.pixelSize: Math.max(14, overlay.hubRadius * 0.28)
                font.bold: true
            }
        }
    }

    // Press Escape to dismiss.
    Item {
        anchors.fill: parent
        focus: true
        Keys.onEscapePressed: Qt.quit()
    }
}
