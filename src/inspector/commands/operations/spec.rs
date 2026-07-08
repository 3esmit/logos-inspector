use super::{chain, delivery, local_nodes, storage, wallet};

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
    Health,
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
pub(super) struct OperationCatalogEntry {
    method: OperationMethod,
    name: &'static str,
    domain: OperationDomain,
    label: &'static str,
    uses_mutating_flag: bool,
    cancellable: bool,
    exclusive_group: Option<OperationExclusiveGroup>,
}

impl OperationCatalogEntry {
    pub(super) const fn new(
        method: OperationMethod,
        name: &'static str,
        domain: OperationDomain,
        label: &'static str,
    ) -> Self {
        Self {
            method,
            name,
            domain,
            label,
            uses_mutating_flag: false,
            cancellable: false,
            exclusive_group: None,
        }
    }

    pub(super) const fn mutating(
        method: OperationMethod,
        name: &'static str,
        domain: OperationDomain,
        label: &'static str,
    ) -> Self {
        Self {
            method,
            name,
            domain,
            label,
            uses_mutating_flag: true,
            cancellable: false,
            exclusive_group: None,
        }
    }

    pub(super) const fn cancellable(
        method: OperationMethod,
        name: &'static str,
        domain: OperationDomain,
        label: &'static str,
        exclusive_group: OperationExclusiveGroup,
    ) -> Self {
        Self {
            method,
            name,
            domain,
            label,
            uses_mutating_flag: true,
            cancellable: true,
            exclusive_group: Some(exclusive_group),
        }
    }

    fn route(self, start_async: bool) -> OperationRoute {
        OperationRoute {
            domain: self.domain,
            method: self.method,
            label: self.label,
            start_async,
        }
    }
}

const STORAGE_DOWNLOAD_START_ALIAS: &str = "storageDownloadStart";

const OPERATION_CATALOGS: &[&[OperationCatalogEntry]] = &[
    storage::OPERATION_CATALOG,
    delivery::OPERATION_CATALOG,
    local_nodes::OPERATION_CATALOG,
    wallet::OPERATION_CATALOG,
    chain::OPERATION_CATALOG,
];

fn operation_catalog_entries() -> impl Iterator<Item = &'static OperationCatalogEntry> {
    OPERATION_CATALOGS.iter().flat_map(|catalog| catalog.iter())
}

fn operation_catalog_entry(method: OperationMethod) -> OperationCatalogEntry {
    for entry in operation_catalog_entries() {
        if entry.method == method {
            return *entry;
        }
    }
    OperationCatalogEntry::new(method, "operation", OperationDomain::Execution, "Operation")
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
        operation_catalog_entries()
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

    pub(crate) fn uses_mutating_flag(self) -> bool {
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

#[cfg(test)]
pub(crate) fn operation_method_names() -> impl Iterator<Item = &'static str> {
    operation_catalog_entries()
        .map(|entry| entry.name)
        .chain(std::iter::once(STORAGE_DOWNLOAD_START_ALIAS))
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
        let delivery_send =
            OperationMethod::from_str("deliverySend").context("deliverySend should exist")?;
        if !delivery_send.uses_mutating_flag() {
            bail!("deliverySend should require mutating flag");
        }
        let indexer_status =
            OperationMethod::from_str("indexerStatus").context("indexerStatus should exist")?;
        if indexer_status.uses_mutating_flag() {
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

    #[test]
    fn operation_catalogs_are_domain_owned() -> Result<()> {
        let cases = [
            (storage::OPERATION_CATALOG, OperationDomain::Storage),
            (delivery::OPERATION_CATALOG, OperationDomain::Delivery),
            (local_nodes::OPERATION_CATALOG, OperationDomain::LocalNodes),
            (wallet::OPERATION_CATALOG, OperationDomain::Wallet),
        ];
        for (catalog, domain) in cases {
            for entry in catalog {
                if entry.domain != domain {
                    bail!("operation `{}` escaped {domain:?} catalog", entry.name);
                }
            }
        }
        Ok(())
    }
}
