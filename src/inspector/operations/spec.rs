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
pub(crate) enum OperationMethod {
    StorageManifests,
    StorageDownloadManifest,
    StorageFetch,
    StorageUploadUrl,
    StorageDownloadToUrl,
    StorageRemove,
    DeliverySubscribe,
    DeliveryUnsubscribe,
    DeliverySend,
    DeliveryCreateNode,
    DeliveryStart,
    DeliveryStop,
    DeliveryStoreQuery,
    LocalNodesAction,
    LocalWalletCreateAccount,
    LocalWalletSendTransaction,
    LocalWalletInstructionSubmit,
    LocalWalletCommand,
    LocalWalletDeployProgram,
    LocalWalletSyncPrivate,
    LocalWalletAccounts,
    BlockchainNode,
    BlockchainBlocks,
    BlockchainLiveBlocks,
    BlockchainBlock,
    BlockchainTransaction,
    Head,
    Programs,
    Block,
    SequencerBlocks,
    Transaction,
    InspectTransaction,
    TraceTransaction,
    Account,
    IndexerHealth,
    IndexerStatus,
    IndexerFinalizedHead,
    IndexerBlocks,
    IndexerBlockByHash,
    IndexerTransferRecipients,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperationRoute {
    pub(crate) domain: OperationDomain,
    pub(crate) method: OperationMethod,
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
}

impl OperationMethod {
    pub(crate) fn from_str(method: &str) -> Option<Self> {
        let method = match method {
            "storageManifests" => Self::StorageManifests,
            "storageDownloadManifest" => Self::StorageDownloadManifest,
            "storageFetch" => Self::StorageFetch,
            "storageUploadUrl" => Self::StorageUploadUrl,
            "storageDownloadToUrl" => Self::StorageDownloadToUrl,
            "storageRemove" => Self::StorageRemove,
            "deliverySubscribe" => Self::DeliverySubscribe,
            "deliveryUnsubscribe" => Self::DeliveryUnsubscribe,
            "deliverySend" => Self::DeliverySend,
            "deliveryCreateNode" => Self::DeliveryCreateNode,
            "deliveryStart" => Self::DeliveryStart,
            "deliveryStop" => Self::DeliveryStop,
            "deliveryStoreQuery" => Self::DeliveryStoreQuery,
            "localNodesAction" => Self::LocalNodesAction,
            "localWalletCreateAccount" => Self::LocalWalletCreateAccount,
            "localWalletSendTransaction" => Self::LocalWalletSendTransaction,
            "localWalletInstructionSubmit" => Self::LocalWalletInstructionSubmit,
            "localWalletCommand" => Self::LocalWalletCommand,
            "localWalletDeployProgram" => Self::LocalWalletDeployProgram,
            "localWalletSyncPrivate" => Self::LocalWalletSyncPrivate,
            "localWalletAccounts" => Self::LocalWalletAccounts,
            "blockchainNode" => Self::BlockchainNode,
            "blockchainBlocks" => Self::BlockchainBlocks,
            "blockchainLiveBlocks" => Self::BlockchainLiveBlocks,
            "blockchainBlock" => Self::BlockchainBlock,
            "blockchainTransaction" => Self::BlockchainTransaction,
            "head" => Self::Head,
            "programs" => Self::Programs,
            "block" => Self::Block,
            "sequencerBlocks" => Self::SequencerBlocks,
            "transaction" => Self::Transaction,
            "inspectTransaction" => Self::InspectTransaction,
            "traceTransaction" => Self::TraceTransaction,
            "account" => Self::Account,
            "indexerHealth" => Self::IndexerHealth,
            "indexerStatus" => Self::IndexerStatus,
            "indexerFinalizedHead" => Self::IndexerFinalizedHead,
            "indexerBlocks" => Self::IndexerBlocks,
            "indexerBlockByHash" => Self::IndexerBlockByHash,
            "indexerTransferRecipients" => Self::IndexerTransferRecipients,
            _ => return None,
        };
        Some(method)
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::StorageManifests => "storageManifests",
            Self::StorageDownloadManifest => "storageDownloadManifest",
            Self::StorageFetch => "storageFetch",
            Self::StorageUploadUrl => "storageUploadUrl",
            Self::StorageDownloadToUrl => "storageDownloadToUrl",
            Self::StorageRemove => "storageRemove",
            Self::DeliverySubscribe => "deliverySubscribe",
            Self::DeliveryUnsubscribe => "deliveryUnsubscribe",
            Self::DeliverySend => "deliverySend",
            Self::DeliveryCreateNode => "deliveryCreateNode",
            Self::DeliveryStart => "deliveryStart",
            Self::DeliveryStop => "deliveryStop",
            Self::DeliveryStoreQuery => "deliveryStoreQuery",
            Self::LocalNodesAction => "localNodesAction",
            Self::LocalWalletCreateAccount => "localWalletCreateAccount",
            Self::LocalWalletSendTransaction => "localWalletSendTransaction",
            Self::LocalWalletInstructionSubmit => "localWalletInstructionSubmit",
            Self::LocalWalletCommand => "localWalletCommand",
            Self::LocalWalletDeployProgram => "localWalletDeployProgram",
            Self::LocalWalletSyncPrivate => "localWalletSyncPrivate",
            Self::LocalWalletAccounts => "localWalletAccounts",
            Self::BlockchainNode => "blockchainNode",
            Self::BlockchainBlocks => "blockchainBlocks",
            Self::BlockchainLiveBlocks => "blockchainLiveBlocks",
            Self::BlockchainBlock => "blockchainBlock",
            Self::BlockchainTransaction => "blockchainTransaction",
            Self::Head => "head",
            Self::Programs => "programs",
            Self::Block => "block",
            Self::SequencerBlocks => "sequencerBlocks",
            Self::Transaction => "transaction",
            Self::InspectTransaction => "inspectTransaction",
            Self::TraceTransaction => "traceTransaction",
            Self::Account => "account",
            Self::IndexerHealth => "indexerHealth",
            Self::IndexerStatus => "indexerStatus",
            Self::IndexerFinalizedHead => "indexerFinalizedHead",
            Self::IndexerBlocks => "indexerBlocks",
            Self::IndexerBlockByHash => "indexerBlockByHash",
            Self::IndexerTransferRecipients => "indexerTransferRecipients",
        }
    }

    pub(crate) fn domain(self) -> OperationDomain {
        match self {
            Self::StorageManifests
            | Self::StorageDownloadManifest
            | Self::StorageFetch
            | Self::StorageUploadUrl
            | Self::StorageDownloadToUrl
            | Self::StorageRemove => OperationDomain::Storage,
            Self::DeliverySubscribe
            | Self::DeliveryUnsubscribe
            | Self::DeliverySend
            | Self::DeliveryCreateNode
            | Self::DeliveryStart
            | Self::DeliveryStop
            | Self::DeliveryStoreQuery => OperationDomain::Delivery,
            Self::LocalNodesAction => OperationDomain::LocalNodes,
            Self::LocalWalletCreateAccount
            | Self::LocalWalletSendTransaction
            | Self::LocalWalletInstructionSubmit
            | Self::LocalWalletCommand
            | Self::LocalWalletDeployProgram
            | Self::LocalWalletSyncPrivate
            | Self::LocalWalletAccounts => OperationDomain::Wallet,
            Self::BlockchainNode
            | Self::BlockchainBlocks
            | Self::BlockchainLiveBlocks
            | Self::BlockchainBlock
            | Self::BlockchainTransaction => OperationDomain::Blockchain,
            Self::IndexerHealth
            | Self::IndexerStatus
            | Self::IndexerFinalizedHead
            | Self::IndexerBlocks
            | Self::IndexerBlockByHash
            | Self::IndexerTransferRecipients => OperationDomain::Indexer,
            Self::Head
            | Self::Programs
            | Self::Block
            | Self::SequencerBlocks
            | Self::Transaction
            | Self::InspectTransaction
            | Self::TraceTransaction
            | Self::Account => OperationDomain::Execution,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::StorageManifests => "Storage manifests",
            Self::StorageDownloadManifest => "Storage manifest",
            Self::StorageFetch => "Storage fetch",
            Self::StorageUploadUrl => "Storage upload",
            Self::StorageDownloadToUrl => "Storage download",
            Self::StorageRemove => "Storage remove",
            Self::DeliverySubscribe => "Delivery subscribe",
            Self::DeliveryUnsubscribe => "Delivery unsubscribe",
            Self::DeliverySend => "Delivery send",
            Self::DeliveryCreateNode => "Delivery create node",
            Self::DeliveryStart => "Delivery start",
            Self::DeliveryStop => "Delivery stop",
            Self::DeliveryStoreQuery => "Delivery store query",
            Self::LocalNodesAction => "Local node action",
            Self::LocalWalletCreateAccount => "Wallet account",
            Self::LocalWalletSendTransaction => "Wallet send",
            Self::LocalWalletInstructionSubmit => "IDL instruction",
            Self::LocalWalletCommand => "Wallet command",
            Self::LocalWalletDeployProgram => "Program deploy",
            Self::LocalWalletSyncPrivate => "Private sync",
            Self::LocalWalletAccounts => "Wallet accounts",
            Self::BlockchainNode => "Blockchain node",
            Self::BlockchainBlocks => "Blockchain blocks",
            Self::BlockchainLiveBlocks => "Blockchain live blocks",
            Self::BlockchainBlock => "Blockchain block",
            Self::BlockchainTransaction => "Blockchain transaction",
            Self::Head => "Execution head",
            Self::Programs => "Programs",
            Self::Block => "Sequencer block",
            Self::SequencerBlocks => "Sequencer blocks",
            Self::Transaction => "Transaction",
            Self::InspectTransaction => "Transaction inspection",
            Self::TraceTransaction => "Transaction trace",
            Self::Account => "Account inspection",
            Self::IndexerHealth => "Indexer health",
            Self::IndexerStatus => "Indexer status",
            Self::IndexerFinalizedHead => "Indexer finalized head",
            Self::IndexerBlocks => "Indexer blocks",
            Self::IndexerBlockByHash => "Indexer block",
            Self::IndexerTransferRecipients => "Indexer transfer recipients",
        }
    }

    fn uses_mutating_flag(self) -> bool {
        matches!(
            self,
            Self::StorageFetch
                | Self::StorageUploadUrl
                | Self::StorageDownloadToUrl
                | Self::StorageRemove
                | Self::DeliveryCreateNode
                | Self::DeliveryStart
                | Self::DeliveryStop
                | Self::DeliverySubscribe
                | Self::DeliveryUnsubscribe
                | Self::DeliverySend
        )
    }

    pub(crate) fn cancellable(self) -> bool {
        self == Self::StorageDownloadToUrl
    }
}

pub(crate) fn operation_route(method: &str) -> Option<OperationRoute> {
    if method == "storageDownloadStart" {
        return Some(OperationRoute {
            domain: OperationDomain::Storage,
            method: OperationMethod::StorageDownloadToUrl,
            label: "Storage download",
            start_async: true,
        });
    }
    let method = OperationMethod::from_str(method)?;
    Some(OperationRoute {
        domain: method.domain(),
        method,
        label: method.label(),
        start_async: false,
    })
}

pub(crate) fn operation_uses_mutating_flag(method: &str) -> bool {
    OperationMethod::from_str(method).is_some_and(OperationMethod::uses_mutating_flag)
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

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail};

    use super::*;

    #[test]
    fn operation_route_maps_direct_methods_to_domains() -> Result<()> {
        let cases = [
            ("storageFetch", OperationDomain::Storage, "Storage fetch"),
            ("deliverySend", OperationDomain::Delivery, "Delivery send"),
            (
                "localWalletAccounts",
                OperationDomain::Wallet,
                "Wallet accounts",
            ),
            (
                "blockchainLiveBlocks",
                OperationDomain::Blockchain,
                "Blockchain live blocks",
            ),
            ("indexerStatus", OperationDomain::Indexer, "Indexer status"),
            (
                "inspectTransaction",
                OperationDomain::Execution,
                "Transaction inspection",
            ),
        ];

        for (name, domain, label) in cases {
            let Some(route) = operation_route(name) else {
                bail!("operation route missing for {name}");
            };
            if route.domain != domain || route.method.as_str() != name || route.label != label {
                bail!("unexpected route for {name}: {route:?}");
            }
        }

        Ok(())
    }

    #[test]
    fn operation_route_preserves_storage_download_start_alias() -> Result<()> {
        let Some(route) = operation_route("storageDownloadStart") else {
            bail!("storage download alias route missing");
        };

        if route.domain != OperationDomain::Storage
            || route.method != OperationMethod::StorageDownloadToUrl
            || !route.start_async
        {
            bail!("unexpected storage download alias route: {route:?}");
        }

        Ok(())
    }

    #[test]
    fn operation_flags_are_owned_by_method_catalog() -> Result<()> {
        if !operation_uses_mutating_flag("deliverySend") {
            bail!("deliverySend should require mutating flag");
        }
        if operation_uses_mutating_flag("indexerStatus") {
            bail!("indexerStatus should not require mutating flag");
        }
        let storage_download = OperationMethod::from_str("storageDownloadToUrl")
            .context("storageDownloadToUrl should exist")?;
        if !storage_download.cancellable() {
            bail!("storageDownloadToUrl should be cancellable");
        }
        let storage_upload = OperationMethod::from_str("storageUploadUrl")
            .context("storageUploadUrl should exist")?;
        if storage_upload.cancellable() {
            bail!("storageUploadUrl should not be cancellable");
        }
        Ok(())
    }
}
