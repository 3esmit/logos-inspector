.import "SocialCollaborationOrchestrator.js" as Orchestrator

function socialCommentTopic(root, layer, entity, id) { return Orchestrator.socialCommentTopic.apply(null, arguments) }
function socialZoneCommentTopic(root, entityRef) { return Orchestrator.socialZoneCommentTopic.apply(null, arguments) }
function socialZoneAccountIdlTopic(root, entityRef) { return Orchestrator.socialZoneAccountIdlTopic.apply(null, arguments) }
function zoneSocialScope(entityRef) { return Orchestrator.zoneSocialScope.apply(null, arguments) }
function socialRuntimeString(root, method, args) { return Orchestrator.socialRuntimeString.apply(null, arguments) }
function socialComments(root, topic) { return Orchestrator.socialComments.apply(null, arguments) }
function socialCommentState(root, topic) { return Orchestrator.socialCommentState.apply(null, arguments) }
function loadSocialComments(root, topic, reset, pageSize, expectedAccountId) { return Orchestrator.loadSocialComments.apply(null, arguments) }
function setSocialCommentState(root, topic, state) { return Orchestrator.setSocialCommentState.apply(null, arguments) }
function applyIncomingComment(root, event) { return Orchestrator.applyIncomingComment.apply(null, arguments) }
function applyIncomingDeliveryMessage(root, message) { return Orchestrator.applyIncomingDeliveryMessage.apply(null, arguments) }
function socialMessagePayload(value) { return Orchestrator.socialMessagePayload.apply(null, arguments) }
function socialCommentRowFromIncomingEvent(root, incoming) { return Orchestrator.socialCommentRowFromIncomingEvent.apply(null, arguments) }
function socialCommentRowsFromMessages(root, messages) { return Orchestrator.socialCommentRowsFromMessages.apply(null, arguments) }
function mergeSocialCommentRows(root, existingRows, incomingRows) { return Orchestrator.mergeSocialCommentRows.apply(null, arguments) }
function socialCommentDedupeKey(row) { return Orchestrator.socialCommentDedupeKey.apply(null, arguments) }
function socialMessageRowKey(message) { return Orchestrator.socialMessageRowKey.apply(null, arguments) }
function socialStoreCursor(root, value) { return Orchestrator.socialStoreCursor.apply(null, arguments) }
function firstStoreCursor(value, depth) { return Orchestrator.firstStoreCursor.apply(null, arguments) }
function lastSocialMessageCursor(root, messages) { return Orchestrator.lastSocialMessageCursor.apply(null, arguments) }
function postSocialComment(root, topic, body, identityKey) { return Orchestrator.postSocialComment.apply(null, arguments) }
function socialDeliveryArgs(root, extra) { return Orchestrator.socialDeliveryArgs.apply(null, arguments) }
function socialMessageSourceAvailable(root) { return Orchestrator.socialMessageSourceAvailable.apply(null, arguments) }
function normalizedSocialGate(gate) { return Orchestrator.normalizedSocialGate.apply(null, arguments) }
function socialGateWithInputMissing(gate, dependency, label) { return Orchestrator.socialGateWithInputMissing.apply(null, arguments) }
function socialGateWithTopic(root, gate, topic) { return Orchestrator.socialGateWithTopic.apply(null, arguments) }
function socialStoreGate(root) { return Orchestrator.socialStoreGate.apply(null, arguments) }
function socialCommentReadGate(root, topic) { return Orchestrator.socialCommentReadGate.apply(null, arguments) }
function socialCommentWriteGate(root, topic) { return Orchestrator.socialCommentWriteGate.apply(null, arguments) }
function socialSharedIdlReadGate(root) { return Orchestrator.socialSharedIdlReadGate.apply(null, arguments) }
function socialSharedIdlWriteGate(root, topic) { return Orchestrator.socialSharedIdlWriteGate.apply(null, arguments) }
function socialMissingDependencyText(row) { return Orchestrator.socialMissingDependencyText.apply(null, arguments) }
function socialGateDetailText(root, gate, fallback) { return Orchestrator.socialGateDetailText.apply(null, arguments) }
function socialStoreAvailable(root) { return Orchestrator.socialStoreAvailable.apply(null, arguments) }
function querySocialStore(root, topic, cursor, pageSize, label) { return Orchestrator.querySocialStore.apply(null, arguments) }
function socialCommentSendAvailable(root, topic) { return Orchestrator.socialCommentSendAvailable.apply(null, arguments) }
function socialCommentReadAvailable(root, topic) { return Orchestrator.socialCommentReadAvailable.apply(null, arguments) }
function socialSharedIdlReadAvailable(root) { return Orchestrator.socialSharedIdlReadAvailable.apply(null, arguments) }
function socialSharedIdlWriteAvailable(root, topic) { return Orchestrator.socialSharedIdlWriteAvailable.apply(null, arguments) }
function validSocialTopic(root, topic) { return Orchestrator.validSocialTopic.apply(null, arguments) }
function socialPageSize(root, pageSize) { return Orchestrator.socialPageSize.apply(null, arguments) }
function loadSocialSettings(root, value) { return Orchestrator.loadSocialSettings.apply(null, arguments) }
function socialSettingsPayload(root) { return Orchestrator.socialSettingsPayload.apply(null, arguments) }
function normalizedSocialIdentityEntry(root, entry, fallbackIndex) { return Orchestrator.normalizedSocialIdentityEntry.apply(null, arguments) }
function socialIdentityRows(root) { return Orchestrator.socialIdentityRows.apply(null, arguments) }
function createSocialIdentity(root, displayName) { return Orchestrator.createSocialIdentity.apply(null, arguments) }
function socialRandomHex(length) { return Orchestrator.socialRandomHex.apply(null, arguments) }
function socialIdentityForKey(root, key) { return Orchestrator.socialIdentityForKey.apply(null, arguments) }
function socialIdentityForConversation(root, topic, key) { return Orchestrator.socialIdentityForConversation.apply(null, arguments) }
function firstSocialIdentity(root) { return Orchestrator.firstSocialIdentity.apply(null, arguments) }
function selectSocialIdentity(root, key) { return Orchestrator.selectSocialIdentity.apply(null, arguments) }
function setSocialIdentityDefaultMode(root, mode) { return Orchestrator.setSocialIdentityDefaultMode.apply(null, arguments) }
function normalizedSocialIdentityDefaultMode(value) { return Orchestrator.normalizedSocialIdentityDefaultMode.apply(null, arguments) }
function socialIdentityPayload(root, identity) { return Orchestrator.socialIdentityPayload.apply(null, arguments) }
function socialIdentityDisplayName(identity) { return Orchestrator.socialIdentityDisplayName.apply(null, arguments) }
function setSharedIdlPolicy(root, policy) { return Orchestrator.setSharedIdlPolicy.apply(null, arguments) }
function normalizedSharedIdlPolicy(value) { return Orchestrator.normalizedSharedIdlPolicy.apply(null, arguments) }
function setSharedIdlAutoShare(root, enabled) { return Orchestrator.setSharedIdlAutoShare.apply(null, arguments) }
function refreshSharedIdlsForAccount(root, accountId, dataHex, ownerProgramId) { return Orchestrator.refreshSharedIdlsForAccount.apply(null, arguments) }
function applySharedIdlPolicy(root, accountId, entry) { return Orchestrator.applySharedIdlPolicy.apply(null, arguments) }
function acceptedSharedIdlEntryForAccount(root, accountId, entry) { return Orchestrator.acceptedSharedIdlEntryForAccount.apply(null, arguments) }
function idlEntryExists(root, key) { return Orchestrator.idlEntryExists.apply(null, arguments) }
function storeSharedIdl(root, accountId, entry) { return Orchestrator.storeSharedIdl.apply(null, arguments) }
function sharedIdlSuggestions(root, accountId) { return Orchestrator.sharedIdlSuggestions.apply(null, arguments) }
function sharedIdlEntriesForAccount(root, accountId, ownerProgramId) { return Orchestrator.sharedIdlEntriesForAccount.apply(null, arguments) }
function publishAccountIdl(root, accountId, ownerProgramId, idlEntry) { return Orchestrator.publishAccountIdl.apply(null, arguments) }
function maybeAutoShareAccountIdl(root, accountId, ownerProgramId, idlEntry) { return Orchestrator.maybeAutoShareAccountIdl.apply(null, arguments) }
