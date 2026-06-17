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
    property var eventCodes: [
        "btn:south",
        "btn:east",
        "btn:west",
        "btn:north",
        "btn:l1",
        "btn:r1",
        "btn:l2",
        "btn:r2",
        "btn:select",
        "btn:start",
        "btn:mode",
        "btn:l3",
        "btn:r3",
        "abs:x",
        "abs:y",
        "abs:rx",
        "abs:ry",
        "abs:z",
        "abs:rz",
        "abs:hat0x",
        "abs:hat0y"
    ]

    onProfilesModelChanged: Qt.callLater(function() {
        if (root.profilesModel.length > 0 && backend.profile_yaml.length === 0)
            root.selectProfile(profileList.currentIndex >= 0 ? profileList.currentIndex : 0)
    })

    function hexId(value) {
        return Number(value).toString(16).padStart(4, "0")
    }

    function capabilitiesText(values) {
        return values && values.length > 0 ? values.join(", ") : "no mapped capabilities"
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
        mappingsModel.append({ fromCode: "btn:west", toCode: "btn:east" })
        mappingsModel.append({ fromCode: "btn:east", toCode: "btn:west" })
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
                                    onClicked: mappingsModel.append({ fromCode: "btn:south", toCode: "btn:south" })
                                }
                            }

                            Frame {
                                Layout.fillWidth: true
                                Layout.preferredHeight: Math.max(260, mappingsModel.count * 48 + 20)

                                ListView {
                                    id: mappingsList
                                    anchors.fill: parent
                                    clip: true
                                    model: mappingsModel

                                    delegate: RowLayout {
                                        width: ListView.view.width
                                        height: 44
                                        spacing: 8

                                        ComboBox {
                                            model: root.eventCodes
                                            Layout.fillWidth: true
                                            Component.onCompleted: currentIndex = Math.max(0, root.eventCodes.indexOf(fromCode))
                                            onActivated: mappingsModel.setProperty(index, "fromCode", currentText)
                                        }

                                        Label {
                                            text: "to"
                                            horizontalAlignment: Text.AlignHCenter
                                            Layout.preferredWidth: 24
                                        }

                                        ComboBox {
                                            model: root.eventCodes
                                            Layout.fillWidth: true
                                            Component.onCompleted: currentIndex = Math.max(0, root.eventCodes.indexOf(toCode))
                                            onActivated: mappingsModel.setProperty(index, "toCode", currentText)
                                        }

                                        Button {
                                            text: "Remove"
                                            onClicked: mappingsModel.remove(index)
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
