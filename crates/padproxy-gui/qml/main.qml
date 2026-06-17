import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.benwbooth.padproxy

ApplicationWindow {
    id: root
    width: 1120
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
                        text: modelData.name + "\n" + modelData.path + "  " + hexId(modelData.vendor) + ":" + hexId(modelData.product)
                        highlighted: ListView.isCurrentItem
                        onClicked: deviceList.currentIndex = index

                        function hexId(value) {
                            return Number(value).toString(16).padStart(4, "0")
                        }
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

                ListView {
                    id: profileList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: root.profilesModel

                    delegate: ItemDelegate {
                        width: ListView.view.width
                        text: modelData.name + "\n" + modelData.output_type
                        highlighted: ListView.isCurrentItem
                        onClicked: profileList.currentIndex = index
                    }
                }
            }
        }

        Pane {
            SplitView.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                Label {
                    text: selectedProfile ? selectedProfile.name : "Mappings"
                    font.bold: true
                }

                Label {
                    text: selectedProfile ? selectedProfile.description : ""
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }

                Frame {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    ListView {
                        anchors.fill: parent
                        clip: true
                        model: selectedProfile ? selectedProfile.mappings : []

                        delegate: RowLayout {
                            width: ListView.view.width
                            height: 36

                            Label {
                                text: modelData.from_name
                                Layout.fillWidth: true
                            }

                            Label {
                                text: "->"
                                horizontalAlignment: Text.AlignHCenter
                                Layout.preferredWidth: 36
                            }

                            Label {
                                text: modelData.to_name
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }
        }
    }
}
