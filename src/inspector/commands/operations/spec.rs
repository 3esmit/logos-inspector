use super::{blockchain, delivery, lez, local_nodes, storage, wallet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationDomain {
    Storage,
    Delivery,
    LocalNodes,
    Wallet,
    Blockchain,
    Execution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OperationExecutor {
    Storage,
    Delivery,
    LocalNodes,
    Wallet,
    Blockchain,
    Lez,
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
pub(super) struct OperationDefinition {
    method: OperationMethod,
    name: &'static str,
    domain: OperationDomain,
    executor: OperationExecutor,
    label: &'static str,
    uses_mutating_flag: bool,
    cancellable: bool,
    exclusive_group: Option<OperationExclusiveGroup>,
}

impl OperationDefinition {
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
            executor: OperationExecutor::for_domain(domain),
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
            executor: OperationExecutor::for_domain(domain),
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
            executor: OperationExecutor::for_domain(domain),
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

const OPERATION_DEFINITION_SETS: &[&[OperationDefinition]] = &[
    storage::OPERATION_DEFINITIONS,
    delivery::OPERATION_DEFINITIONS,
    local_nodes::OPERATION_DEFINITIONS,
    wallet::OPERATION_DEFINITIONS,
    blockchain::OPERATION_DEFINITIONS,
    lez::OPERATION_DEFINITIONS,
];

fn operation_definitions() -> impl Iterator<Item = &'static OperationDefinition> {
    OPERATION_DEFINITION_SETS
        .iter()
        .flat_map(|catalog| catalog.iter())
}

fn operation_definition(method: OperationMethod) -> OperationDefinition {
    for entry in operation_definitions() {
        if entry.method == method {
            return *entry;
        }
    }
    OperationDefinition::new(method, "operation", OperationDomain::Execution, "Operation")
}

impl OperationDomain {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Storage => "storage",
            Self::Delivery => "delivery",
            Self::LocalNodes => "localNodes",
            Self::Wallet => "wallet",
            Self::Blockchain => "blockchain",
            Self::Execution => "execution",
        }
    }
}

impl OperationExecutor {
    const fn for_domain(domain: OperationDomain) -> Self {
        match domain {
            OperationDomain::Storage => Self::Storage,
            OperationDomain::Delivery => Self::Delivery,
            OperationDomain::LocalNodes => Self::LocalNodes,
            OperationDomain::Wallet => Self::Wallet,
            OperationDomain::Blockchain => Self::Blockchain,
            OperationDomain::Execution => Self::Lez,
        }
    }
}

impl OperationMethod {
    pub(crate) fn from_str(method: &str) -> Option<Self> {
        operation_definitions()
            .find(|entry| entry.name == method)
            .map(|entry| entry.method)
    }

    pub(crate) fn as_str(self) -> &'static str {
        operation_definition(self).name
    }

    pub(crate) fn domain(self) -> OperationDomain {
        operation_definition(self).domain
    }

    pub(super) fn executor(self) -> OperationExecutor {
        operation_definition(self).executor
    }

    pub(crate) fn label(self) -> &'static str {
        operation_definition(self).label
    }

    pub(crate) fn uses_mutating_flag(self) -> bool {
        operation_definition(self).uses_mutating_flag
    }

    pub(crate) fn cancellable(self) -> bool {
        operation_definition(self).cancellable
    }

    pub(crate) fn exclusive_group(self) -> Option<OperationExclusiveGroup> {
        operation_definition(self).exclusive_group
    }
}

pub(crate) fn operation_route(method: &str) -> Option<OperationRoute> {
    if method == STORAGE_DOWNLOAD_START_ALIAS {
        return Some(operation_definition(OperationMethod::StorageDownloadToUrl).route(true));
    }
    let method = OperationMethod::from_str(method)?;
    Some(operation_definition(method).route(false))
}

#[cfg(test)]
pub(crate) fn operation_method_names() -> impl Iterator<Item = &'static str> {
    operation_definitions()
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
            (
                "localWalletDeployProgram",
                OperationDomain::Execution,
                "Program deploy",
            ),
            (
                "localWalletInstructionSubmit",
                OperationDomain::Execution,
                "IDL instruction",
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
    fn operation_flags_are_owned_by_method_definition() -> Result<()> {
        let delivery_send =
            OperationMethod::from_str("deliverySend").context("deliverySend should exist")?;
        if !delivery_send.uses_mutating_flag() {
            bail!("deliverySend should require mutating flag");
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
    fn operation_executor_is_owned_by_method_definition() -> Result<()> {
        let cases = [
            ("storageFetch", OperationExecutor::Storage),
            ("deliverySend", OperationExecutor::Delivery),
            ("localNodesAction", OperationExecutor::LocalNodes),
            ("localWalletAccounts", OperationExecutor::Wallet),
            ("blockchainBlock", OperationExecutor::Blockchain),
            ("localWalletDeployProgram", OperationExecutor::Lez),
        ];

        for (method, executor) in cases {
            let method = OperationMethod::from_str(method)
                .with_context(|| format!("{method} should exist"))?;
            if method.executor() != executor {
                bail!(
                    "unexpected executor for {}: {:?}",
                    method.as_str(),
                    method.executor()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn operation_definitions_are_domain_owned() -> Result<()> {
        let cases = [
            (storage::OPERATION_DEFINITIONS, OperationDomain::Storage),
            (delivery::OPERATION_DEFINITIONS, OperationDomain::Delivery),
            (
                local_nodes::OPERATION_DEFINITIONS,
                OperationDomain::LocalNodes,
            ),
            (wallet::OPERATION_DEFINITIONS, OperationDomain::Wallet),
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

    #[test]
    fn operation_definitions_have_unique_names_and_round_trip() -> Result<()> {
        let mut names = std::collections::BTreeSet::new();

        for definition in operation_definitions() {
            if !names.insert(definition.name) {
                bail!("duplicate operation definition name `{}`", definition.name);
            }
            if OperationMethod::from_str(definition.name) != Some(definition.method) {
                bail!(
                    "operation definition `{}` does not round trip",
                    definition.name
                );
            }
            if definition.method.as_str() != definition.name {
                bail!(
                    "operation method {:?} reports `{}` instead of `{}`",
                    definition.method,
                    definition.method.as_str(),
                    definition.name
                );
            }
        }

        if names.contains(STORAGE_DOWNLOAD_START_ALIAS) {
            bail!("storage download alias collides with a direct operation name");
        }

        Ok(())
    }
}
