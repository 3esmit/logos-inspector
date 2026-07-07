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
    ResolveLezTarget,
    IndexerHealth,
    IndexerStatus,
    IndexerFinalizedHead,
    IndexerBlocks,
    IndexerBlockByHash,
    IndexerTransferRecipients,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationExclusiveGroup {
    StorageDownload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperationRoute {
    pub(crate) domain: OperationDomain,
    pub(crate) method: OperationMethod,
    pub(crate) label: &'static str,
    pub(crate) start_async: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OperationCatalogEntry {
    method: OperationMethod,
    name: &'static str,
    domain: OperationDomain,
    label: &'static str,
    uses_mutating_flag: bool,
    cancellable: bool,
    exclusive_group: Option<OperationExclusiveGroup>,
}

macro_rules! operation_entry {
    ($method:ident, $name:literal, $domain:ident, $label:literal) => {
        OperationCatalogEntry {
            method: OperationMethod::$method,
            name: $name,
            domain: OperationDomain::$domain,
            label: $label,
            uses_mutating_flag: false,
            cancellable: false,
            exclusive_group: None,
        }
    };
    ($method:ident, $name:literal, $domain:ident, $label:literal, mutating) => {
        OperationCatalogEntry {
            method: OperationMethod::$method,
            name: $name,
            domain: OperationDomain::$domain,
            label: $label,
            uses_mutating_flag: true,
            cancellable: false,
            exclusive_group: None,
        }
    };
    ($method:ident, $name:literal, $domain:ident, $label:literal, mutating, cancellable, $exclusive_group:ident) => {
        OperationCatalogEntry {
            method: OperationMethod::$method,
            name: $name,
            domain: OperationDomain::$domain,
            label: $label,
            uses_mutating_flag: true,
            cancellable: true,
            exclusive_group: Some(OperationExclusiveGroup::$exclusive_group),
        }
    };
}

const STORAGE_DOWNLOAD_START_ALIAS: &str = "storageDownloadStart";

const OPERATION_CATALOG: &[OperationCatalogEntry] = &[
    operation_entry!(
        StorageManifests,
        "storageManifests",
        Storage,
        "Storage manifests"
    ),
    operation_entry!(
        StorageDownloadManifest,
        "storageDownloadManifest",
        Storage,
        "Storage manifest"
    ),
    operation_entry!(
        StorageFetch,
        "storageFetch",
        Storage,
        "Storage fetch",
        mutating
    ),
    operation_entry!(
        StorageUploadUrl,
        "storageUploadUrl",
        Storage,
        "Storage upload",
        mutating
    ),
    operation_entry!(
        StorageDownloadToUrl,
        "storageDownloadToUrl",
        Storage,
        "Storage download",
        mutating,
        cancellable,
        StorageDownload
    ),
    operation_entry!(
        StorageRemove,
        "storageRemove",
        Storage,
        "Storage remove",
        mutating
    ),
    operation_entry!(
        DeliverySubscribe,
        "deliverySubscribe",
        Delivery,
        "Delivery subscribe",
        mutating
    ),
    operation_entry!(
        DeliveryUnsubscribe,
        "deliveryUnsubscribe",
        Delivery,
        "Delivery unsubscribe",
        mutating
    ),
    operation_entry!(
        DeliverySend,
        "deliverySend",
        Delivery,
        "Delivery send",
        mutating
    ),
    operation_entry!(
        DeliveryCreateNode,
        "deliveryCreateNode",
        Delivery,
        "Delivery create node",
        mutating
    ),
    operation_entry!(
        DeliveryStart,
        "deliveryStart",
        Delivery,
        "Delivery start",
        mutating
    ),
    operation_entry!(
        DeliveryStop,
        "deliveryStop",
        Delivery,
        "Delivery stop",
        mutating
    ),
    operation_entry!(
        DeliveryStoreQuery,
        "deliveryStoreQuery",
        Delivery,
        "Delivery store query"
    ),
    operation_entry!(
        LocalNodesAction,
        "localNodesAction",
        LocalNodes,
        "Local node action"
    ),
    operation_entry!(
        LocalWalletCreateAccount,
        "localWalletCreateAccount",
        Wallet,
        "Wallet account"
    ),
    operation_entry!(
        LocalWalletSendTransaction,
        "localWalletSendTransaction",
        Wallet,
        "Wallet send"
    ),
    operation_entry!(
        LocalWalletInstructionSubmit,
        "localWalletInstructionSubmit",
        Wallet,
        "IDL instruction"
    ),
    operation_entry!(
        LocalWalletCommand,
        "localWalletCommand",
        Wallet,
        "Wallet command"
    ),
    operation_entry!(
        LocalWalletDeployProgram,
        "localWalletDeployProgram",
        Wallet,
        "Program deploy"
    ),
    operation_entry!(
        LocalWalletSyncPrivate,
        "localWalletSyncPrivate",
        Wallet,
        "Private sync"
    ),
    operation_entry!(
        LocalWalletAccounts,
        "localWalletAccounts",
        Wallet,
        "Wallet accounts"
    ),
    operation_entry!(
        BlockchainNode,
        "blockchainNode",
        Blockchain,
        "Blockchain node"
    ),
    operation_entry!(
        BlockchainBlocks,
        "blockchainBlocks",
        Blockchain,
        "Blockchain blocks"
    ),
    operation_entry!(
        BlockchainLiveBlocks,
        "blockchainLiveBlocks",
        Blockchain,
        "Blockchain live blocks"
    ),
    operation_entry!(
        BlockchainBlock,
        "blockchainBlock",
        Blockchain,
        "Blockchain block"
    ),
    operation_entry!(
        BlockchainTransaction,
        "blockchainTransaction",
        Blockchain,
        "Blockchain transaction"
    ),
    operation_entry!(Head, "head", Execution, "Execution head"),
    operation_entry!(Programs, "programs", Execution, "Programs"),
    operation_entry!(Block, "block", Execution, "Sequencer block"),
    operation_entry!(
        SequencerBlocks,
        "sequencerBlocks",
        Execution,
        "Sequencer blocks"
    ),
    operation_entry!(Transaction, "transaction", Execution, "Transaction"),
    operation_entry!(
        InspectTransaction,
        "inspectTransaction",
        Execution,
        "Transaction inspection"
    ),
    operation_entry!(
        TraceTransaction,
        "traceTransaction",
        Execution,
        "Transaction trace"
    ),
    operation_entry!(Account, "account", Execution, "Account inspection"),
    operation_entry!(
        ResolveLezTarget,
        "resolveLezTarget",
        Execution,
        "LEZ lookup"
    ),
    operation_entry!(IndexerHealth, "indexerHealth", Indexer, "Indexer health"),
    operation_entry!(IndexerStatus, "indexerStatus", Indexer, "Indexer status"),
    operation_entry!(
        IndexerFinalizedHead,
        "indexerFinalizedHead",
        Indexer,
        "Indexer finalized head"
    ),
    operation_entry!(IndexerBlocks, "indexerBlocks", Indexer, "Indexer blocks"),
    operation_entry!(
        IndexerBlockByHash,
        "indexerBlockByHash",
        Indexer,
        "Indexer block"
    ),
    operation_entry!(
        IndexerTransferRecipients,
        "indexerTransferRecipients",
        Indexer,
        "Indexer transfer recipients"
    ),
];

impl OperationCatalogEntry {
    fn route(self, start_async: bool) -> OperationRoute {
        OperationRoute {
            domain: self.domain,
            method: self.method,
            label: self.label,
            start_async,
        }
    }
}

fn operation_catalog_entry(method: OperationMethod) -> OperationCatalogEntry {
    for entry in OPERATION_CATALOG {
        if entry.method == method {
            return *entry;
        }
    }
    OperationCatalogEntry {
        method,
        name: "operation",
        domain: OperationDomain::Execution,
        label: "Operation",
        uses_mutating_flag: false,
        cancellable: false,
        exclusive_group: None,
    }
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
        OPERATION_CATALOG
            .iter()
            .find(|entry| entry.name == method)
            .map(|entry| entry.method)
    }

    pub(crate) fn as_str(self) -> &'static str {
        operation_catalog_entry(self).name
    }

    pub(crate) fn domain(self) -> OperationDomain {
        operation_catalog_entry(self).domain
    }

    pub(crate) fn label(self) -> &'static str {
        operation_catalog_entry(self).label
    }

    fn uses_mutating_flag(self) -> bool {
        operation_catalog_entry(self).uses_mutating_flag
    }

    pub(crate) fn cancellable(self) -> bool {
        operation_catalog_entry(self).cancellable
    }

    pub(crate) fn exclusive_group(self) -> Option<OperationExclusiveGroup> {
        operation_catalog_entry(self).exclusive_group
    }
}

pub(crate) fn operation_route(method: &str) -> Option<OperationRoute> {
    if method == STORAGE_DOWNLOAD_START_ALIAS {
        return Some(operation_catalog_entry(OperationMethod::StorageDownloadToUrl).route(true));
    }
    let method = OperationMethod::from_str(method)?;
    Some(operation_catalog_entry(method).route(false))
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
            ("resolveLezTarget", OperationDomain::Execution, "LEZ lookup"),
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
        if storage_download.exclusive_group() != Some(OperationExclusiveGroup::StorageDownload) {
            bail!("storageDownloadToUrl should own the storage download exclusive group");
        }
        let storage_upload = OperationMethod::from_str("storageUploadUrl")
            .context("storageUploadUrl should exist")?;
        if storage_upload.cancellable() {
            bail!("storageUploadUrl should not be cancellable");
        }
        if storage_upload.exclusive_group().is_some() {
            bail!("storageUploadUrl should not own an exclusive group");
        }
        Ok(())
    }
}
