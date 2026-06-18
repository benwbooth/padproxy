import QtQuick
import QtMultimedia

// A screen magnifier overlay: captures the screen and shows a zoomed view of
// the region around the screen center. Launched with `padproxy --overlay
// magnifier`. Screen capture uses Qt's QScreenCapture (works on X11 and on
// Wayland where a screencast portal is available).
Window {
    id: overlay
    visible: true
    width: 480
    height: 480
    x: 80
    y: 80
    color: "#111418"
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    // Magnification factor and the screen point to zoom into (center by default).
    property real zoom: 3.0
    property real focusX: 0.5
    property real focusY: 0.5

    ScreenCapture {
        id: screenCapture
        active: true
    }

    CaptureSession {
        screenCapture: screenCapture
        videoOutput: videoOutput
    }

    Item {
        anchors.fill: parent
        clip: true

        // The captured screen, scaled up; positioned so the focus point sits at
        // the center of the magnifier window.
        VideoOutput {
            id: videoOutput
            property real sourceW: implicitWidth > 0 ? implicitWidth : overlay.width
            property real sourceH: implicitHeight > 0 ? implicitHeight : overlay.height
            width: sourceW * overlay.zoom
            height: sourceH * overlay.zoom
            x: overlay.width / 2 - overlay.focusX * width
            y: overlay.height / 2 - overlay.focusY * height
            fillMode: VideoOutput.PreserveAspectFit
        }

        // Crosshair marking the magnified focus point.
        Rectangle {
            anchors.centerIn: parent
            width: 2
            height: 16
            color: "#00ff66"
        }
        Rectangle {
            anchors.centerIn: parent
            width: 16
            height: 2
            color: "#00ff66"
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.color: "#00ff66"
        border.width: 2
    }

    Item {
        anchors.fill: parent
        focus: true
        Keys.onEscapePressed: Qt.quit()
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal)
                overlay.zoom = Math.min(overlay.zoom + 0.5, 10);
            else if (event.key === Qt.Key_Minus)
                overlay.zoom = Math.max(overlay.zoom - 0.5, 1);
        }
    }
}
