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
    property var outputOptionsModel: (function() {
        const values = parseJson(backend.output_options)
        return values.length > 0 ? values : [
            { id: "xbox360", label: "Xbox 360", supported: true, note: "Implemented through Linux uinput." }
        ]
    })()
    property var selectedProfile: profilesModel.length > 0 && profileList.currentIndex >= 0
        ? profilesModel[profileList.currentIndex]
        : null
    property bool editorDirty: false
    property int selectedLayerIndex: 0
    property var layerMappings: []
    property int selectedMappingIndex: -1
    property string selectedMappingSide: "from"
    property bool hookListening: false
    property string hookStatus: backend.capture_status
    property var sourceVisibilityOptions: [
        { label: "Hide original controller", grab: true },
        { label: "Keep original visible", grab: false }
    ]
    property var unmappedControlOptions: [
        { label: "Pass unmapped controls", passthrough: true },
        { label: "Drop unmapped controls", passthrough: false }
    ]
    property var activationModeOptions: [
        { label: "Hold", mode: "hold" },
        { label: "Toggle", mode: "toggle" }
    ]
    property var mappingActionOptions: [
        { label: "Map", action: "map" },
        { label: "Disable", action: "disable" },
        { label: "Macro", action: "macro" }
    ]
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
    property var analogAxisCodes: [
        "abs:x",
        "abs:y",
        "abs:rx",
        "abs:ry",
        "abs:z",
        "abs:rz",
        "abs:hat0x",
        "abs:hat0y"
    ]
    property var buttonEventCodes: [
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
        "btn:thumbr"
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

    function outputIndex(outputId) {
        const normalized = outputId || "xbox360"
        for (let i = 0; i < root.outputOptionsModel.length; i++) {
            if (root.outputOptionsModel[i].id === normalized)
                return i
        }
        return 0
    }

    function currentOutput() {
        const index = outputTypeBox.currentIndex
        if (index < 0 || index >= root.outputOptionsModel.length)
            return root.outputOptionsModel[0]
        return root.outputOptionsModel[index]
    }

    function currentOutputId() {
        const output = root.currentOutput()
        return output ? output.id : "xbox360"
    }

    function currentOutputSupported() {
        const output = root.currentOutput()
        return output ? output.supported : false
    }

    function sourceVisibilityIndex(grabSource) {
        for (let i = 0; i < root.sourceVisibilityOptions.length; i++) {
            if (root.sourceVisibilityOptions[i].grab === grabSource)
                return i
        }
        return 0
    }

    function unmappedControlIndex(passthrough) {
        for (let i = 0; i < root.unmappedControlOptions.length; i++) {
            if (root.unmappedControlOptions[i].passthrough === passthrough)
                return i
        }
        return 0
    }

    function currentGrabSource() {
        const index = sourceVisibilityBox.currentIndex
        return root.sourceVisibilityOptions[index] ? root.sourceVisibilityOptions[index].grab : true
    }

    function currentPassthrough() {
        const index = unmappedControlBox.currentIndex
        return root.unmappedControlOptions[index] ? root.unmappedControlOptions[index].passthrough : true
    }

    function activationModeIndex(mode) {
        const normalized = mode || "hold"
        for (let i = 0; i < root.activationModeOptions.length; i++) {
            if (root.activationModeOptions[i].mode === normalized)
                return i
        }
        return 0
    }

    function currentActivationMode() {
        const index = activationModeBox.currentIndex
        return root.activationModeOptions[index] ? root.activationModeOptions[index].mode : "hold"
    }

    function mappingActionIndex(action) {
        const normalized = action || "map"
        for (let i = 0; i < root.mappingActionOptions.length; i++) {
            if (root.mappingActionOptions[i].action === normalized)
                return i
        }
        return 0
    }

    function mappingActionAt(index) {
        const option = root.mappingActionOptions[index]
        return option ? option.action : "map"
    }

    function normalizedTurboInterval(value) {
        const number = Number(value)
        if (!Number.isFinite(number))
            return 75
        return Math.max(10, Math.min(5000, Math.round(number)))
    }

    function normalizedPercent(value, fallback, minValue, maxValue) {
        const number = Number(value)
        if (!Number.isFinite(number))
            return fallback
        return Math.max(minValue, Math.min(maxValue, Math.round(number)))
    }

    function analogRangeMin(code) {
        if (code === "abs:z" || code === "abs:rz")
            return 0
        if (code === "abs:hat0x" || code === "abs:hat0y")
            return -1
        return -32768
    }

    function analogRangeMax(code) {
        if (code === "abs:z" || code === "abs:rz")
            return 255
        if (code === "abs:hat0x" || code === "abs:hat0y")
            return 1
        return 32767
    }

    function addAnalogAxis(code) {
        let axisCode = code || ""
        if (axisCode.length === 0) {
            for (let i = 0; i < root.analogAxisCodes.length; i++) {
                let used = false
                for (let row = 0; row < analogModel.count; row++) {
                    if (analogModel.get(row).code === root.analogAxisCodes[i]) {
                        used = true
                        break
                    }
                }
                if (!used) {
                    axisCode = root.analogAxisCodes[i]
                    break
                }
            }
        }
        if (axisCode.length === 0)
            axisCode = "abs:x"
        analogModel.append({
            code: axisCode,
            deadzonePercent: 0,
            sensitivityPercent: 100,
            invert: false,
            outputMin: root.analogRangeMin(axisCode),
            outputMax: root.analogRangeMax(axisCode)
        })
    }

    function loadAnalogSettings(analog) {
        analogModel.clear()
        const axes = analog && analog.axes ? analog.axes : []
        for (let i = 0; i < axes.length; i++) {
            const axisCode = axes[i].code_name || axes[i].code || "abs:x"
            analogModel.append({
                code: axisCode,
                deadzonePercent: root.normalizedPercent((axes[i].deadzone || 0) * 100, 0, 0, 99),
                sensitivityPercent: root.normalizedPercent((axes[i].sensitivity || 1) * 100, 100, 1, 400),
                invert: axes[i].invert === true,
                outputMin: axes[i].output_min !== undefined ? axes[i].output_min : root.analogRangeMin(axisCode),
                outputMax: axes[i].output_max !== undefined ? axes[i].output_max : root.analogRangeMax(axisCode)
            })
        }
    }

    function analogYaml() {
        if (analogModel.count === 0)
            return ""

        let text = "analog:\n"
        text += "  axes:\n"
        for (let i = 0; i < analogModel.count; i++) {
            const row = analogModel.get(i)
            const code = row.code || "abs:x"
            text += "    - code: " + code + "\n"
            text += "      deadzone: " + (root.normalizedPercent(row.deadzonePercent, 0, 0, 99) / 100).toFixed(2) + "\n"
            text += "      sensitivity: " + (root.normalizedPercent(row.sensitivityPercent, 100, 1, 400) / 100).toFixed(2) + "\n"
            text += "      invert: " + (row.invert === true ? "true" : "false") + "\n"
            text += "      output_min: " + (row.outputMin !== undefined ? row.outputMin : root.analogRangeMin(code)) + "\n"
            text += "      output_max: " + (row.outputMax !== undefined ? row.outputMax : root.analogRangeMax(code)) + "\n"
        }
        return text
    }

    function sanitizeLayerId(value, fallback) {
        const normalized = String(value || fallback || "shift_1")
            .trim()
            .toLowerCase()
            .replace(/[\s-]+/g, "_")
            .replace(/[^a-z0-9_]/g, "")
        return normalized.length > 0 ? normalized : fallback
    }

    function mappingRowsFromModel() {
        const rows = []
        for (let i = 0; i < mappingsModel.count; i++) {
            const row = mappingsModel.get(i)
            rows.push({
                fromCode: row.fromCode,
                toCode: row.toCode,
                action: row.action || "map",
                turboEnabled: row.turboEnabled === true,
                turboIntervalMs: root.normalizedTurboInterval(row.turboIntervalMs)
            })
        }
        return rows
    }

    function mappingRowsFromProfile(mappings) {
        const rows = []
        for (let i = 0; mappings && i < mappings.length; i++) {
            const turbo = mappings[i].turbo || null
            const action = mappings[i].action || "map"
            rows.push({
                fromCode: mappings[i].from_name || mappings[i].from || "btn:south",
                toCode: action === "macro"
                    ? root.macroTargetFromProfile(mappings[i])
                    : mappings[i].to_name || mappings[i].to || "btn:south",
                action: action,
                turboEnabled: turbo !== null,
                turboIntervalMs: turbo && turbo.interval_ms ? turbo.interval_ms : 75
            })
        }
        return rows
    }

    function macroTargetFromProfile(mapping) {
        const macro = mapping && mapping.macro ? mapping.macro : null
        const events = macro && macro.events ? macro.events : []
        for (let i = 0; i < events.length; i++) {
            if (events[i].code_name && events[i].code_name.length > 0)
                return events[i].code_name
        }
        return mapping.to_name || mapping.to || "btn:south"
    }

    function setMappingRows(rows) {
        mappingsModel.clear()
        for (let i = 0; rows && i < rows.length; i++) {
            mappingsModel.append({
                fromCode: rows[i].fromCode || "btn:south",
                toCode: rows[i].toCode || "btn:south",
                action: rows[i].action || "map",
                turboEnabled: rows[i].turboEnabled === true,
                turboIntervalMs: root.normalizedTurboInterval(rows[i].turboIntervalMs)
            })
        }
        root.selectedMappingIndex = mappingsModel.count > 0 ? 0 : -1
    }

    function addLayer(layerId, layerName, activationMode, activationControl, consumeActivation, rows) {
        layersModel.append({
            layerId: layerId,
            layerName: layerName,
            activationMode: activationMode || "hold",
            activationControl: activationControl || "btn:tl",
            consumeActivation: consumeActivation !== false
        })
        const mappings = []
        for (let i = 0; rows && i < rows.length; i++)
            mappings.push({
                fromCode: rows[i].fromCode,
                toCode: rows[i].toCode,
                action: rows[i].action || "map",
                turboEnabled: rows[i].turboEnabled === true,
                turboIntervalMs: root.normalizedTurboInterval(rows[i].turboIntervalMs)
            })
        root.layerMappings.push(mappings)
    }

    function resetLayers(mainRows) {
        layersModel.clear()
        root.layerMappings = []
        root.addLayer("main", "Main", "hold", "btn:tl", true, mainRows || [])
        root.selectedLayerIndex = 0
        root.setMappingRows(root.layerMappings[0])
        Qt.callLater(root.loadCurrentLayerFields)
    }

    function currentLayer() {
        if (root.selectedLayerIndex < 0 || root.selectedLayerIndex >= layersModel.count)
            return null
        return layersModel.get(root.selectedLayerIndex)
    }

    function currentLayerIsMain() {
        const layer = root.currentLayer()
        return !layer || layer.layerId === "main" || root.selectedLayerIndex === 0
    }

    function syncCurrentLayerMappings() {
        if (root.selectedLayerIndex < 0)
            return
        root.layerMappings[root.selectedLayerIndex] = root.mappingRowsFromModel()
    }

    function syncCurrentLayerMetadata() {
        if (root.selectedLayerIndex < 0 || root.selectedLayerIndex >= layersModel.count)
            return

        if (root.currentLayerIsMain()) {
            layersModel.setProperty(root.selectedLayerIndex, "layerId", "main")
            layersModel.setProperty(root.selectedLayerIndex, "layerName", "Main")
            return
        }

        const fallback = "shift_" + root.selectedLayerIndex
        layersModel.setProperty(
            root.selectedLayerIndex,
            "layerId",
            root.sanitizeLayerId(layerIdField.text, fallback)
        )
        layersModel.setProperty(
            root.selectedLayerIndex,
            "layerName",
            layerNameField.text.trim() || fallback.replace("_", " ")
        )
        layersModel.setProperty(root.selectedLayerIndex, "activationMode", root.currentActivationMode())
        layersModel.setProperty(
            root.selectedLayerIndex,
            "activationControl",
            activationControlBox.currentText || "btn:tl"
        )
        layersModel.setProperty(
            root.selectedLayerIndex,
            "consumeActivation",
            consumeActivationCheck.checked
        )
    }

    function loadCurrentLayerFields() {
        const layer = root.currentLayer()
        if (!layer)
            return
        layerBox.currentIndex = root.selectedLayerIndex
        layerIdField.text = layer.layerId || "main"
        layerNameField.text = layer.layerName || layer.layerId || "Main"
        activationModeBox.currentIndex = root.activationModeIndex(layer.activationMode)
        activationControlBox.currentIndex = Math.max(0, root.eventCodes.indexOf(layer.activationControl || "btn:tl"))
        consumeActivationCheck.checked = layer.consumeActivation !== false
    }

    function selectLayerIndex(index) {
        if (index < 0 || index >= layersModel.count)
            return
        root.syncCurrentLayerMetadata()
        root.syncCurrentLayerMappings()
        root.selectedLayerIndex = index
        root.setMappingRows(root.layerMappings[index])
        root.loadCurrentLayerFields()
    }

    function addShiftLayer() {
        root.syncCurrentLayerMetadata()
        root.syncCurrentLayerMappings()
        if (layersModel.count >= 11) {
            root.hookStatus = "Profiles support up to 10 shift layers."
            return
        }
        const number = layersModel.count
        root.addLayer("shift_" + number, "Shift " + number, "hold", "btn:tl", true, [])
        root.selectLayerIndex(layersModel.count - 1)
    }

    function removeCurrentLayer() {
        if (root.currentLayerIsMain())
            return
        const index = root.selectedLayerIndex
        layersModel.remove(index)
        root.layerMappings.splice(index, 1)
        root.selectLayerIndex(Math.max(0, index - 1))
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
        mappingsModel.append({
            fromCode: fromCode,
            toCode: toCode,
            action: "map",
            turboEnabled: false,
            turboIntervalMs: 75
        })
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
        if (root.selectedMappingSide === "to" && row.action === "disable")
            return false
        return root.selectedMappingSide === "to" ? row.toCode === code : row.fromCode === code
    }

    function mappingCountFor(code, side) {
        let total = 0
        const propertyName = side === "to" ? "toCode" : "fromCode"
        for (let i = 0; i < mappingsModel.count; i++) {
            const row = mappingsModel.get(i)
            if (side === "to" && row.action === "disable")
                continue
            if (row[propertyName] === code)
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
        outputTypeBox.currentIndex = root.outputIndex("xbox360")
        sourceVisibilityBox.currentIndex = root.sourceVisibilityIndex(true)
        unmappedControlBox.currentIndex = root.unmappedControlIndex(true)
        root.resetLayers([
            { fromCode: "btn:west", toCode: "btn:east" },
            { fromCode: "btn:east", toCode: "btn:west" }
        ])
        analogModel.clear()
        backend.new_profile()
    }

    function loadStructuredProfile(profile) {
        profileIdField.text = profile.id || ""
        profileNameField.text = profile.name || ""
        profileDescriptionField.text = profile.description || ""
        matchNameField.text = profile.device_match && profile.device_match.name ? profile.device_match.name : "*"
        outputTypeBox.currentIndex = root.outputIndex(profile.output_type || "xbox360")
        sourceVisibilityBox.currentIndex = root.sourceVisibilityIndex(profile.grab_source !== false)
        unmappedControlBox.currentIndex = root.unmappedControlIndex(profile.passthrough !== false)
        layersModel.clear()
        root.layerMappings = []
        if (profile.layers && profile.layers.length > 0) {
            for (let i = 0; i < profile.layers.length; i++) {
                const layer = profile.layers[i]
                const activation = layer.activation || {}
                root.addLayer(
                    layer.id || (i === 0 ? "main" : "shift_" + i),
                    layer.name || (i === 0 ? "Main" : "Shift " + i),
                    activation.mode || "hold",
                    activation.control_name || "btn:tl",
                    activation.consume !== false,
                    root.mappingRowsFromProfile(layer.mappings)
                )
            }
        } else {
            root.addLayer("main", "Main", "hold", "btn:tl", true, root.mappingRowsFromProfile(profile.mappings))
        }
        root.selectedLayerIndex = 0
        root.setMappingRows(root.layerMappings[0])
        root.loadAnalogSettings(profile.analog)
        Qt.callLater(root.loadCurrentLayerFields)
    }

    function yamlString(value) {
        return "\"" + String(value).replace(/\\/g, "\\\\").replace(/"/g, "\\\"") + "\""
    }

    function mappingYaml(indent, row) {
        const action = row.action || "map"
        let text = ""
        text += indent + "- from: " + row.fromCode + "\n"
        if (action === "disable") {
            text += indent + "  action: disable\n"
        } else if (action === "macro") {
            text += indent + "  action: macro\n"
            text += indent + "  macro:\n"
            text += indent + "    events:\n"
            text += indent + "      - tap: " + (row.toCode || "btn:south") + "\n"
        } else {
            text += indent + "  to: " + row.toCode + "\n"
            if (row.turboEnabled === true) {
                text += indent + "  turbo:\n"
                text += indent + "    interval_ms: " + root.normalizedTurboInterval(row.turboIntervalMs) + "\n"
            }
        }
        return text
    }

    function structuredProfileYaml() {
        root.syncCurrentLayerMetadata()
        root.syncCurrentLayerMappings()
        let text = ""
        text += "id: " + yamlString(profileIdField.text.trim() || "my-layout") + "\n"
        text += "name: " + yamlString(profileNameField.text.trim() || profileIdField.text.trim() || "My layout") + "\n"
        text += "description: " + yamlString(profileDescriptionField.text) + "\n"
        text += "match:\n"
        text += "  name: " + yamlString(matchNameField.text.trim() || "*") + "\n"
        text += "output:\n"
        text += "  type: " + root.currentOutputId() + "\n"
        text += "passthrough: " + (root.currentPassthrough() ? "true" : "false") + "\n"
        text += "grab_source: " + (root.currentGrabSource() ? "true" : "false") + "\n"
        text += root.analogYaml()
        if (layersModel.count <= 1) {
            const rows = root.layerMappings[0] || []
            if (rows.length === 0) {
                text += "mappings: []\n"
            } else {
                text += "mappings:\n"
                for (let i = 0; i < rows.length; i++) {
                    text += root.mappingYaml("  ", rows[i])
                }
            }
        } else {
            text += "layers:\n"
            for (let i = 0; i < layersModel.count; i++) {
                const layer = layersModel.get(i)
                const rows = root.layerMappings[i] || []
                text += "  - id: " + root.sanitizeLayerId(layer.layerId, i === 0 ? "main" : "shift_" + i) + "\n"
                text += "    name: " + yamlString(layer.layerName || (i === 0 ? "Main" : "Shift " + i)) + "\n"
                if (i > 0) {
                    text += "    activation:\n"
                    text += "      mode: " + (layer.activationMode || "hold") + "\n"
                    text += "      control: " + (layer.activationControl || "btn:tl") + "\n"
                    text += "      consume: " + (layer.consumeActivation !== false ? "true" : "false") + "\n"
                }
                if (rows.length === 0) {
                    text += "    mappings: []\n"
                } else {
                    text += "    mappings:\n"
                    for (let rowIndex = 0; rowIndex < rows.length; rowIndex++) {
                        text += root.mappingYaml("      ", rows[rowIndex])
                    }
                }
            }
        }
        return text
    }

    function currentProfileYaml() {
        return editorTabs.currentIndex === 0 ? root.structuredProfileYaml() : profileEditor.text
    }

    function saveCurrentEditor() {
        if (editorTabs.currentIndex === 0)
            backend.save_profile(root.currentProfileYaml())
        else
            backend.save_profile(root.currentProfileYaml())
    }

    function startRemap() {
        const device = root.selectedDevice()
        if (!device)
            return

        const result = backend.start_remap(device.path, root.currentProfileYaml())
        if (result === "ok")
            remapTimer.restart()
    }

    function stopRemap() {
        remapTimer.stop()
        backend.stop_remap()
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

    ListModel {
        id: layersModel
    }

    ListModel {
        id: analogModel
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

    Timer {
        id: remapTimer
        interval: 250
        repeat: true

        onTriggered: {
            backend.poll_remap()
            if (!backend.remap_active)
                stop()
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
                        text: backend.remap_active ? "Remap Off" : "Apply"
                        enabled: backend.remap_active
                            || (root.selectedDevice() !== null && root.currentOutputSupported())
                        onClicked: backend.remap_active ? root.stopRemap() : root.startRemap()
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

                Label {
                    text: backend.remap_status
                    visible: text.length > 0
                    elide: Text.ElideRight
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
                                    model: root.outputOptionsModel
                                    textRole: "label"
                                    valueRole: "id"
                                    Layout.fillWidth: true

                                    delegate: ItemDelegate {
                                        width: outputTypeBox.width
                                        text: modelData.label + (modelData.supported ? "" : " (planned)")
                                        enabled: modelData.supported
                                        highlighted: outputTypeBox.highlightedIndex === index
                                    }
                                }

                                Label { text: "Original" }
                                ComboBox {
                                    id: sourceVisibilityBox
                                    model: root.sourceVisibilityOptions
                                    textRole: "label"
                                    Layout.fillWidth: true
                                }

                                Label { text: "Unmapped" }
                                ComboBox {
                                    id: unmappedControlBox
                                    model: root.unmappedControlOptions
                                    textRole: "label"
                                    Layout.fillWidth: true
                                }
                            }

                            Frame {
                                Layout.fillWidth: true

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 8

                                    RowLayout {
                                        Layout.fillWidth: true

                                        Label {
                                            text: "Layer"
                                            font.bold: true
                                        }

                                        ComboBox {
                                            id: layerBox
                                            model: layersModel
                                            textRole: "layerName"
                                            Layout.fillWidth: true
                                            onActivated: function(index) {
                                                root.selectLayerIndex(index)
                                            }
                                        }

                                        Button {
                                            text: "Add Shift"
                                            enabled: layersModel.count < 11
                                            onClicked: root.addShiftLayer()
                                        }

                                        Button {
                                            text: "Remove"
                                            enabled: !root.currentLayerIsMain()
                                            onClicked: root.removeCurrentLayer()
                                        }
                                    }

                                    GridLayout {
                                        visible: !root.currentLayerIsMain()
                                        columns: 2
                                        columnSpacing: 10
                                        rowSpacing: 8
                                        Layout.fillWidth: true

                                        Label { text: "Layer ID" }
                                        TextField {
                                            id: layerIdField
                                            Layout.fillWidth: true
                                            onEditingFinished: root.syncCurrentLayerMetadata()
                                        }

                                        Label { text: "Name" }
                                        TextField {
                                            id: layerNameField
                                            Layout.fillWidth: true
                                            onEditingFinished: root.syncCurrentLayerMetadata()
                                        }

                                        Label { text: "Activation" }
                                        ComboBox {
                                            id: activationModeBox
                                            model: root.activationModeOptions
                                            textRole: "label"
                                            Layout.fillWidth: true
                                            onActivated: root.syncCurrentLayerMetadata()
                                        }

                                        Label { text: "Control" }
                                        ComboBox {
                                            id: activationControlBox
                                            model: root.eventCodes
                                            Layout.fillWidth: true
                                            onActivated: root.syncCurrentLayerMetadata()
                                        }

                                        Label { text: "Activator input" }
                                        CheckBox {
                                            id: consumeActivationCheck
                                            text: "Do not pass activator through"
                                            checked: true
                                            onToggled: root.syncCurrentLayerMetadata()
                                        }
                                    }
                                }
                            }

                            Frame {
                                Layout.fillWidth: true
                                Layout.preferredHeight: Math.max(190, analogModel.count * 116 + 56)

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 8

                                    RowLayout {
                                        Layout.fillWidth: true

                                        Label {
                                            text: "Analog"
                                            font.bold: true
                                        }

                                        Item {
                                            Layout.fillWidth: true
                                        }

                                        Button {
                                            text: "Add Axis"
                                            onClicked: root.addAnalogAxis("")
                                        }
                                    }

                                    ListView {
                                        id: analogList
                                        Layout.fillWidth: true
                                        Layout.fillHeight: true
                                        clip: true
                                        model: analogModel

                                        delegate: Rectangle {
                                            width: ListView.view.width
                                            height: 110
                                            color: index % 2 === 0 ? "#1d232b" : "transparent"
                                            radius: 4

                                            ColumnLayout {
                                                anchors.fill: parent
                                                anchors.margins: 4
                                                spacing: 4

                                                RowLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 8

                                                    ComboBox {
                                                        model: root.analogAxisCodes
                                                        currentIndex: Math.max(0, root.analogAxisCodes.indexOf(code))
                                                        Layout.fillWidth: true
                                                        onActivated: {
                                                            const nextCode = currentText
                                                            analogModel.setProperty(index, "code", nextCode)
                                                            analogModel.setProperty(index, "outputMin", root.analogRangeMin(nextCode))
                                                            analogModel.setProperty(index, "outputMax", root.analogRangeMax(nextCode))
                                                        }
                                                        ToolTip.visible: hovered
                                                        ToolTip.text: root.eventLabel(currentText)
                                                    }

                                                    CheckBox {
                                                        text: "Invert"
                                                        checked: invert === true
                                                        Layout.preferredWidth: 86
                                                        onToggled: analogModel.setProperty(index, "invert", checked)
                                                    }

                                                    Button {
                                                        text: "Remove"
                                                        onClicked: analogModel.remove(index)
                                                    }
                                                }

                                                RowLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 8

                                                    Label {
                                                        text: "DZ"
                                                        Layout.preferredWidth: 30
                                                        horizontalAlignment: Text.AlignRight
                                                    }

                                                    SpinBox {
                                                        from: 0
                                                        to: 99
                                                        editable: true
                                                        value: root.normalizedPercent(deadzonePercent, 0, 0, 99)
                                                        Layout.fillWidth: true
                                                        onValueModified: analogModel.setProperty(index, "deadzonePercent", value)
                                                    }

                                                    Label {
                                                        text: "Sens"
                                                        Layout.preferredWidth: 42
                                                        horizontalAlignment: Text.AlignRight
                                                    }

                                                    SpinBox {
                                                        from: 1
                                                        to: 400
                                                        editable: true
                                                        value: root.normalizedPercent(sensitivityPercent, 100, 1, 400)
                                                        Layout.fillWidth: true
                                                        onValueModified: analogModel.setProperty(index, "sensitivityPercent", value)
                                                    }
                                                }

                                                RowLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 8

                                                    Label {
                                                        text: "Min"
                                                        Layout.preferredWidth: 30
                                                        horizontalAlignment: Text.AlignRight
                                                    }

                                                    SpinBox {
                                                        from: root.analogRangeMin(code)
                                                        to: root.analogRangeMax(code) - 1
                                                        editable: true
                                                        value: outputMin !== undefined ? outputMin : root.analogRangeMin(code)
                                                        Layout.fillWidth: true
                                                        onValueModified: analogModel.setProperty(index, "outputMin", value)
                                                    }

                                                    Label {
                                                        text: "Max"
                                                        Layout.preferredWidth: 42
                                                        horizontalAlignment: Text.AlignRight
                                                    }

                                                    SpinBox {
                                                        from: root.analogRangeMin(code) + 1
                                                        to: root.analogRangeMax(code)
                                                        editable: true
                                                        value: outputMax !== undefined ? outputMax : root.analogRangeMax(code)
                                                        Layout.fillWidth: true
                                                        onValueModified: analogModel.setProperty(index, "outputMax", value)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true

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
                                        text: (function() {
                                            const row = root.selectedMapping()
                                            if (!row)
                                                return "No mapping row"
                                            const prefix = "Row " + (root.selectedMappingIndex + 1) + ": "
                                                + root.eventLabel(row.fromCode)
                                            if (row.action === "disable")
                                                return prefix + " disabled"
                                            if (row.action === "macro")
                                                return prefix + " macro tap " + root.eventLabel(row.toCode)
                                            return prefix + " -> " + root.eventLabel(row.toCode)
                                                + (row.turboEnabled === true ? " turbo" : "")
                                        })()
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
                                Layout.preferredHeight: Math.max(240, mappingsModel.count * 64 + 20)

                                ListView {
                                    id: mappingsList
                                    anchors.fill: parent
                                    clip: true
                                    model: mappingsModel
                                    currentIndex: root.selectedMappingIndex
                                    onCurrentIndexChanged: root.selectedMappingIndex = currentIndex

                                    delegate: Rectangle {
                                        width: ListView.view.width
                                        height: 60
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

                                            ComboBox {
                                                model: root.mappingActionOptions
                                                textRole: "label"
                                                currentIndex: root.mappingActionIndex(action)
                                                Layout.preferredWidth: 112
                                                onPressedChanged: {
                                                    if (pressed)
                                                        root.selectedMappingIndex = index
                                                }
                                                onActivated: function(actionIndex) {
                                                    root.selectedMappingIndex = index
                                                    const nextAction = root.mappingActionAt(actionIndex)
                                                    mappingsModel.setProperty(index, "action", nextAction)
                                                    if (nextAction === "disable" || nextAction === "macro")
                                                        mappingsModel.setProperty(index, "turboEnabled", false)
                                                    if (nextAction === "macro"
                                                            && root.buttonEventCodes.indexOf(toCode) < 0)
                                                        mappingsModel.setProperty(index, "toCode", "btn:south")
                                                }
                                            }

                                            Label {
                                                text: action === "disable" ? "" : action === "macro" ? "tap" : "to"
                                                horizontalAlignment: Text.AlignHCenter
                                                Layout.preferredWidth: 24
                                            }

                                            ComboBox {
                                                readonly property var targetCodes: action === "macro"
                                                    ? root.buttonEventCodes
                                                    : root.eventCodes
                                                model: targetCodes
                                                currentIndex: Math.max(0, targetCodes.indexOf(toCode))
                                                Layout.fillWidth: true
                                                enabled: action !== "disable"
                                                opacity: enabled ? 1.0 : 0.35
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

                                            CheckBox {
                                                text: "Turbo"
                                                enabled: action !== "disable" && action !== "macro"
                                                checked: turboEnabled === true
                                                Layout.preferredWidth: 78
                                                onToggled: {
                                                    root.selectedMappingIndex = index
                                                    mappingsModel.setProperty(index, "turboEnabled", checked)
                                                }
                                            }

                                            SpinBox {
                                                from: 10
                                                to: 5000
                                                stepSize: 5
                                                editable: true
                                                value: root.normalizedTurboInterval(turboIntervalMs)
                                                enabled: action !== "disable" && action !== "macro" && turboEnabled === true
                                                visible: enabled
                                                Layout.preferredWidth: 88
                                                onValueModified: {
                                                    root.selectedMappingIndex = index
                                                    mappingsModel.setProperty(index, "turboIntervalMs", value)
                                                }
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
