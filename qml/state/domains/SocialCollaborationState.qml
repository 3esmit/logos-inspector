import QtQuick
import QtQml.Models
import "../social/SocialCollaborationOrchestrator.js" as Orchestrator
import "../social" as Social

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
    property int sharedIdlRetryGeneration: 0
    property var sharedIdlRetryRequests: ({})
    property int sharedIdlRetryBaseDelayMs: 1000
    property int sharedIdlRetryMaxAttempts: 4
    property ListModel socialIdentities: ListModel {}
    readonly property var deliveryAdapterInitialization: sourceRouting.deliveryOperationAdapter()
    readonly property var storageAdapterInitialization: sourceRouting.storageOperationAdapter()
    property Social.DeliveryStoreQueryCoordinator storeQueryCoordinator: Social.DeliveryStoreQueryCoordinator {
        gateway: root.gateway
        adapterInitialization: root.deliveryAdapterInitialization
        mutatingDiagnosticsEnabled: root.messagingMutatingDiagnosticsEnabled
    }
    property Social.SocialWriteCoordinator writeCoordinator: Social.SocialWriteCoordinator {
        gateway: root.gateway
        storageAdapterInitialization: root.storageAdapterInitialization
        deliveryAdapterInitialization: root.deliveryAdapterInitialization
        storageMutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        deliveryMutatingDiagnosticsEnabled: root.messagingMutatingDiagnosticsEnabled
    }
    readonly property bool sharedIdlRetriesRunning: Object.keys(sharedIdlRetryRequests || {}).length > 0
    readonly property bool operationsRunning: storeQueryCoordinator.running || writeCoordinator.running
        || sharedIdlRetriesRunning
    readonly property bool writesRunning: writeCoordinator.running

    onDeliveryAdapterInitializationChanged: invalidateReadSourceRequests()
    onStorageAdapterInitializationChanged: invalidateReadSourceRequests()
    onMessagingMutatingDiagnosticsEnabledChanged: invalidateReadSourceRequests()
    onStorageMutatingDiagnosticsEnabledChanged: invalidateReadSourceRequests()

    function commentTopic(layer, entity, id) {
        return Orchestrator.socialCommentTopic(root, layer, entity, id)
    }
    function zoneCommentTopic(entityRef) {
        return Orchestrator.socialZoneCommentTopic(root, entityRef)
    }
    function zoneAccountIdlTopic(entityRef) {
        return Orchestrator.socialZoneAccountIdlTopic(root, entityRef)
    }
    function commentsView(topic) {
        return Orchestrator.commentView(root, topic)
    }
    function loadComments(topic, reset, pageSize, expectedAccountId) {
        return Orchestrator.loadSocialComments(root, topic, reset, pageSize, expectedAccountId)
    }
    function postComment(topic, body, identityKey, entityRef, onComplete) {
        return Orchestrator.postSocialComment(root, topic, body, identityKey, entityRef, onComplete)
    }
    function applyIncomingComment(event) {
        return Orchestrator.applyIncomingComment(root, event)
    }
    function applyIncomingDeliveryMessage(message) {
        return Orchestrator.applyIncomingDeliveryMessage(root, message)
    }

    function loadSettings(value) {
        return Orchestrator.loadSocialSettings(root, value)
    }
    function settingsPayload() {
        return Orchestrator.socialSettingsPayload(root)
    }
    function identitiesView() {
        return Orchestrator.identityView(root)
    }
    function createIdentity(displayName) {
        return Orchestrator.createSocialIdentity(root, displayName)
    }
    function selectIdentity(key) {
        return Orchestrator.selectSocialIdentity(root, key)
    }
    function setIdentityDefaultMode(mode) {
        return Orchestrator.setSocialIdentityDefaultMode(root, mode)
    }

    function setSharedIdlPolicy(policy) {
        return Orchestrator.setSharedIdlPolicy(root, policy)
    }
    function setSharedIdlAutoShare(enabled) {
        return Orchestrator.setSharedIdlAutoShare(root, enabled)
    }
    function refreshSharedIdlsForAccount(entityRef, dataHex, ownerProgramId) {
        return Orchestrator.refreshSharedIdlsForAccount(root, entityRef, dataHex, ownerProgramId)
    }
    function applySharedIdlPolicy(accountId, entry) {
        return Orchestrator.applySharedIdlPolicy(root, accountId, entry)
    }
    function sharedIdlSuggestions(accountId, ownerProgramId) {
        return Orchestrator.sharedIdlSuggestions(root, accountId, ownerProgramId)
    }
    function sharedIdlEntriesForAccount(accountId, ownerProgramId) {
        return Orchestrator.sharedIdlEntriesForAccount(root, accountId, ownerProgramId)
    }
    function publishAccountIdl(entityRef, ownerProgramId, idlEntry, onComplete) {
        return Orchestrator.publishAccountIdl(root, entityRef, ownerProgramId, idlEntry, onComplete)
    }
    function publishRegisteredIdl(accountId, idlKey, onComplete) {
        return Orchestrator.publishRegisteredIdl(root, accountId, idlKey, onComplete)
    }
    function maybeAutoShareAccountIdl(entityRef, ownerProgramId, idlEntry) {
        return Orchestrator.maybeAutoShareAccountIdl(root, entityRef, ownerProgramId, idlEntry)
    }

    function queryDeliveryStore(scope, cursor, pageSize, label, callback) {
        return storeQueryCoordinator.start(scope, cursor, pageSize, label, callback)
    }

    function startCommentWrite(request, callback) {
        return writeCoordinator.startComment(request, callback)
    }
    function startSharedIdlWrite(request, callback) {
        return writeCoordinator.startSharedIdl(request, callback)
    }
    function pollOperations() {
        const queryPolls = storeQueryCoordinator.poll()
        const retryStarts = Orchestrator.pollSharedIdlRetries(root)
        const writePoll = writeCoordinator.poll()
        return writePoll !== null ? writePoll : (queryPolls || retryStarts)
    }
    function storeQueryCallerPending(callerKey) {
        return storeQueryCoordinator.callerPending(callerKey)
    }
    function isCurrentStoreQuery(ticket) {
        return storeQueryCoordinator.isCurrent(ticket)
    }
    function releaseStoreQuery(ticket) {
        return storeQueryCoordinator.release(ticket)
    }
    function invalidateStoreQueryCaller(callerKey) {
        return storeQueryCoordinator.invalidateCaller(callerKey)
    }
    function invalidateSharedIdlRequests() {
        Orchestrator.invalidateSharedIdlRetries(root)
        writeCoordinator.invalidateKind("shared-idl", qsTr("Shared IDL settings changed during publication."))
        storeQueryCoordinator.invalidateFamily("shared-idl")
    }

    function invalidateSourceRequests() {
        writeCoordinator.invalidate(qsTr("Social source changed during write."))
        invalidateReadSourceRequests()
    }
    function invalidateReadSourceRequests() {
        Orchestrator.invalidateSharedIdlRetries(root)
        storeQueryCoordinator.invalidateSource()
        Orchestrator.invalidateSocialCommentRequests(root)
    }
}
