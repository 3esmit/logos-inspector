#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationDomain {
    Storage,
    Delivery,
    LocalNodes,
    Wallet,
    Indexer,
    Blockchain,
    Execution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LegacyOperationRoute {
    pub(crate) domain: OperationDomain,
    pub(crate) method: &'static str,
    pub(crate) label: &'static str,
    pub(crate) start_async: bool,
}

impl OperationDomain {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Storage => "storage",
            Self::Delivery => "delivery",
            Self::LocalNodes => "localNodes",
            Self::Wallet => "wallet",
            Self::Indexer => "indexer",
            Self::Blockchain => "blockchain",
            Self::Execution => "execution",
        }
    }

    pub(crate) fn from_method(method: &str) -> Self {
        if method.starts_with("storage") {
            Self::Storage
        } else if method.starts_with("delivery") {
            Self::Delivery
        } else if method.starts_with("localNodes") || method.starts_with("localDevnet") {
            Self::LocalNodes
        } else if method.starts_with("localWallet") || method.starts_with("bedrockWallet") {
            Self::Wallet
        } else if method.starts_with("indexer") {
            Self::Indexer
        } else if method.starts_with("blockchain") {
            Self::Blockchain
        } else {
            Self::Execution
        }
    }

    pub(crate) fn is_storage_or_delivery(self) -> bool {
        matches!(self, Self::Storage | Self::Delivery)
    }
}

pub(crate) fn legacy_operation_route(method: &str) -> Option<LegacyOperationRoute> {
    let route = match method {
        "storageDownloadStart" => LegacyOperationRoute {
            domain: OperationDomain::Storage,
            method: "storageDownloadToUrl",
            label: "Storage download",
            start_async: true,
        },
        "localWalletCreateAccount" => wallet_route("localWalletCreateAccount", "Wallet account"),
        "localWalletSendTransaction" => wallet_route("localWalletSendTransaction", "Wallet send"),
        "localWalletInstructionSubmit" => {
            wallet_route("localWalletInstructionSubmit", "IDL instruction")
        }
        "localWalletCommand" => wallet_route("localWalletCommand", "Wallet command"),
        "localWalletDeployProgram" => wallet_route("localWalletDeployProgram", "Program deploy"),
        "localWalletSyncPrivate" => wallet_route("localWalletSyncPrivate", "Private sync"),
        "localWalletAccounts" => wallet_route("localWalletAccounts", "Wallet accounts"),
        "localNodesAction" => LegacyOperationRoute {
            domain: OperationDomain::LocalNodes,
            method: "localNodesAction",
            label: "Local node action",
            start_async: false,
        },
        "storageManifests" => storage_route("storageManifests", "Storage manifests"),
        "storageDownloadManifest" => storage_route("storageDownloadManifest", "Storage manifest"),
        "storageFetch" => storage_route("storageFetch", "Storage fetch"),
        "storageUploadUrl" => storage_route("storageUploadUrl", "Storage upload"),
        "storageDownloadToUrl" => storage_route("storageDownloadToUrl", "Storage download"),
        "storageRemove" => storage_route("storageRemove", "Storage remove"),
        "deliveryCreateNode" => delivery_route("deliveryCreateNode", "Delivery create node"),
        "deliveryStart" => delivery_route("deliveryStart", "Delivery start"),
        "deliveryStop" => delivery_route("deliveryStop", "Delivery stop"),
        "deliverySubscribe" => delivery_route("deliverySubscribe", "Delivery subscribe"),
        "deliveryUnsubscribe" => delivery_route("deliveryUnsubscribe", "Delivery unsubscribe"),
        "deliverySend" => delivery_route("deliverySend", "Delivery send"),
        "deliveryStoreQuery" => delivery_route("deliveryStoreQuery", "Delivery store query"),
        _ => return None,
    };
    Some(route)
}

pub(crate) fn operation_uses_mutating_flag(method: &str) -> bool {
    matches!(
        method,
        "storageFetch"
            | "storageUploadUrl"
            | "storageDownloadToUrl"
            | "storageRemove"
            | "deliveryCreateNode"
            | "deliveryStart"
            | "deliveryStop"
            | "deliverySubscribe"
            | "deliveryUnsubscribe"
            | "deliverySend"
    )
}

pub(crate) fn operation_cancellable(method: &str) -> bool {
    method == "storageDownloadToUrl"
}

pub(crate) fn normalized_operation_method(method: &str) -> String {
    let normalized = method
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    if normalized.is_empty() {
        "operation".to_owned()
    } else {
        normalized
    }
}

fn wallet_route(method: &'static str, label: &'static str) -> LegacyOperationRoute {
    LegacyOperationRoute {
        domain: OperationDomain::Wallet,
        method,
        label,
        start_async: false,
    }
}

fn storage_route(method: &'static str, label: &'static str) -> LegacyOperationRoute {
    LegacyOperationRoute {
        domain: OperationDomain::Storage,
        method,
        label,
        start_async: false,
    }
}

fn delivery_route(method: &'static str, label: &'static str) -> LegacyOperationRoute {
    LegacyOperationRoute {
        domain: OperationDomain::Delivery,
        method,
        label,
        start_async: false,
    }
}
