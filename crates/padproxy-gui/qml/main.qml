import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.benwbooth.padproxy

ApplicationWindow {
    id: root
    width: 1320
    height: 720
    visible: true
    title: "PadProxy"

    Component.onCompleted: Qt.callLater(function() {
        root.show()
        root.raise()
        root.requestActivate()
    })

    PadProxyController {
        id: backend
        Component.onCompleted: refresh()
    }

    function parseJson(text) {
        if (!text || text.length === 0)
            return []
        try {
            return JSON.parse(text)
        } catch (error) {
            return []
        }
    }

    property var devicesModel: parseJson(backend.devices)
    property var profilesModel: parseJson(backend.profiles)
    property var selectedProfile: profilesModel.length > 0 && profileList.currentIndex >= 0
        ? profilesModel[profileList.currentIndex]
        : null
    property bool editorDirty: false
    property int selectedMappingIndex: -1
    property string selectedMappingSide: "from"
    property bool hookListening: false
    property string hookStatus: backend.capture_status
    property var eventCodes: [
        "btn:south",
        "btn:east",
        "btn:west",
        "btn:north",
        "btn:tl",
        "btn:tr",
        "btn:tl2",
        "btn:tr2",
        "btn:select",
        "btn:start",
        "btn:mode",
        "btn:thumbl",
        "btn:thumbr",
        "abs:x",
        "abs:y",
        "abs:rx",
        "abs:ry",
        "abs:z",
        "abs:rz",
        "abs:hat0x",
        "abs:hat0y"
    ]
    property var controllerTemplates: [
        {
            name: "Xbox",
            image: "qrc:/qt/qml/com/benwbooth/padproxy/qml/images/controller-xbox.svg",
            controls: [
                { code: "btn:tl2", label: "LT", x: 0.22, y: 0.10, w: 0.14, h: 0.11 },
                { code: "btn:tr2", label: "RT", x: 0.78, y: 0.10, w: 0.14, h: 0.11 },
                { code: "btn:tl", label: "LB", x: 0.25, y: 0.22, w: 0.14, h: 0.10 },
                { code: "btn:tr", label: "RB", x: 0.75, y: 0.22, w: 0.14, h: 0.10 },
                { code: "abs:x", label: "LS X", x: 0.28, y: 0.43, w: 0.12, h: 0.11 },
                { code: "abs:y", label: "LS Y", x: 0.28, y: 0.56, w: 0.12, h: 0.11 },
                { code: "btn:thumbl", label: "L3", x: 0.28, y: 0.69, w: 0.11, h: 0.10 },
                { code: "abs:hat0x", label: "D-X", x: 0.42, y: 0.62, w: 0.11, h: 0.10 },
                { code: "abs:hat0y", label: "D-Y", x: 0.42, y: 0.75, w: 0.11, h: 0.10 },
                { code: "btn:select", label: "View", x: 0.43, y: 0.43, w: 0.10, h: 0.09 },
                { code: "btn:mode", label: "Guide", x: 0.50, y: 0.52, w: 0.10, h: 0.09 },
                { code: "btn:start", label: "Menu", x: 0.57, y: 0.43, w: 0.10, h: 0.09 },
                { code: "abs:rx", label: "RS X", x: 0.59, y: 0.67, w: 0.12, h: 0.11 },
                { code: "abs:ry", label: "RS Y", x: 0.59, y: 0.80, w: 0.12, h: 0.11 },
                { code: "btn:thumbr", label: "R3", x: 0.72, y: 0.75, w: 0.11, h: 0.10 },
                { code: "btn:west", label: "X", x: 0.72, y: 0.43, w: 0.08, h: 0.08 },
                { code: "btn:north", label: "Y", x: 0.80, y: 0.34, w: 0.08, h: 0.08 },
                { code: "btn:east", label: "B", x: 0.88, y: 0.43, w: 0.08, h: 0.08 },
                { code: "btn:south", label: "A", x: 0.80, y: 0.52, w: 0.08, h: 0.08 }
            ]
        },
        {
            name: "PlayStation",
            image: "qrc:/qt/qml/com/benwbooth/padproxy/qml/images/controller-playstation.svg",
            controls: [
                { code: "btn:tl2", label: "L2", x: 0.22, y: 0.10, w: 0.14, h: 0.11 },
                { code: "btn:tr2", label: "R2", x: 0.78, y: 0.10, w: 0.14, h: 0.11 },
                { code: "btn:tl", label: "L1", x: 0.25, y: 0.22, w: 0.14, h: 0.10 },
                { code: "btn:tr", label: "R1", x: 0.75, y: 0.22, w: 0.14, h: 0.10 },
                { code: "abs:x", label: "LS X", x: 0.31, y: 0.58, w: 0.12, h: 0.11 },
                { code: "abs:y", label: "LS Y", x: 0.31, y: 0.71, w: 0.12, h: 0.11 },
                { code: "btn:thumbl", label: "L3", x: 0.31, y: 0.84, w: 0.11, h: 0.10 },
                { code: "abs:hat0x", label: "D-X", x: 0.23, y: 0.43, w: 0.11, h: 0.10 },
                { code: "abs:hat0y", label: "D-Y", x: 0.23, y: 0.56, w: 0.11, h: 0.10 },
                { code: "btn:select", label: "Share", x: 0.42, y: 0.45, w: 0.10, h: 0.09 },
                { code: "btn:mode", label: "PS", x: 0.50, y: 0.58, w: 0.10, h: 0.09 },
                { code: "btn:start", label: "Options", x: 0.58, y: 0.45, w: 0.10, h: 0.09 },
                { code: "abs:rx", label: "RS X", x: 0.69, y: 0.58, w: 0.12, h: 0.11 },
                { code: "abs:ry", label: "RS Y", x: 0.69, y: 0.71, w: 0.12, h: 0.11 },
                { code: "btn:thumbr", label: "R3", x: 0.69, y: 0.84, w: 0.11, h: 0.10 },
                { code: "btn:west", label: "Square", x: 0.72, y: 0.43, w: 0.09, h: 0.08 },
                { code: "btn:north", label: "Triangle", x: 0.80, y: 0.34, w: 0.09, h: 0.08 },
                { code: "btn:east", label: "Circle", x: 0.88, y: 0.43, w: 0.09, h: 0.08 },
                { code: "btn:south", label: "Cross", x: 0.80, y: 0.52, w: 0.09, h: 0.08 }
            ]
        },
        {
            name: "Generic",
            image: "qrc:/qt/qml/com/benwbooth/padproxy/qml/images/controller-generic.svg",
            controls: [
                { code: "btn:tl2", label: "L2", x: 0.22, y: 0.10, w: 0.14, h: 0.11 },
                { code: "btn:tr2", label: "R2", x: 0.78, y: 0.10, w: 0.14, h: 0.11 },
                { code: "btn:tl", label: "L1", x: 0.25, y: 0.22, w: 0.14, h: 0.10 },
                { code: "btn:tr", label: "R1", x: 0.75, y: 0.22, w: 0.14, h: 0.10 },
                { code: "abs:x", label: "Left X", x: 0.29, y: 0.55, w: 0.12, h: 0.11 },
                { code: "abs:y", label: "Left Y", x: 0.29, y: 0.68, w: 0.12, h: 0.11 },
                { code: "btn:thumbl", label: "L3", x: 0.29, y: 0.81, w: 0.11, h: 0.10 },
                { code: "abs:hat0x", label: "D-X", x: 0.41, y: 0.65, w: 0.11, h: 0.10 },
                { code: "abs:hat0y", label: "D-Y", x: 0.41, y: 0.78, w: 0.11, h: 0.10 },
                { code: "btn:select", label: "Select", x: 0.43, y: 0.45, w: 0.10, h: 0.09 },
                { code: "btn:mode", label: "Home", x: 0.50, y: 0.55, w: 0.10, h: 0.09 },
                { code: "btn:start", label: "Start", x: 0.57, y: 0.45, w: 0.10, h: 0.09 },
                { code: "abs:rx", label: "Right X", x: 0.62, y: 0.65, w: 0.12, h: 0.11 },
                { code: "abs:ry", label: "Right Y", x: 0.62, y: 0.78, w: 0.12, h: 0.11 },
                { code: "btn:thumbr", label: "R3", x: 0.73, y: 0.81, w: 0.11, h: 0.10 },
                { code: "btn:west", label: "West", x: 0.72, y: 0.43, w: 0.09, h: 0.08 },
                { code: "btn:north", label: "North", x: 0.80, y: 0.34, w: 0.09, h: 0.08 },
                { code: "btn:east", label: "East", x: 0.88, y: 0.43, w: 0.09, h: 0.08 },
                { code: "btn:south", label: "South", x: 0.80, y: 0.52, w: 0.09, h: 0.08 }
            ]
        }
    ]
    property var controllerTemplateNames: controllerTemplates.map(function(template) { return template.name })

    onProfilesModelChanged: Qt.callLater(function() {
        if (root.profilesModel.length > 0 && backend.profile_yaml.length === 0)
            root.selectProfile(profileList.currentIndex >= 0 ? profileList.currentIndex : 0)
    })

    onDevicesModelChanged: Qt.callLater(function() {
        if (root.devicesModel.length > 0 && deviceList.currentIndex < 0)
            deviceList.currentIndex = 0
    })

    function hexId(value) {
        return Number(value).toString(16).padStart(4, "0")
    }

    function capabilitiesText(values) {
        return values && values.length > 0 ? values.join(", ") : "no mapped capabilities"
    }

    function selectedDevice() {
        if (deviceList.currentIndex < 0 || deviceList.currentIndex >= root.devicesModel.length)
            return null
        return root.devicesModel[deviceList.currentIndex]
    }

    function eventLabel(code) {
        switch (code) {
        case "btn:south": return "South / A / Cross"
        case "btn:east": return "East / B / Circle"
        case "btn:west": return "West / X / Square"
        case "btn:north": return "North / Y / Triangle"
        case "btn:tl": return "L1 / LB"
        case "btn:tr": return "R1 / RB"
        case "btn:tl2": return "L2 / LT"
        case "btn:tr2": return "R2 / RT"
        case "btn:select": return "Select / View / Share"
        case "btn:start": return "Start / Menu / Options"
        case "btn:mode": return "Home / Guide / PS"
        case "btn:thumbl": return "L3"
        case "btn:thumbr": return "R3"
        case "abs:x": return "Left stick X"
        case "abs:y": return "Left stick Y"
        case "abs:rx": return "Right stick X"
        case "abs:ry": return "Right stick Y"
        case "abs:z": return "Left trigger axis"
        case "abs:rz": return "Right trigger axis"
        case "abs:hat0x": return "D-pad X"
        case "abs:hat0y": return "D-pad Y"
        default: return code
        }
    }

    function currentControllerTemplate(index) {
        if (index < 0 || index >= root.controllerTemplates.length)
            return root.controllerTemplates[0]
        return root.controllerTemplates[index]
    }

    function selectedMapping() {
        if (root.selectedMappingIndex < 0 || root.selectedMappingIndex >= mappingsModel.count)
            return null
        return mappingsModel.get(root.selectedMappingIndex)
    }

    function ensureMappingSelection(defaultCode) {
        if (mappingsModel.count === 0)
            root.addMapping(defaultCode || "btn:south", defaultCode || "btn:south")
        if (root.selectedMappingIndex < 0 || root.selectedMappingIndex >= mappingsModel.count)
            root.selectedMappingIndex = 0
    }

    function addMapping(fromCode, toCode) {
        mappingsModel.append({ fromCode: fromCode, toCode: toCode })
        root.selectedMappingIndex = mappingsModel.count - 1
    }

    function updateSelectedMapping(code) {
        root.ensureMappingSelection(code)
        const propertyName = root.selectedMappingSide === "to" ? "toCode" : "fromCode"
        mappingsModel.setProperty(root.selectedMappingIndex, propertyName, code)
    }

    function startHook() {
        root.ensureMappingSelection("btn:south")
        const device = root.selectedDevice()
        if (!device) {
            root.hookStatus = "Select a controller first."
            return
        }

        const result = backend.start_capture(device.path)
        root.hookStatus = backend.capture_status
        if (result === "ok") {
            root.hookListening = true
            hookTimer.restart()
        }
    }

    function stopHook() {
        hookTimer.stop()
        root.hookListening = false
        backend.stop_capture()
        root.hookStatus = backend.capture_status
    }

    function isSelectedMappingControl(code) {
        const row = root.selectedMapping()
        if (!row)
            return false
        return root.selectedMappingSide === "to" ? row.toCode === code : row.fromCode === code
    }

    function mappingCountFor(code, side) {
        let total = 0
        const propertyName = side === "to" ? "toCode" : "fromCode"
        for (let i = 0; i < mappingsModel.count; i++) {
            if (mappingsModel.get(i)[propertyName] === code)
                total += 1
        }
        return total
    }

    function selectProfile(index) {
        profileList.currentIndex = index
        if (root.selectedProfile) {
            root.loadStructuredProfile(root.selectedProfile)
            backend.edit_profile(root.selectedProfile.source_path)
        }
    }

    function newStructuredProfile() {
        profileList.currentIndex = -1
        profileIdField.text = "my-layout"
        profileNameField.text = "My layout"
        profileDescriptionField.text = "Custom controller mapping."
        matchNameField.text = "*"
        outputTypeBox.currentIndex = 0
        passthroughCheck.checked = true
        grabSourceCheck.checked = true
        mappingsModel.clear()
        root.addMapping("btn:west", "btn:east")
        root.addMapping("btn:east", "btn:west")
        root.selectedMappingIndex = 0
        backend.new_profile()
    }

    function loadStructuredProfile(profile) {
        profileIdField.text = profile.id || ""
        profileNameField.text = profile.name || ""
        profileDescriptionField.text = profile.description || ""
        matchNameField.text = profile.device_match && profile.device_match.name ? profile.device_match.name : "*"
        outputTypeBox.currentIndex = Math.max(0, outputTypeBox.model.indexOf(profile.output_type || "xbox360"))
        passthroughCheck.checked = profile.passthrough !== false
        grabSourceCheck.checked = profile.grab_source !== false
        mappingsModel.clear()
        if (profile.mappings) {
            for (let i = 0; i < profile.mappings.length; i++) {
                mappingsModel.append({
                    fromCode: profile.mappings[i].from_name,
                    toCode: profile.mappings[i].to_name
                })
            }
        }
        root.selectedMappingIndex = mappingsModel.count > 0 ? 0 : -1
    }

    function yamlString(value) {
        return "\"" + String(value).replace(/\\/g, "\\\\").replace(/"/g, "\\\"") + "\""
    }

    function structuredProfileYaml() {
        let text = ""
        text += "id: " + yamlString(profileIdField.text.trim() || "my-layout") + "\n"
        text += "name: " + yamlString(profileNameField.text.trim() || profileIdField.text.trim() || "My layout") + "\n"
        text += "description: " + yamlString(profileDescriptionField.text) + "\n"
        text += "match:\n"
        text += "  name: " + yamlString(matchNameField.text.trim() || "*") + "\n"
        text += "output:\n"
        text += "  type: " + outputTypeBox.currentText + "\n"
        text += "passthrough: " + (passthroughCheck.checked ? "true" : "false") + "\n"
        text += "grab_source: " + (grabSourceCheck.checked ? "true" : "false") + "\n"
        if (mappingsModel.count === 0) {
            text += "mappings: []\n"
        } else {
            text += "mappings:\n"
            for (let i = 0; i < mappingsModel.count; i++) {
                const row = mappingsModel.get(i)
                text += "  - from: " + row.fromCode + "\n"
                text += "    to: " + row.toCode + "\n"
            }
        }
        return text
    }

    function saveCurrentEditor() {
        if (editorTabs.currentIndex === 0)
            backend.save_profile(root.structuredProfileYaml())
        else
            backend.save_profile(profileEditor.text)
    }

    Connections {
        target: backend

        function onProfile_yamlChanged() {
            profileEditor.text = backend.profile_yaml
            root.editorDirty = false
        }
    }

    ListModel {
        id: mappingsModel
    }

    Timer {
        id: hookTimer
        interval: 35
        repeat: true

        onTriggered: {
            const code = backend.poll_capture_event()
            if (!code || code.length === 0) {
                root.hookStatus = backend.capture_status
                return
            }

            if (root.eventCodes.indexOf(code) >= 0)
                root.updateSelectedMapping(code)
            root.stopHook()
        }
    }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: 12

            Label {
                text: "PadProxy"
                font.pixelSize: 18
                font.bold: true
                Layout.leftMargin: 12
            }

            Button {
                text: "Refresh"
                onClicked: backend.refresh()
            }

            Label {
                text: backend.status
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }

    SplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal

        Pane {
            SplitView.preferredWidth: 440
            SplitView.minimumWidth: 340

            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                Label {
                    text: "Controllers"
                    font.bold: true
                }

                ListView {
                    id: deviceList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: root.devicesModel

                    delegate: ItemDelegate {
                        width: ListView.view.width
                        text: "[" + modelData.device_kind + "] " + modelData.name
                            + "\n" + modelData.path + "  " + hexId(modelData.vendor) + ":" + hexId(modelData.product)
                            + "\n" + capabilitiesText(modelData.capabilities)
                        highlighted: ListView.isCurrentItem
                        onClicked: deviceList.currentIndex = index
                    }
                }
            }
        }

        Pane {
            SplitView.preferredWidth: 360
            SplitView.minimumWidth: 280

            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                Label {
                    text: "Profiles"
                    font.bold: true
                }

                RowLayout {
                    Layout.fillWidth: true

                    Button {
                        text: "New"
                        onClicked: root.newStructuredProfile()
                    }

                    Button {
                        text: "Edit"
                        enabled: root.selectedProfile !== null
                        onClicked: backend.edit_profile(root.selectedProfile.source_path)
                    }
                }

                ListView {
                    id: profileList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: root.profilesModel

                    delegate: ItemDelegate {
                        width: ListView.view.width
                        text: modelData.name + "\n" + modelData.output_type + "  " + modelData.source_path
                        highlighted: ListView.isCurrentItem
                        onClicked: root.selectProfile(index)
                    }
                }
            }
        }

        Pane {
            SplitView.fillWidth: true
            SplitView.minimumWidth: 420

            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true

                    Label {
                        text: selectedProfile ? selectedProfile.name : "Profile Editor"
                        font.bold: true
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }

                    Button {
                        text: "Save"
                        enabled: editorTabs.currentIndex === 0
                            ? profileIdField.text.trim().length > 0
                            : profileEditor.text.length > 0
                        onClicked: root.saveCurrentEditor()
                    }
                }

                Label {
                    text: backend.editing_profile_path.length > 0
                        ? backend.editing_profile_path
                        : "No profile loaded."
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }

                TabBar {
                    id: editorTabs
                    Layout.fillWidth: true

                    TabButton {
                        text: "Mappings"
                    }

                    TabButton {
                        text: "YAML"
                    }
                }

                StackLayout {
                    currentIndex: editorTabs.currentIndex
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    ScrollView {
                        contentWidth: availableWidth

                        ColumnLayout {
                            width: parent.availableWidth
                            spacing: 10

                            GridLayout {
                                columns: 2
                                columnSpacing: 10
                                rowSpacing: 8
                                Layout.fillWidth: true

                                Label { text: "ID" }
                                TextField {
                                    id: profileIdField
                                    Layout.fillWidth: true
                                }

                                Label { text: "Name" }
                                TextField {
                                    id: profileNameField
                                    Layout.fillWidth: true
                                }

                                Label { text: "Description" }
                                TextField {
                                    id: profileDescriptionField
                                    Layout.fillWidth: true
                                }

                                Label { text: "Match" }
                                TextField {
                                    id: matchNameField
                                    Layout.fillWidth: true
                                }

                                Label { text: "Output" }
                                ComboBox {
                                    id: outputTypeBox
                                    model: ["xbox360"]
                                    Layout.fillWidth: true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true

                                CheckBox {
                                    id: passthroughCheck
                                    text: "Passthrough"
                                    checked: true
                                }

                                CheckBox {
                                    id: grabSourceCheck
                                    text: "Grab source"
                                    checked: true
                                }

                                Item {
                                    Layout.fillWidth: true
                                }

                                Button {
                                    text: "Add Mapping"
                                    onClicked: root.addMapping("btn:south", "btn:south")
                                }
                            }

                            Frame {
                                Layout.fillWidth: true
                                Layout.preferredHeight: Math.max(300, width * 0.48)

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 8

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8

                                        Label {
                                            text: "Controller"
                                            font.bold: true
                                        }

                                        ComboBox {
                                            id: controllerTemplateBox
                                            model: root.controllerTemplateNames
                                            Layout.preferredWidth: 180
                                        }

                                        Item {
                                            Layout.fillWidth: true
                                        }

                                        Button {
                                            text: "Source"
                                            checkable: true
                                            checked: root.selectedMappingSide === "from"
                                            onClicked: root.selectedMappingSide = "from"
                                        }

                                        Button {
                                            text: "Target"
                                            checkable: true
                                            checked: root.selectedMappingSide === "to"
                                            onClicked: root.selectedMappingSide = "to"
                                        }

                                        Button {
                                            text: root.hookListening ? "Stop" : "Listen"
                                            enabled: root.selectedDevice() !== null
                                            checkable: true
                                            checked: root.hookListening
                                            onClicked: root.hookListening ? root.stopHook() : root.startHook()
                                        }
                                    }

                                    Rectangle {
                                        id: controllerSurface
                                        Layout.fillWidth: true
                                        Layout.fillHeight: true
                                        color: "#161a1f"
                                        border.color: "#343b44"
                                        radius: 6
                                        clip: true

                                        Image {
                                            anchors.fill: parent
                                            anchors.margins: 8
                                            fillMode: Image.PreserveAspectFit
                                            smooth: true
                                            source: root.currentControllerTemplate(controllerTemplateBox.currentIndex).image
                                        }

                                        Repeater {
                                            model: root.currentControllerTemplate(controllerTemplateBox.currentIndex).controls

                                            delegate: Button {
                                                readonly property bool activeControl: root.isSelectedMappingControl(modelData.code)
                                                readonly property int fromCount: root.mappingCountFor(modelData.code, "from")
                                                readonly property int toCount: root.mappingCountFor(modelData.code, "to")

                                                x: controllerSurface.width * modelData.x - width / 2
                                                y: controllerSurface.height * modelData.y - height / 2
                                                width: Math.max(52, controllerSurface.width * modelData.w)
                                                height: Math.max(32, controllerSurface.height * modelData.h)
                                                text: modelData.label
                                                display: AbstractButton.TextOnly
                                                onClicked: root.updateSelectedMapping(modelData.code)

                                                ToolTip.visible: hovered
                                                ToolTip.text: root.eventLabel(modelData.code)

                                                contentItem: Text {
                                                    text: parent.text
                                                    color: parent.activeControl ? "#111111" : "#f1f5f9"
                                                    font.pixelSize: 12
                                                    font.bold: true
                                                    horizontalAlignment: Text.AlignHCenter
                                                    verticalAlignment: Text.AlignVCenter
                                                    elide: Text.ElideRight
                                                }

                                                background: Rectangle {
                                                    radius: 5
                                                    color: activeControl
                                                        ? "#ffd84d"
                                                        : (fromCount + toCount > 0 ? "#334155" : "#20262d")
                                                    border.width: activeControl ? 2 : 1
                                                    border.color: activeControl
                                                        ? "#f59e0b"
                                                        : (fromCount > 0 && toCount > 0 ? "#89b4fa" : "#56616f")
                                                }

                                                Rectangle {
                                                    visible: parent.fromCount > 0 || parent.toCount > 0
                                                    width: 18
                                                    height: 18
                                                    radius: 9
                                                    anchors.right: parent.right
                                                    anchors.top: parent.top
                                                    anchors.rightMargin: -5
                                                    anchors.topMargin: -5
                                                    color: parent.activeControl ? "#111111" : "#ffd84d"
                                                    border.color: "#111111"

                                                    Text {
                                                        anchors.centerIn: parent
                                                        text: parent.parent.fromCount + parent.parent.toCount
                                                        color: parent.parent.activeControl ? "#ffd84d" : "#111111"
                                                        font.pixelSize: 10
                                                        font.bold: true
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    Label {
                                        text: root.selectedMapping()
                                            ? "Row " + (root.selectedMappingIndex + 1) + ": "
                                                + root.eventLabel(root.selectedMapping().fromCode)
                                                + " -> "
                                                + root.eventLabel(root.selectedMapping().toCode)
                                            : "No mapping row"
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }

                                    Label {
                                        text: root.hookStatus
                                        visible: text.length > 0
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                }
                            }

                            Frame {
                                Layout.fillWidth: true
                                Layout.preferredHeight: Math.max(220, mappingsModel.count * 48 + 20)

                                ListView {
                                    id: mappingsList
                                    anchors.fill: parent
                                    clip: true
                                    model: mappingsModel
                                    currentIndex: root.selectedMappingIndex
                                    onCurrentIndexChanged: root.selectedMappingIndex = currentIndex

                                    delegate: Rectangle {
                                        width: ListView.view.width
                                        height: 44
                                        color: index === root.selectedMappingIndex ? "#263241" : "transparent"
                                        border.color: index === root.selectedMappingIndex ? "#ffd84d" : "transparent"
                                        radius: 4

                                        MouseArea {
                                            anchors.fill: parent
                                            onClicked: root.selectedMappingIndex = index
                                        }

                                        RowLayout {
                                            anchors.fill: parent
                                            anchors.margins: 4
                                            spacing: 8

                                            ComboBox {
                                                model: root.eventCodes
                                                currentIndex: Math.max(0, root.eventCodes.indexOf(fromCode))
                                                Layout.fillWidth: true
                                                onPressedChanged: {
                                                    if (pressed) {
                                                        root.selectedMappingIndex = index
                                                        root.selectedMappingSide = "from"
                                                    }
                                                }
                                                onActivated: {
                                                    root.selectedMappingIndex = index
                                                    root.selectedMappingSide = "from"
                                                    mappingsModel.setProperty(index, "fromCode", currentText)
                                                }
                                                ToolTip.visible: hovered
                                                ToolTip.text: root.eventLabel(currentText)
                                            }

                                            Label {
                                                text: "to"
                                                horizontalAlignment: Text.AlignHCenter
                                                Layout.preferredWidth: 24
                                            }

                                            ComboBox {
                                                model: root.eventCodes
                                                currentIndex: Math.max(0, root.eventCodes.indexOf(toCode))
                                                Layout.fillWidth: true
                                                onPressedChanged: {
                                                    if (pressed) {
                                                        root.selectedMappingIndex = index
                                                        root.selectedMappingSide = "to"
                                                    }
                                                }
                                                onActivated: {
                                                    root.selectedMappingIndex = index
                                                    root.selectedMappingSide = "to"
                                                    mappingsModel.setProperty(index, "toCode", currentText)
                                                }
                                                ToolTip.visible: hovered
                                                ToolTip.text: root.eventLabel(currentText)
                                            }

                                            Button {
                                                text: "Remove"
                                                onClicked: {
                                                    mappingsModel.remove(index)
                                                    if (root.selectedMappingIndex >= mappingsModel.count)
                                                        root.selectedMappingIndex = mappingsModel.count - 1
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    TextArea {
                        id: profileEditor
                        text: backend.profile_yaml
                        font.family: "monospace"
                        wrapMode: TextEdit.NoWrap
                        selectByMouse: true
                        persistentSelection: true
                        onTextChanged: root.editorDirty = text !== backend.profile_yaml

                        background: Rectangle {
                            color: "#1f2328"
                            border.color: profileEditor.activeFocus ? "#d6b44c" : "#3a3f44"
                            radius: 4
                        }
                    }
                }
            }
        }
    }
}
