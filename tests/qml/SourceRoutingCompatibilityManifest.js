.pragma library

function manifest() {
    return {
        retainedAliases: [],
        retainedAliasDecision: "No AppModel source-routing alias has a production compatibility consumer.",
        retainedCompositionMembers: [
            {
                name: "sourceRouting",
                reason: "AppModel remains the composition root for the focused SourceRoutingState facade.",
                consumers: ["AppShell", "feature models", "state projections"]
            },
            {
                name: "storageSource",
                reason: "AppModel caches the focused storage view while composing StorageAppState.",
                consumers: ["AppModel.storageApp"]
            },
            {
                name: "deliverySource",
                reason: "AppModel caches the focused delivery view while composing DeliveryAppState.",
                consumers: ["AppModel.deliveryApp"]
            }
        ],
        retiredMembers: [
            retired("blockchainArgs", "method", "production", ["AppModel.chainPages.gateway"]),
            retired("loadSourcePolicy", "method", "production", ["AppShell.Component.onCompleted"]),
            retired("sourcePolicyDefault", "method", "none", []),
            retired("sourceModePolicy", "method", "none", []),
            retired("sourceModePolicies", "method", "none", []),
            retired("sourceModeOptions", "method", "test_only", ["tst_app_model.qml"]),
            retired("sourceModeIndexFor", "method", "production", ["SettingsProfileWorkspace.sourceIndexFor"]),
            retired("sourceModeAt", "method", "production", ["SettingsProfileWorkspace.sourceModeAt"]),
            retired("sourceModeAdapter", "method", "none", []),
            retired("resolvedSourceModeKey", "method", "none", []),
            retired("sourceModeTargetKind", "method", "none", []),
            retired("sourceModeUsesEndpoint", "method", "test_only", ["tst_app_model.qml"]),
            retired("sourceModeSupportsCidProbe", "method", "test_only", ["tst_app_model.qml"]),
            retired("sourceModeSupportsMutatingDiagnostics", "method", "test_only", ["tst_app_model.qml"]),
            retired("coreSourceView", "method", "none", []),
            retired("deliverySourceView", "method", "none", []),
            retired("storageSourceView", "method", "none", []),
            retired("sourceFamilyView", "method", "none", []),
            retired("deliveryReportView", "method", "none", []),
            retired("storageReportView", "method", "test_only", ["tst_app_model.qml"]),
            retired("deliverySourceTarget", "method", "production", ["SourceInspectionReadModel", "SourceObservationProjection"]),
            retired("configuredMessagingRestUrl", "method", "production", ["AppModel.capabilityRegistryRuntimeInputs"]),
            retired("normalizedMessagingSourceMode", "method", "test_only", ["tst_app_model.qml"]),
            retired("effectiveMessagingSourceMode", "method", "production", ["SourceInspectionReadModel", "SocialCollaborationOrchestrator"]),
            retired("normalizedCoreSourceMode", "method", "test_only", ["tst_app_model.qml"]),
            retired("effectiveCoreSourceMode", "method", "test_only", ["tst_app_model.qml"]),
            retired("blockchainSourceLabel", "method", "production", ["SettingsPage.qml"]),
            retired("blockchainSourceTarget", "method", "none", []),
            retired("storageSourceTarget", "method", "production", ["AppModelSearch", "FooterStatusProjection", "SourceInspectionReadModel", "SourceObservationProjection"]),
            retired("configuredStorageRestUrl", "method", "production", ["AppModel.capabilityRegistryRuntimeInputs", "SettingsPage.qml", "ModulePage.qml"]),
            retired("normalizedStorageSourceMode", "method", "test_only", ["tst_app_model.qml"]),
            retired("sourcePolicy", "alias", "production", ["AppModel.networkProfiles"]),
            retired("sourcePolicyLoaded", "alias", "test_only", ["tst_app_model.qml"])
        ],
        requiredFacadeMethods: [
            "blockchainArgs", "loadSourcePolicy", "sourcePolicyDefault", "sourceModePolicy",
            "sourceModePolicies", "sourceModeOptions", "sourceModeIndexFor", "sourceModeAt",
            "sourceModeAdapter", "resolvedSourceModeKey", "sourceModeTargetKind",
            "sourceModeUsesEndpoint", "sourceModeSupportsCidProbe",
            "sourceModeSupportsMutatingDiagnostics", "coreSourceView", "deliverySourceView",
            "storageSourceView", "sourceFamilyView", "deliveryReportView", "storageReportView",
            "deliverySourceTarget", "configuredMessagingRestUrl", "normalizedMessagingSourceMode",
            "effectiveMessagingSourceMode", "normalizedCoreSourceMode", "effectiveCoreSourceMode",
            "blockchainSourceLabel", "blockchainSourceTarget", "storageSourceTarget",
            "configuredStorageRestUrl", "normalizedStorageSourceMode"
        ],
        requiredFacadeProperties: ["sourcePolicy", "sourcePolicyLoaded"]
    }
}

function retired(name, kind, consumerClass, formerConsumers) {
    return {
        name: name,
        kind: kind,
        consumerClass: consumerClass,
        formerConsumers: formerConsumers
    }
}
