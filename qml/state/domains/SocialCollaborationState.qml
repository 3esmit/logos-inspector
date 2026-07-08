import QtQuick
import QtQml.Models

QtObject {
    property int commentPageSize: 20
    property string identityDefaultMode: "perConversation"
    property string selectedIdentityKey: ""
    property var conversationIdentityKeys: ({})
    property int identityRevision: 0
    property var commentState: ({})
    property int commentRevision: 0
    property var sharedIdls: ({})
    property string sharedIdlPolicy: "suggestion"
    property bool sharedIdlAutoShare: false
    property var autoSharedIdls: ({})
    property int sharedIdlRevision: 0
    property ListModel identities: ListModel {}
}
