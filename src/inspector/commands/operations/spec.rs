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
    StorageUploadPayload,
    StorageUploadBackupCatalogEntry,
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
pub(super) enum OperationCommand {
    Storage(storage::StorageCommand),
    Delivery(delivery::DeliveryCommand),
    LocalNodes(local_nodes::LocalNodesCommand),
    Wallet(wallet::WalletCommand),
    Blockchain(blockchain::BlockchainCommand),
    Execution(lez::ExecutionCommand),
}

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
pub(super) enum AffectedContextKey {
    Source,
    Endpoint,
    Cid,
    Path,
    Filename,
    BackupCatalogId,
    SlotRange,
    BlockId,
    TransactionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContextPresence {
    Required,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AffectedContextField {
    key: AffectedContextKey,
    presence: ContextPresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OperationPolicyDefinition {
    class: OperationClass,
    affected_context_fields: &'static [AffectedContextField],
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
    command: OperationCommand,
    name: &'static str,
    label: &'static str,
    cancellable: bool,
    exclusive_group: Option<OperationExclusiveGroup>,
    policy: OperationPolicyDefinition,
}

impl OperationDefinition {
    pub(super) const fn new(
        command: OperationCommand,
        name: &'static str,
        label: &'static str,
        class: OperationClass,
    ) -> Self {
        Self {
            command,
            name,
            label,
            cancellable: false,
            exclusive_group: None,
            policy: OperationPolicyDefinition::new(class),
        }
    }

    pub(super) const fn with_context_inputs(
        mut self,
        affected_context_fields: &'static [AffectedContextField],
    ) -> Self {
        self.policy.affected_context_fields = affected_context_fields;
        self
    }

    pub(super) const fn cancellable(mut self, exclusive_group: OperationExclusiveGroup) -> Self {
        self.cancellable = true;
        self.exclusive_group = Some(exclusive_group);
        self
    }

    pub(super) const fn method(self) -> OperationMethod {
        self.command.method()
    }

    pub(super) const fn name(self) -> &'static str {
        self.name
    }

    pub(super) const fn domain(self) -> OperationDomain {
        self.command.domain()
    }

    pub(super) const fn command(self) -> OperationCommand {
        self.command
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
            domain: self.domain(),
            method: self.method(),
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
            affected_context_fields: &[],
            restart_policy,
            confirmation_required,
        }
    }

    pub(super) const fn class(self) -> OperationClass {
        self.class
    }

    pub(super) const fn affected_context_fields(self) -> &'static [AffectedContextField] {
        self.affected_context_fields
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
        .find(|entry| entry.method() == method)
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

impl OperationCommand {
    const fn method(self) -> OperationMethod {
        match self {
            Self::Storage(command) => command.method(),
            Self::Delivery(command) => command.method(),
            Self::LocalNodes(command) => command.method(),
            Self::Wallet(command) => command.method(),
            Self::Blockchain(command) => command.method(),
            Self::Execution(command) => command.method(),
        }
    }

    const fn domain(self) -> OperationDomain {
        match self {
            Self::Storage(_) => OperationDomain::Storage,
            Self::Delivery(_) => OperationDomain::Delivery,
            Self::LocalNodes(_) => OperationDomain::LocalNodes,
            Self::Wallet(_) => OperationDomain::Wallet,
            Self::Blockchain(_) => OperationDomain::Blockchain,
            Self::Execution(_) => OperationDomain::Execution,
        }
    }
}

impl AffectedContextKey {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Endpoint => "endpoint",
            Self::Cid => "cid",
            Self::Path => "path",
            Self::Filename => "filename",
            Self::BackupCatalogId => "backupCatalogId",
            Self::SlotRange => "slotRange",
            Self::BlockId => "blockId",
            Self::TransactionId => "transactionId",
        }
    }
}

impl AffectedContextField {
    pub(super) const fn required(key: AffectedContextKey) -> Self {
        Self {
            key,
            presence: ContextPresence::Required,
        }
    }

    pub(super) const fn optional(key: AffectedContextKey) -> Self {
        Self {
            key,
            presence: ContextPresence::Optional,
        }
    }

    pub(super) const fn key(self) -> AffectedContextKey {
        self.key
    }

    pub(super) const fn presence(self) -> ContextPresence {
        self.presence
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
            .map(|entry| entry.method())
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
    fn operation_command_is_owned_by_method_definition() -> Result<()> {
        let cases = [
            (
                "storageFetch",
                OperationCommand::Storage(storage::StorageCommand::Fetch),
            ),
            (
                "deliverySend",
                OperationCommand::Delivery(delivery::DeliveryCommand::Send),
            ),
            (
                "localNodesAction",
                OperationCommand::LocalNodes(local_nodes::LocalNodesCommand::Action),
            ),
            (
                "localWalletAccounts",
                OperationCommand::Wallet(wallet::WalletCommand::Accounts),
            ),
            (
                "blockchainBlock",
                OperationCommand::Blockchain(blockchain::BlockchainCommand::Block),
            ),
            (
                "localWalletDeployProgram",
                OperationCommand::Execution(lez::ExecutionCommand::DeployProgram),
            ),
        ];

        for (method, command) in cases {
            let definition = OperationMethod::from_str(method)
                .and_then(operation_definition)
                .with_context(|| format!("{method} should exist"))?;
            if definition.command() != command {
                bail!(
                    "unexpected command for {}: {:?}",
                    definition.name(),
                    definition.command()
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
                if entry.domain() != domain {
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
            if OperationMethod::from_str(definition.name) != Some(definition.method()) {
                bail!(
                    "operation definition `{}` does not round trip",
                    definition.name
                );
            }
            let resolved = operation_definition(definition.method())
                .with_context(|| format!("definition missing for `{}`", definition.name))?;
            if resolved.name() != definition.name {
                bail!(
                    "operation method {:?} reports `{}` instead of `{}`",
                    definition.method(),
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
                OperationCommand::Storage(storage::StorageCommand::Manifests),
                "storageManifests",
                "Storage manifests",
                OperationDomain::Storage,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::StorageDownloadManifest,
                OperationCommand::Storage(storage::StorageCommand::DownloadManifest),
                "storageDownloadManifest",
                "Storage manifest",
                OperationDomain::Storage,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::StorageFetch,
                OperationCommand::Storage(storage::StorageCommand::Fetch),
                "storageFetch",
                "Storage fetch",
                OperationDomain::Storage,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::StorageUploadUrl,
                OperationCommand::Storage(storage::StorageCommand::UploadUrl),
                "storageUploadUrl",
                "Storage upload",
                OperationDomain::Storage,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::StorageUploadPayload,
                OperationCommand::Storage(storage::StorageCommand::UploadPayload),
                "storageUploadPayload",
                "Storage payload upload",
                OperationDomain::Storage,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::StorageUploadBackupCatalogEntry,
                OperationCommand::Storage(storage::StorageCommand::UploadBackupCatalogEntry),
                "storageUploadBackupCatalogEntry",
                "Backup upload",
                OperationDomain::Storage,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::StorageDownloadToUrl,
                OperationCommand::Storage(storage::StorageCommand::DownloadToUrl),
                "storageDownloadToUrl",
                "Storage download",
                OperationDomain::Storage,
                OperationClass::Mutating,
                true,
                Some(OperationExclusiveGroup::StorageDownload),
            ),
            (
                OperationMethod::StorageRemove,
                OperationCommand::Storage(storage::StorageCommand::Remove),
                "storageRemove",
                "Storage remove",
                OperationDomain::Storage,
                OperationClass::Destructive,
                false,
                None,
            ),
            (
                OperationMethod::DeliverySubscribe,
                OperationCommand::Delivery(delivery::DeliveryCommand::Subscribe),
                "deliverySubscribe",
                "Delivery subscribe",
                OperationDomain::Delivery,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryUnsubscribe,
                OperationCommand::Delivery(delivery::DeliveryCommand::Unsubscribe),
                "deliveryUnsubscribe",
                "Delivery unsubscribe",
                OperationDomain::Delivery,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::DeliverySend,
                OperationCommand::Delivery(delivery::DeliveryCommand::Send),
                "deliverySend",
                "Delivery send",
                OperationDomain::Delivery,
                OperationClass::Mutating,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryCreateNode,
                OperationCommand::Delivery(delivery::DeliveryCommand::CreateNode),
                "deliveryCreateNode",
                "Delivery create node",
                OperationDomain::Delivery,
                OperationClass::Lifecycle,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryStart,
                OperationCommand::Delivery(delivery::DeliveryCommand::Start),
                "deliveryStart",
                "Delivery start",
                OperationDomain::Delivery,
                OperationClass::Lifecycle,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryStop,
                OperationCommand::Delivery(delivery::DeliveryCommand::Stop),
                "deliveryStop",
                "Delivery stop",
                OperationDomain::Delivery,
                OperationClass::Lifecycle,
                false,
                None,
            ),
            (
                OperationMethod::DeliveryStoreQuery,
                OperationCommand::Delivery(delivery::DeliveryCommand::StoreQuery),
                "deliveryStoreQuery",
                "Delivery store query",
                OperationDomain::Delivery,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::LocalNodesAction,
                OperationCommand::LocalNodes(local_nodes::LocalNodesCommand::Action),
                "localNodesAction",
                "Local node action",
                OperationDomain::LocalNodes,
                OperationClass::Lifecycle,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletCreateAccount,
                OperationCommand::Wallet(wallet::WalletCommand::CreateAccount),
                "localWalletCreateAccount",
                "Wallet account",
                OperationDomain::Wallet,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletSendTransaction,
                OperationCommand::Wallet(wallet::WalletCommand::SendTransaction),
                "localWalletSendTransaction",
                "Wallet send",
                OperationDomain::Wallet,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletInstructionSubmit,
                OperationCommand::Execution(lez::ExecutionCommand::SubmitInstruction),
                "localWalletInstructionSubmit",
                "IDL instruction",
                OperationDomain::Execution,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletCommand,
                OperationCommand::Wallet(wallet::WalletCommand::Command),
                "localWalletCommand",
                "Wallet command",
                OperationDomain::Wallet,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletDeployProgram,
                OperationCommand::Execution(lez::ExecutionCommand::DeployProgram),
                "localWalletDeployProgram",
                "Program deploy",
                OperationDomain::Execution,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletSyncPrivate,
                OperationCommand::Wallet(wallet::WalletCommand::SyncPrivate),
                "localWalletSyncPrivate",
                "Private sync",
                OperationDomain::Wallet,
                OperationClass::SigningSubmission,
                false,
                None,
            ),
            (
                OperationMethod::LocalWalletAccounts,
                OperationCommand::Wallet(wallet::WalletCommand::Accounts),
                "localWalletAccounts",
                "Wallet accounts",
                OperationDomain::Wallet,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainNode,
                OperationCommand::Blockchain(blockchain::BlockchainCommand::Node),
                "blockchainNode",
                "Blockchain node",
                OperationDomain::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainBlocks,
                OperationCommand::Blockchain(blockchain::BlockchainCommand::Blocks),
                "blockchainBlocks",
                "Blockchain blocks",
                OperationDomain::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainLiveBlocks,
                OperationCommand::Blockchain(blockchain::BlockchainCommand::LiveBlocks),
                "blockchainLiveBlocks",
                "Blockchain live blocks",
                OperationDomain::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainBlock,
                OperationCommand::Blockchain(blockchain::BlockchainCommand::Block),
                "blockchainBlock",
                "Blockchain block",
                OperationDomain::Blockchain,
                OperationClass::ReadPoll,
                false,
                None,
            ),
            (
                OperationMethod::BlockchainTransaction,
                OperationCommand::Blockchain(blockchain::BlockchainCommand::Transaction),
                "blockchainTransaction",
                "Blockchain transaction",
                OperationDomain::Blockchain,
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

        for &(method, command, name, label, domain, class, cancellable, exclusive_group) in
            &expected
        {
            let definition = operation_definition(method)
                .with_context(|| format!("definition missing for {method:?}"))?;
            let policy = definition.policy();
            if definition.command() != command
                || definition.name() != name
                || definition.label() != label
                || definition.domain() != domain
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

    #[test]
    fn affected_context_contracts_cover_every_declared_field() -> Result<()> {
        let expected: &[(OperationMethod, &[AffectedContextField])] = &[
            (
                OperationMethod::StorageManifests,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                ],
            ),
            (
                OperationMethod::StorageDownloadManifest,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::Cid),
                ],
            ),
            (
                OperationMethod::StorageFetch,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::Cid),
                ],
            ),
            (
                OperationMethod::StorageUploadUrl,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::Path),
                ],
            ),
            (
                OperationMethod::StorageUploadPayload,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::Filename),
                ],
            ),
            (
                OperationMethod::StorageUploadBackupCatalogEntry,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::BackupCatalogId),
                ],
            ),
            (
                OperationMethod::StorageDownloadToUrl,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::Cid),
                    AffectedContextField::required(AffectedContextKey::Path),
                ],
            ),
            (
                OperationMethod::StorageRemove,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::Cid),
                ],
            ),
            (
                OperationMethod::DeliverySubscribe,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                ],
            ),
            (
                OperationMethod::DeliveryUnsubscribe,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                ],
            ),
            (
                OperationMethod::DeliverySend,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                ],
            ),
            (
                OperationMethod::DeliveryCreateNode,
                &[AffectedContextField::required(AffectedContextKey::Source)],
            ),
            (
                OperationMethod::DeliveryStart,
                &[AffectedContextField::required(AffectedContextKey::Source)],
            ),
            (
                OperationMethod::DeliveryStop,
                &[AffectedContextField::required(AffectedContextKey::Source)],
            ),
            (
                OperationMethod::DeliveryStoreQuery,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::required(AffectedContextKey::Endpoint),
                ],
            ),
            (
                OperationMethod::BlockchainNode,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                ],
            ),
            (
                OperationMethod::BlockchainBlocks,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::SlotRange),
                ],
            ),
            (
                OperationMethod::BlockchainLiveBlocks,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::SlotRange),
                ],
            ),
            (
                OperationMethod::BlockchainBlock,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::BlockId),
                ],
            ),
            (
                OperationMethod::BlockchainTransaction,
                &[
                    AffectedContextField::required(AffectedContextKey::Source),
                    AffectedContextField::optional(AffectedContextKey::Endpoint),
                    AffectedContextField::required(AffectedContextKey::TransactionId),
                ],
            ),
        ];

        for definition in operation_definitions() {
            let expected_fields = expected
                .iter()
                .find(|(method, _)| *method == definition.method())
                .map_or(&[][..], |(_, fields)| *fields);
            let actual = definition.policy().affected_context_fields();
            if actual != expected_fields {
                bail!(
                    "unexpected affected-context contract for {:?}: {actual:?}",
                    definition.method()
                );
            }
        }

        for &(method, _) in expected {
            if operation_definition(method).is_none() {
                bail!("affected-context contract has no definition for {method:?}");
            }
        }
        Ok(())
    }
}
