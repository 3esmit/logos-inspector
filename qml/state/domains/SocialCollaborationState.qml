import QtQuick
import QtQml.Models
import "../social/SocialCollaborationOrchestrator.js" as Orchestrator

QtObject {
    id: root

    required property var bridge
    required property string inspectorModule
    required property var sourceRouting
    required property var registeredIdls
    required property var gateway
    required property bool busy
    required property string messagingSourceMode
    required property bool messagingMutatingDiagnosticsEnabled
    required property bool storageMutatingDiagnosticsEnabled

    property int socialCommentPageSize: 20
    property string socialIdentityDefaultMode: "perConversation"
    property string selectedSocialIdentityKey: ""
    property var socialConversationIdentityKeys: ({})
    property int socialIdentityRevision: 0
    property var socialCommentState: ({})
    property int socialCommentRevision: 0
    property var socialSharedIdls: ({})
    property string sharedIdlPolicy: "suggestion"
    property bool sharedIdlAutoShare: false
    property var socialAutoSharedIdls: ({})
    property int sharedIdlRevision: 0
    property ListModel socialIdentities: ListModel {}

    function commentTopic(layer, entity, id) { return Orchestrator.socialCommentTopic(root, layer, entity, id) }
    function zoneCommentTopic(entityRef) { return Orchestrator.socialZoneCommentTopic(root, entityRef) }
    function zoneAccountIdlTopic(entityRef) { return Orchestrator.socialZoneAccountIdlTopic(root, entityRef) }
    function commentsView(topic) { return Orchestrator.commentView(root, topic) }
    function loadComments(topic, reset, pageSize, expectedAccountId) { return Orchestrator.loadSocialComments(root, topic, reset, pageSize, expectedAccountId) }
    function postComment(topic, body, identityKey, entityRef) { return Orchestrator.postSocialComment(root, topic, body, identityKey, entityRef) }
    function applyIncomingComment(event) { return Orchestrator.applyIncomingComment(root, event) }
    function applyIncomingDeliveryMessage(message) { return Orchestrator.applyIncomingDeliveryMessage(root, message) }

    function loadSettings(value) { return Orchestrator.loadSocialSettings(root, value) }
    function settingsPayload() { return Orchestrator.socialSettingsPayload(root) }
    function identitiesView() { return Orchestrator.identityView(root) }
    function createIdentity(displayName) { return Orchestrator.createSocialIdentity(root, displayName) }
    function selectIdentity(key) { return Orchestrator.selectSocialIdentity(root, key) }
    function setIdentityDefaultMode(mode) { return Orchestrator.setSocialIdentityDefaultMode(root, mode) }

    function setSharedIdlPolicy(policy) { return Orchestrator.setSharedIdlPolicy(root, policy) }
    function setSharedIdlAutoShare(enabled) { return Orchestrator.setSharedIdlAutoShare(root, enabled) }
    function refreshSharedIdlsForAccount(entityRef, dataHex, ownerProgramId) { return Orchestrator.refreshSharedIdlsForAccount(root, entityRef, dataHex, ownerProgramId) }
    function applySharedIdlPolicy(accountId, entry) { return Orchestrator.applySharedIdlPolicy(root, accountId, entry) }
    function sharedIdlSuggestions(accountId, ownerProgramId) { return Orchestrator.sharedIdlSuggestions(root, accountId, ownerProgramId) }
    function sharedIdlEntriesForAccount(accountId, ownerProgramId) { return Orchestrator.sharedIdlEntriesForAccount(root, accountId, ownerProgramId) }
    function publishAccountIdl(entityRef, ownerProgramId, idlEntry) { return Orchestrator.publishAccountIdl(root, entityRef, ownerProgramId, idlEntry) }
    function maybeAutoShareAccountIdl(entityRef, ownerProgramId, idlEntry) { return Orchestrator.maybeAutoShareAccountIdl(root, entityRef, ownerProgramId, idlEntry) }
}
