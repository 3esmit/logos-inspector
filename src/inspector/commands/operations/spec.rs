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

macro_rules! define_operation_methods {
    ($($method:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum OperationMethod {
            $($method),+
        }

        impl OperationMethod {
            #[cfg(test)]
            pub(super) const ALL: &'static [Self] = &[$(Self::$method),+];
        }
    };
}

define_operation_methods!(
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
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationExclusiveGroup {
    StorageDownload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OperationClass {
    Destructive,
    Lifecycle,
    Mutating,
    ReadPoll,
    SigningSubmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RestartPolicy {
    ManualRequired,
    SafeReadPolling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OperationPolicyDefinition {
    class: OperationClass,
    affected_context_keys: &'static [&'static str],
    restart_policy: RestartPolicy,
    confirmation_required: bool,
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
    cancellable: bool,
    exclusive_group: Option<OperationExclusiveGroup>,
    policy: OperationPolicyDefinition,
}

impl OperationDefinition {
    pub(super) const fn new(
        method: OperationMethod,
        name: &'static str,
        domain: OperationDomain,
        label: &'static str,
        class: OperationClass,
    ) -> Self {
        Self {
            method,
            name,
            domain,
            executor: OperationExecutor::for_domain(domain),
            label,
            cancellable: false,
            exclusive_group: None,
            policy: OperationPolicyDefinition::new(class),
        }
    }

    pub(super) const fn with_context_inputs(
        mut self,
        affected_context_keys: &'static [&'static str],
    ) -> Self {
        self.policy.affected_context_keys = affected_context_keys;
        self
    }

    pub(super) const fn cancellable(mut self, exclusive_group: OperationExclusiveGroup) -> Self {
        self.cancellable = true;
        self.exclusive_group = Some(exclusive_group);
        self
    }

    pub(super) const fn method(self) -> OperationMethod {
        self.method
    }

    pub(super) const fn name(self) -> &'static str {
        self.name
    }

    pub(super) const fn domain(self) -> OperationDomain {
        self.domain
    }

    pub(super) const fn executor(self) -> OperationExecutor {
        self.executor
    }

    pub(super) const fn label(self) -> &'static str {
        self.label
    }

    pub(super) const fn is_cancellable(self) -> bool {
        self.cancellable
    }

    pub(super) const fn exclusive_group(self) -> Option<OperationExclusiveGroup> {
        self.exclusive_group
    }

    pub(super) const fn policy(self) -> OperationPolicyDefinition {
        self.policy
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

impl OperationPolicyDefinition {
    pub(super) const fn new(class: OperationClass) -> Self {
        let (restart_policy, confirmation_required) = match class {
            OperationClass::ReadPoll => (RestartPolicy::SafeReadPolling, false),
            OperationClass::Destructive
            | OperationClass::Lifecycle
            | OperationClass::Mutating
            | OperationClass::SigningSubmission => (RestartPolicy::ManualRequired, true),
        };
        Self {
            class,
            affected_context_keys: &[],
            restart_policy,
            confirmation_required,
        }
    }

    pub(super) const fn class(self) -> OperationClass {
        self.class
    }

    pub(super) const fn affected_context_keys(self) -> &'static [&'static str] {
        self.affected_context_keys
    }

    pub(super) const fn restart_policy(self) -> RestartPolicy {
        self.restart_policy
    }

    pub(super) const fn confirmation_required(self) -> bool {
        self.confirmation_required
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

pub(super) fn operation_definition(method: OperationMethod) -> Option<OperationDefinition> {
    operation_definitions()
        .find(|entry| entry.method == method)
        .copied()
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

impl OperationClass {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Destructive => "destructive",
            Self::Lifecycle => "lifecycle",
            Self::Mutating => "mutating",
            Self::ReadPoll => "read_poll",
            Self::SigningSubmission => "signing_submission",
        }
    }
}

impl RestartPolicy {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ManualRequired => "manual_required",
            Self::SafeReadPolling => "safe_read_polling",
        }
    }
}

impl OperationMethod {
    pub(crate) fn from_str(method: &str) -> Option<Self> {
        operation_definitions()
            .find(|entry| entry.name == method)
            .map(|entry| entry.method)
    }

    #[cfg(test)]
    pub(super) fn definition(self) -> Option<OperationDefinition> {
        operation_definition(self)
    }
}

pub(crate) fn operation_route(method: &str) -> Option<OperationRoute> {
    if method == STORAGE_DOWNLOAD_START_ALIAS {
        return operation_definition(OperationMethod::StorageDownloadToUrl)
            .map(|definition| definition.route(true));
    }
    let method = OperationMethod::from_str(method)?;
    operation_definition(method).map(|definition| definition.route(false))
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
            let definition = operation_definition(route.method)
                .with_context(|| format!("definition missing for {name}"))?;
            if route.domain != domain || definition.name() != name || route.label != label {
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
        let storage_download = OperationMethod::from_str("storageDownloadToUrl")
            .and_then(operation_definition)
            .context("storageDownloadToUrl should exist")?;
        if !storage_download.is_cancellable() {
            bail!("storageDownloadToUrl should be cancellable");
        }
        if storage_download.exclusive_group() != Some(OperationExclusiveGroup::StorageDownload) {
            bail!("storageDownloadToUrl should own the storage download exclusive group");
        }
        let storage_upload = OperationMethod::from_str("storageUploadUrl")
            .and_then(operation_definition)
            .context("storageUploadUrl should exist")?;
        if storage_upload.is_cancellable() {
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
            let definition = OperationMethod::from_str(method)
                .and_then(operation_definition)
                .with_context(|| format!("{method} should exist"))?;
            if definition.executor() != executor {
                bail!(
                    "unexpected executor for {}: {:?}",
                    definition.name(),
                    definition.executor()
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
            (
                blockchain::OPERATION_DEFINITIONS,
                OperationDomain::Blockchain,
            ),
            (lez::OPERATION_DEFINITIONS, OperationDomain::Execution),
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
            let resolved = operation_definition(definition.method)
                .with_context(|| format!("definition missing for `{}`", definition.name))?;
            if resolved.name() != definition.name {
                bail!(
                    "operation method {:?} reports `{}` instead of `{}`",
                    definition.method,
                    resolved.name(),
                    definition.name
                );
            }
        }

        if names.contains(STORAGE_DOWNLOAD_START_ALIAS) {
            bail!("storage download alias collides with a direct operation name");
        }

        Ok(())
    }

    #[test]
    fn operation_definitions_cover_every_method_once_and_match_policy_contract() -> Result<()> {
        let expected = [
            (
                OperationMethod::StorageManifests,
                OperationExecutor::Storage,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::StorageDownloadManifest,
                OperationExecutor::Storage,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::StorageFetch,
                OperationExecutor::Storage,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::StorageUploadUrl,
                OperationExecutor::Storage,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::StorageDownloadToUrl,
                OperationExecutor::Storage,
                OperationClass::Mutating,
                true,
                Some(OperationExclusiveGroup::StorageDownload),
            ),
            (
                OperationMethod::StorageRemove,
                OperationExecutor::Storage,
                OperationClass::Destructive,
                false,
                None,
            ),
            (
                OperationMethod::DeliverySubscribe,
                OperationExecutor::Delivery,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryUnsubscribe,
                OperationExecutor::Delivery,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::DeliverySend,
                OperationExecutor::Delivery,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryCreateNode,
                OperationExecutor::Delivery,
                OperationClass::Lifecycle,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryStart,
                OperationExecutor::Delivery,
                OperationClass::Lifecycle,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryStop,
                OperationExecutor::Delivery,
                OperationClass::Lifecycle,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryStoreQuery,
                OperationExecutor::Delivery,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::LocalNodesAction,
                OperationExecutor::LocalNodes,
                OperationClass::Lifecycle,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletCreateAccount,
                OperationExecutor::Wallet,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletSendTransaction,
                OperationExecutor::Wallet,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletInstructionSubmit,
                OperationExecutor::Lez,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletCommand,
                OperationExecutor::Wallet,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletDeployProgram,
                OperationExecutor::Lez,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletSyncPrivate,
                OperationExecutor::Wallet,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletAccounts,
                OperationExecutor::Wallet,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainNode,
                OperationExecutor::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainBlocks,
                OperationExecutor::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainLiveBlocks,
                OperationExecutor::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainBlock,
                OperationExecutor::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainTransaction,
                OperationExecutor::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
        ];

        if expected.len() != OperationMethod::ALL.len()
            || operation_definitions().count() != OperationMethod::ALL.len()
        {
            bail!(
                "operation inventory drifted: {} expected rows, {} methods, {} definitions",
                expected.len(),
                OperationMethod::ALL.len(),
                operation_definitions().count()
            );
        }

        for &method in OperationMethod::ALL {
            let expected_rows = expected
                .iter()
                .filter(|(expected_method, ..)| *expected_method == method)
                .count();
            let definition_rows = operation_definitions()
                .filter(|definition| definition.method() == method)
                .count();
            if expected_rows != 1 || definition_rows != 1 {
                bail!(
                    "{method:?} has {expected_rows} expected rows and {definition_rows} definitions"
                );
            }
        }

        for &(method, executor, class, cancellable, exclusive_group) in &expected {
            let definition = operation_definition(method)
                .with_context(|| format!("definition missing for {method:?}"))?;
            let policy = definition.policy();
            if definition.executor() != executor
                || definition.is_cancellable() != cancellable
                || definition.exclusive_group() != exclusive_group
                || policy.class() != class
            {
                bail!("unexpected operation facts for {method:?}: {definition:?}");
            }
            let read_poll = class == OperationClass::ReadPoll;
            if policy.restart_policy()
                != if read_poll {
                    RestartPolicy::SafeReadPolling
                } else {
                    RestartPolicy::ManualRequired
                }
                || policy.confirmation_required() == read_poll
            {
                bail!("inconsistent derived policy facts for {method:?}: {policy:?}");
            }
        }

        Ok(())
    }
}
