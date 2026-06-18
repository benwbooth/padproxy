import QtQuick

// A transparent, always-on-top, click-through crosshair overlay drawn at the
// center of the screen. Launched with `padproxy --overlay crosshair`.
Window {
    id: overlay
    visible: true
    visibility: Window.FullScreen
    color: "transparent"
    flags: Qt.FramelessWindowHint
        | Qt.WindowStaysOnTopHint
        | Qt.WindowTransparentForInput

    // Crosshair appearance.
    property color lineColor: "#00ff66"
    property int gap: 6
    property int armLength: 18
    property int thickness: 2

    Canvas {
        id: canvas
        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();

            const cx = width / 2;
            const cy = height / 2;

            ctx.strokeStyle = overlay.lineColor;
            ctx.lineWidth = overlay.thickness;
            ctx.beginPath();
            // Left / right arms.
            ctx.moveTo(cx - overlay.gap - overlay.armLength, cy);
            ctx.lineTo(cx - overlay.gap, cy);
            ctx.moveTo(cx + overlay.gap, cy);
            ctx.lineTo(cx + overlay.gap + overlay.armLength, cy);
            // Top / bottom arms.
            ctx.moveTo(cx, cy - overlay.gap - overlay.armLength);
            ctx.lineTo(cx, cy - overlay.gap);
            ctx.moveTo(cx, cy + overlay.gap);
            ctx.lineTo(cx, cy + overlay.gap + overlay.armLength);
            ctx.stroke();

            // Center dot.
            ctx.fillStyle = overlay.lineColor;
            ctx.beginPath();
            ctx.arc(cx, cy, overlay.thickness, 0, Math.PI * 2);
            ctx.fill();
        }
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
    }
}
