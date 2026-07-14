use std::{
    collections::BTreeSet,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

#[cfg(test)]
use std::fs;

use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value, json};

use crate::{
    inspection::NetworkScope,
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
    support::{
        config_path::settings_state_path,
        local_state::{
            DirectoryDurability, LocalStateSession, StateFile, StoredBytes, lock_local_state_in,
        },
    },
};

use super::config::{
    ChannelSourceConfig, ChannelSourceConfigApplyRequest, ChannelSourceConfigMutation,
    ChannelSourceRole, ChannelSourceTarget, ConfiguredIndexerSource, ConfiguredSequencerSource,
    PersistedSequencerAttestation, SequencerAttestationReceipt, generate_source_id,
    normalize_channel_id, normalize_channel_source_configs, normalize_label,
    normalize_network_scope, validate_source_id,
};
use super::probe::{
    ChannelSourceFailureKind, DefaultSequencerTargetAttestor, SequencerTargetAttestor,
};

const SETTINGS_VERSION: u64 = 2;
const CHANNEL_SOURCE_CONFIGS_KEY: &str = "channel_source_configs";
const SOURCE_ID_GENERATION_ATTEMPTS: usize = 8;

pub(crate) type ChannelSourceConfigMutationFuture =
    Pin<Box<dyn Future<Output = Result<ChannelSourceConfigApplyOutcome>> + Send + 'static>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelSourceAttestationOutcome {
    NotRequired,
    Persisted,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelSourceConfigApplyOutcome {
    pub(crate) config: ChannelSourceConfig,
    pub(crate) attestation: ChannelSourceAttestationOutcome,
}

pub(crate) trait ChannelSourceConfigMutationInterface: Send + Sync + 'static {
    fn load(&self) -> Result<Vec<ChannelSourceConfig>>;

    fn apply(
        self: Arc<Self>,
        request: ChannelSourceConfigApplyRequest,
    ) -> ChannelSourceConfigMutationFuture;
}

#[derive(Clone)]
pub(crate) struct SettingsChannelSourceConfigMutation {
    path: Option<PathBuf>,
    attestor: Arc<dyn SequencerTargetAttestor>,
}

impl SettingsChannelSourceConfigMutation {
    #[must_use]
    pub(crate) fn with_module_transport(
        module_transport: SharedModuleTransport,
        module_transport_kind: ModuleTransportKind,
    ) -> Self {
        Self {
            path: None,
            attestor: Arc::new(DefaultSequencerTargetAttestor::with_module_transport(
                module_transport,
                module_transport_kind,
            )),
        }
    }

    async fn apply_request(
        &self,
        request: ChannelSourceConfigApplyRequest,
    ) -> Result<ChannelSourceConfigApplyOutcome> {
        let store = self.store()?;
        let prepared = store.prepare_mutation(request)?;
        let (receipt, attestation) = self.attest(&prepared).await?;
        let config = store.commit_prepared_mutation(prepared, receipt)?;
        Ok(ChannelSourceConfigApplyOutcome {
            config,
            attestation,
        })
    }

    async fn attest(
        &self,
        prepared: &PreparedChannelSourceMutation,
    ) -> Result<(
        Option<SequencerAttestationReceipt>,
        ChannelSourceAttestationOutcome,
    )> {
        let Some(plan) = prepared.attestation.as_ref() else {
            return Ok((None, ChannelSourceAttestationOutcome::NotRequired));
        };
        match self.attestor.clone().attest(plan.target.clone()).await {
            Ok(reported_channel_id) => {
                let reported_channel_id = normalize_channel_id(&reported_channel_id)?;
                if reported_channel_id != prepared.channel_id {
                    bail!("Sequencer attestation reported another Channel");
                }
                Ok((
                    Some(SequencerAttestationReceipt {
                        reported_channel_id,
                        target_fingerprint: plan.target.fingerprint(),
                        attested_at_unix: crate::support::time::now_millis() / 1_000,
                    }),
                    ChannelSourceAttestationOutcome::Persisted,
                ))
            }
            Err(failure)
                if plan.allow_pending
                    && matches!(
                        failure.kind,
                        ChannelSourceFailureKind::Timeout | ChannelSourceFailureKind::Unavailable
                    ) =>
            {
                Ok((None, ChannelSourceAttestationOutcome::Pending))
            }
            Err(failure) => {
                Err(anyhow::Error::new(failure).context("Sequencer Channel attestation failed"))
            }
        }
    }

    fn store(&self) -> Result<SettingsStore> {
        self.path
            .clone()
            .map(SettingsStore::new)
            .map_or_else(|| settings_state_path().map(SettingsStore::new), Ok)
    }

    #[cfg(test)]
    fn with_store(store: &SettingsStore, attestor: Arc<dyn SequencerTargetAttestor>) -> Self {
        Self {
            path: Some(store.path.clone()),
            attestor,
        }
    }
}

impl ChannelSourceConfigMutationInterface for SettingsChannelSourceConfigMutation {
    fn load(&self) -> Result<Vec<ChannelSourceConfig>> {
        let store = self.store()?;
        let guard = settings_guard(&store.path)?;
        Ok(store.load_document_locked(&guard)?.channel_source_configs)
    }

    fn apply(
        self: Arc<Self>,
        request: ChannelSourceConfigApplyRequest,
    ) -> ChannelSourceConfigMutationFuture {
        Box::pin(async move { self.apply_request(request).await })
    }
}

struct PreparedChannelSourceMutation {
    network_scope: NetworkScope,
    channel_id: String,
    expected_config_revision: u64,
    mutation: ChannelSourceConfigMutation,
    base_config: Option<ChannelSourceConfig>,
    attestation: Option<SequencerAttestationPlan>,
}

struct SequencerAttestationPlan {
    target: ChannelSourceTarget,
    allow_pending: bool,
}

pub fn load_channel_source_configs() -> Result<Vec<ChannelSourceConfig>> {
    let store = SettingsStore::new(settings_state_path()?);
    let guard = settings_guard(&store.path)?;
    Ok(store.load_document_locked(&guard)?.channel_source_configs)
}

#[deprecated(note = "use the channelSourceConfigApply command transaction")]
pub fn apply_channel_source_config(
    request: ChannelSourceConfigApplyRequest,
) -> Result<ChannelSourceConfig> {
    SettingsStore::new(settings_state_path()?).apply(request)
}

#[deprecated(note = "use the channelSourceConfigApply retry_attestation command transaction")]
pub fn record_sequencer_attestation(
    network_scope: NetworkScope,
    channel_id: &str,
    expected_config_revision: u64,
    source_id: &str,
    receipt: SequencerAttestationReceipt,
) -> Result<ChannelSourceConfig> {
    SettingsStore::new(settings_state_path()?).record_attestation(
        network_scope,
        channel_id,
        expected_config_revision,
        source_id,
        receipt,
    )
}

pub(crate) fn rebind_channel_source_configs(
    old_scope: NetworkScope,
    new_scope: NetworkScope,
) -> Result<()> {
    SettingsStore::new(settings_state_path()?).rebind_network_scope(old_scope, new_scope)
}

pub(crate) fn load_settings_state() -> Result<Value> {
    SettingsStore::new(settings_state_path()?).load()
}

pub(crate) fn save_user_settings_state(state: &Value) -> Result<Value> {
    SettingsStore::new(settings_state_path()?).save_user_settings(state)
}

pub(crate) fn settings_state_from_stored(stored: &StoredBytes) -> Result<Value> {
    let document = match stored {
        StoredBytes::Missing => SettingsDocument::default(),
        StoredBytes::Present(bytes) => {
            let value = serde_json::from_slice(bytes).context("failed to parse settings state")?;
            SettingsDocument::from_value(value)?
        }
    };
    document.into_value()
}

pub(crate) fn normalized_settings_state_from_backup(
    state: &Value,
    current_state: &Value,
) -> Result<Value> {
    let mut incoming = SettingsDocument::from_value(state.clone())?;
    let current = SettingsDocument::from_value(current_state.clone())?;
    rebase_imported_config_revisions(
        &mut incoming.channel_source_configs,
        &current.channel_source_configs,
    )?;
    incoming.into_value()
}

#[derive(Debug, Clone)]
struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load(&self) -> Result<Value> {
        let guard = settings_guard(&self.path)?;
        self.load_document_locked(&guard)?.into_value()
    }

    fn save_user_settings(&self, state: &Value) -> Result<Value> {
        let mut guard = settings_guard(&self.path)?;
        let current = self.load_document_locked(&guard)?;
        let incoming = SettingsDocument::from_user_value(state.clone())?;
        let document = SettingsDocument {
            fields: incoming.fields,
            channel_source_configs: current.channel_source_configs,
        };
        let durability = self.write_document_locked(&mut guard, &document)?;
        Ok(saved_report(&self.path, durability))
    }

    #[cfg(test)]
    fn replace_from_backup(&self, state: &Value) -> Result<Value> {
        let mut guard = settings_guard(&self.path)?;
        let document = SettingsDocument::from_value(state.clone())?;
        let durability = self.write_document_locked(&mut guard, &document)?;
        Ok(saved_report(&self.path, durability))
    }

    fn apply(&self, request: ChannelSourceConfigApplyRequest) -> Result<ChannelSourceConfig> {
        self.apply_with_attestation(request, None)
    }

    fn prepare_mutation(
        &self,
        request: ChannelSourceConfigApplyRequest,
    ) -> Result<PreparedChannelSourceMutation> {
        let guard = settings_guard(&self.path)?;
        let document = self.load_document_locked(&guard)?;
        let network_scope = normalize_network_scope(request.network_scope)?;
        let channel_id = normalize_channel_id(&request.channel_id)?;
        let base_config = document
            .channel_source_configs
            .iter()
            .find(|config| config.network_scope == network_scope && config.channel_id == channel_id)
            .cloned();
        let current_revision = base_config
            .as_ref()
            .map_or(0, |config| config.config_revision);
        if current_revision != request.expected_config_revision {
            bail!(
                "Channel source configuration revision conflict: expected {}, current {current_revision}",
                request.expected_config_revision
            );
        }
        let (mutation, attestation) =
            normalized_mutation_plan(request.mutation, base_config.as_ref())?;
        Ok(PreparedChannelSourceMutation {
            network_scope,
            channel_id,
            expected_config_revision: request.expected_config_revision,
            mutation,
            base_config,
            attestation,
        })
    }

    fn commit_prepared_mutation(
        &self,
        prepared: PreparedChannelSourceMutation,
        attestation: Option<SequencerAttestationReceipt>,
    ) -> Result<ChannelSourceConfig> {
        let mut guard = settings_guard(&self.path)?;
        let mut document = self.load_document_locked(&guard)?;
        let current = document
            .channel_source_configs
            .iter()
            .find(|config| {
                config.network_scope == prepared.network_scope
                    && config.channel_id == prepared.channel_id
            })
            .cloned();
        if current != prepared.base_config {
            let current_revision = current.as_ref().map_or(0, |config| config.config_revision);
            bail!(
                "Channel source configuration revision conflict: exact target changed after revision {} was read, current {current_revision}",
                prepared.expected_config_revision
            );
        }

        let mut config = current.unwrap_or_else(|| ChannelSourceConfig {
            network_scope: prepared.network_scope.clone(),
            channel_id: prepared.channel_id.clone(),
            config_revision: 0,
            sequencer_sources: Vec::new(),
            selected_sequencer_source_id: None,
            indexer_source: None,
        });
        let mut source_ids = configured_source_ids(&document.channel_source_configs);
        apply_mutation(&mut config, prepared.mutation, &mut source_ids, attestation)?;
        config.config_revision = prepared
            .expected_config_revision
            .checked_add(1)
            .context("Channel source configuration revision overflow")?;
        config = config.normalized()?;

        if let Some(current) = document.channel_source_configs.iter_mut().find(|current| {
            current.network_scope == prepared.network_scope
                && current.channel_id == prepared.channel_id
        }) {
            *current = config.clone();
        } else {
            document.channel_source_configs.push(config.clone());
        }
        self.write_document_locked(&mut guard, &document)?;
        Ok(config)
    }

    fn apply_with_attestation(
        &self,
        request: ChannelSourceConfigApplyRequest,
        attestation: Option<SequencerAttestationReceipt>,
    ) -> Result<ChannelSourceConfig> {
        let prepared = self.prepare_mutation(request)?;
        self.commit_prepared_mutation(prepared, attestation)
    }

    fn record_attestation(
        &self,
        network_scope: NetworkScope,
        channel_id: &str,
        expected_config_revision: u64,
        source_id: &str,
        receipt: SequencerAttestationReceipt,
    ) -> Result<ChannelSourceConfig> {
        let mut guard = settings_guard(&self.path)?;
        let mut document = self.load_document_locked(&guard)?;
        let network_scope = normalize_network_scope(network_scope)?;
        let channel_id = normalize_channel_id(channel_id)?;
        let source_id = validate_source_id(source_id)?;
        let reported_channel_id = normalize_channel_id(&receipt.reported_channel_id)?;
        if reported_channel_id != channel_id {
            bail!("Sequencer attestation reported another Channel");
        }
        let Some(config) = document.channel_source_configs.iter_mut().find(|config| {
            config.network_scope == network_scope && config.channel_id == channel_id
        }) else {
            bail!("Channel source configuration does not exist");
        };
        if config.config_revision != expected_config_revision {
            bail!(
                "Channel source configuration revision conflict: expected {expected_config_revision}, current {}",
                config.config_revision
            );
        }
        let Some(source) = config
            .sequencer_sources
            .iter_mut()
            .find(|source| source.source_id == source_id)
        else {
            bail!("Sequencer source `{source_id}` does not exist");
        };
        let expected_fingerprint = source.target.fingerprint();
        if receipt.target_fingerprint != expected_fingerprint {
            bail!("Sequencer attestation target fingerprint is stale");
        }
        source.channel_attestation = PersistedSequencerAttestation::PersistedAttested {
            channel_id,
            target_fingerprint: expected_fingerprint,
            attested_at_unix: receipt.attested_at_unix,
        };
        config.config_revision = config
            .config_revision
            .checked_add(1)
            .context("Channel source configuration revision overflow")?;
        let updated = config.clone().normalized()?;
        *config = updated.clone();
        self.write_document_locked(&mut guard, &document)?;
        Ok(updated)
    }

    fn rebind_network_scope(&self, old_scope: NetworkScope, new_scope: NetworkScope) -> Result<()> {
        let mut guard = settings_guard(&self.path)?;
        let old_scope = normalize_network_scope(old_scope)?;
        let new_scope = normalize_network_scope(new_scope)?;
        if old_scope == new_scope {
            return Ok(());
        }
        let mut document = self.load_document_locked(&guard)?;
        let current = std::mem::take(&mut document.channel_source_configs);
        let mut retained = Vec::with_capacity(current.len());
        let mut moved = Vec::new();
        for mut config in current {
            if config.network_scope == old_scope {
                config.network_scope = new_scope.clone();
                moved.push(config);
            } else {
                retained.push(config);
            }
        }
        if moved.is_empty() {
            document.channel_source_configs = retained;
            return Ok(());
        }
        for config in moved {
            if let Some(existing) = retained.iter().find(|existing| {
                existing.network_scope == new_scope && existing.channel_id == config.channel_id
            }) {
                if existing != &config {
                    bail!(
                        "Channel source configuration conflicts at promoted network scope for `{}`",
                        config.channel_id
                    );
                }
            } else {
                retained.push(config);
            }
        }
        document.channel_source_configs = retained;
        self.write_document_locked(&mut guard, &document)
            .map(|_| ())
    }

    fn load_document_locked(&self, guard: &LocalStateSession) -> Result<SettingsDocument> {
        match guard.read(StateFile::Settings)? {
            StoredBytes::Missing => Ok(SettingsDocument::default()),
            StoredBytes::Present(bytes) => {
                let value = serde_json::from_slice(&bytes).with_context(|| {
                    format!(
                        "failed to parse settings state from {}",
                        self.path.display()
                    )
                })?;
                SettingsDocument::from_value(value)
            }
        }
    }

    fn write_document_locked(
        &self,
        guard: &mut LocalStateSession,
        document: &SettingsDocument,
    ) -> Result<DirectoryDurability> {
        let value = document.clone().into_value()?;
        let bytes =
            serde_json::to_vec_pretty(&value).context("failed to serialize settings state")?;
        guard.atomic_replace(StateFile::Settings, &bytes)
    }

    #[cfg(test)]
    fn load_document_unlocked(&self) -> Result<SettingsDocument> {
        let guard = settings_guard(&self.path)?;
        self.load_document_locked(&guard)
    }

    #[cfg(test)]
    fn write_document_unlocked(&self, document: &SettingsDocument) -> Result<()> {
        let mut guard = settings_guard(&self.path)?;
        self.write_document_locked(&mut guard, document).map(|_| ())
    }

    #[cfg(test)]
    fn write_document_with_hook<F>(
        &self,
        document: &SettingsDocument,
        before_replace: F,
    ) -> Result<()>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
        let mut guard = settings_guard(&self.path)?;
        let value = document.clone().into_value()?;
        let bytes =
            serde_json::to_vec_pretty(&value).context("failed to serialize settings state")?;
        before_replace(&self.path)?;
        guard
            .atomic_replace(StateFile::Settings, &bytes)
            .map(|_| ())
    }
}

fn normalized_mutation_plan(
    mutation: ChannelSourceConfigMutation,
    config: Option<&ChannelSourceConfig>,
) -> Result<(
    ChannelSourceConfigMutation,
    Option<SequencerAttestationPlan>,
)> {
    match mutation {
        ChannelSourceConfigMutation::AddSequencer {
            label,
            target,
            allow_insecure_http,
        } => {
            let label = normalize_label(label)?;
            let target = target.normalized(ChannelSourceRole::Sequencer, allow_insecure_http)?;
            Ok((
                ChannelSourceConfigMutation::AddSequencer {
                    label,
                    target: target.clone(),
                    allow_insecure_http,
                },
                Some(SequencerAttestationPlan {
                    target,
                    allow_pending: true,
                }),
            ))
        }
        ChannelSourceConfigMutation::UpdateSequencer {
            source_id,
            label,
            target,
            allow_insecure_http,
        } => {
            let source_id = validate_source_id(&source_id)?;
            let label = normalize_label(label)?;
            let target = target.normalized(ChannelSourceRole::Sequencer, allow_insecure_http)?;
            let source = required_sequencer_source(config, &source_id)?;
            let attestation = (source.target != target).then(|| SequencerAttestationPlan {
                target: target.clone(),
                allow_pending: true,
            });
            Ok((
                ChannelSourceConfigMutation::UpdateSequencer {
                    source_id,
                    label,
                    target,
                    allow_insecure_http,
                },
                attestation,
            ))
        }
        ChannelSourceConfigMutation::RemoveSequencer { source_id } => {
            let source_id = validate_source_id(&source_id)?;
            required_sequencer_source(config, &source_id)?;
            Ok((
                ChannelSourceConfigMutation::RemoveSequencer { source_id },
                None,
            ))
        }
        ChannelSourceConfigMutation::SelectSequencer { source_id } => {
            let source_id = source_id.as_deref().map(validate_source_id).transpose()?;
            if let Some(source_id) = source_id.as_ref() {
                required_sequencer_source(config, source_id)?;
            }
            Ok((
                ChannelSourceConfigMutation::SelectSequencer { source_id },
                None,
            ))
        }
        ChannelSourceConfigMutation::RetryAttestation { source_id } => {
            let source_id = validate_source_id(&source_id)?;
            let source = required_sequencer_source(config, &source_id)?;
            Ok((
                ChannelSourceConfigMutation::RetryAttestation { source_id },
                Some(SequencerAttestationPlan {
                    target: source.target.clone(),
                    allow_pending: false,
                }),
            ))
        }
        ChannelSourceConfigMutation::SetIndexer {
            label,
            target,
            allow_insecure_http,
        } => Ok((
            ChannelSourceConfigMutation::SetIndexer {
                label: normalize_label(label)?,
                target: target.normalized(ChannelSourceRole::Indexer, allow_insecure_http)?,
                allow_insecure_http,
            },
            None,
        )),
        ChannelSourceConfigMutation::RemoveIndexer => {
            if config
                .and_then(|config| config.indexer_source.as_ref())
                .is_none()
            {
                bail!("Indexer source does not exist");
            }
            Ok((ChannelSourceConfigMutation::RemoveIndexer, None))
        }
    }
}

fn required_sequencer_source<'a>(
    config: Option<&'a ChannelSourceConfig>,
    source_id: &str,
) -> Result<&'a ConfiguredSequencerSource> {
    config
        .and_then(|config| {
            config
                .sequencer_sources
                .iter()
                .find(|source| source.source_id == source_id)
        })
        .with_context(|| format!("Sequencer source `{source_id}` does not exist"))
}

#[derive(Debug, Clone)]
struct SettingsDocument {
    fields: Map<String, Value>,
    channel_source_configs: Vec<ChannelSourceConfig>,
}

impl SettingsDocument {
    fn from_user_value(value: Value) -> Result<Self> {
        let mut fields = value
            .as_object()
            .cloned()
            .context("settings state must be an object")?;
        settings_version(fields.get("version"))?;
        fields.remove("version");
        fields.remove(CHANNEL_SOURCE_CONFIGS_KEY);
        Ok(Self {
            fields,
            channel_source_configs: Vec::new(),
        })
    }

    fn from_value(value: Value) -> Result<Self> {
        let mut fields = value
            .as_object()
            .cloned()
            .context("settings state must be an object")?;
        let version = settings_version(fields.get("version"))?;
        let configs = fields
            .remove(CHANNEL_SOURCE_CONFIGS_KEY)
            .unwrap_or_else(|| Value::Array(Vec::new()));
        fields.remove("version");
        let channel_source_configs = if version == SETTINGS_VERSION {
            let configs = serde_json::from_value(configs)
                .context("Channel source configuration list is invalid")?;
            normalize_channel_source_configs(configs)?
        } else {
            Vec::new()
        };
        Ok(Self {
            fields,
            channel_source_configs,
        })
    }

    fn into_value(mut self) -> Result<Value> {
        self.channel_source_configs =
            normalize_channel_source_configs(self.channel_source_configs)?;
        self.fields
            .insert("version".to_owned(), Value::from(SETTINGS_VERSION));
        self.fields.insert(
            CHANNEL_SOURCE_CONFIGS_KEY.to_owned(),
            serde_json::to_value(self.channel_source_configs)
                .context("failed to serialize Channel source configurations")?,
        );
        Ok(Value::Object(self.fields))
    }
}

impl Default for SettingsDocument {
    fn default() -> Self {
        let fields = json!({
            "blockchain_refresh_rate": 30,
            "messaging_refresh_rate": 30,
            "storage_refresh_rate": 30,
            "social_identities": [],
            "social_identity_default_mode": "perConversation",
            "social_selected_identity_key": "",
            "social_conversation_identity_keys": {},
            "shared_idl_policy": "suggestion",
            "shared_idl_auto_share": false,
            "social_auto_shared_idls": {},
            "favorites": []
        })
        .as_object()
        .cloned()
        .unwrap_or_default();
        Self {
            fields,
            channel_source_configs: Vec::new(),
        }
    }
}

fn rebase_imported_config_revisions(
    incoming: &mut [ChannelSourceConfig],
    current: &[ChannelSourceConfig],
) -> Result<()> {
    for config in incoming {
        let existing = current.iter().find(|candidate| {
            candidate.network_scope == config.network_scope
                && candidate.channel_id == config.channel_id
        });
        config.config_revision = match existing {
            None => 1,
            Some(existing) if same_config_semantics(existing, config) => existing.config_revision,
            Some(existing) => existing
                .config_revision
                .checked_add(1)
                .context("Channel source configuration revision overflow during backup import")?,
        };
    }
    Ok(())
}

fn same_config_semantics(left: &ChannelSourceConfig, right: &ChannelSourceConfig) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.config_revision = 1;
    right.config_revision = 1;
    left == right
}

fn apply_mutation(
    config: &mut ChannelSourceConfig,
    mutation: ChannelSourceConfigMutation,
    source_ids: &mut BTreeSet<String>,
    attestation: Option<SequencerAttestationReceipt>,
) -> Result<()> {
    match mutation {
        ChannelSourceConfigMutation::AddSequencer {
            label,
            target,
            allow_insecure_http,
        } => {
            let source_id = fresh_source_id(source_ids)?;
            let target = target.normalized(ChannelSourceRole::Sequencer, allow_insecure_http)?;
            let channel_attestation =
                sequencer_attestation(&config.channel_id, &target, attestation)?;
            config.sequencer_sources.push(ConfiguredSequencerSource {
                source_id,
                label: normalize_label(label)?,
                target,
                channel_attestation,
            });
        }
        ChannelSourceConfigMutation::UpdateSequencer {
            source_id,
            label,
            target,
            allow_insecure_http,
        } => {
            let source_id = validate_source_id(&source_id)?;
            let target = target.normalized(ChannelSourceRole::Sequencer, allow_insecure_http)?;
            let owner_channel_id = config.channel_id.clone();
            let Some(source) = config
                .sequencer_sources
                .iter_mut()
                .find(|source| source.source_id == source_id)
            else {
                bail!("Sequencer source `{source_id}` does not exist");
            };
            if source.target != target || attestation.is_some() {
                source.channel_attestation =
                    sequencer_attestation(&owner_channel_id, &target, attestation)?;
            }
            source.label = normalize_label(label)?;
            source.target = target;
        }
        ChannelSourceConfigMutation::RemoveSequencer { source_id } => {
            reject_unused_attestation(attestation)?;
            let source_id = validate_source_id(&source_id)?;
            let previous_len = config.sequencer_sources.len();
            config
                .sequencer_sources
                .retain(|source| source.source_id != source_id);
            if config.sequencer_sources.len() == previous_len {
                bail!("Sequencer source `{source_id}` does not exist");
            }
            if config.selected_sequencer_source_id.as_deref() == Some(source_id.as_str()) {
                config.selected_sequencer_source_id = None;
            }
        }
        ChannelSourceConfigMutation::SelectSequencer { source_id } => {
            reject_unused_attestation(attestation)?;
            let source_id = source_id.as_deref().map(validate_source_id).transpose()?;
            if let Some(source_id) = source_id.as_ref()
                && !config
                    .sequencer_sources
                    .iter()
                    .any(|source| source.source_id == *source_id)
            {
                bail!("Sequencer source `{source_id}` does not exist");
            }
            config.selected_sequencer_source_id = source_id;
        }
        ChannelSourceConfigMutation::RetryAttestation { source_id } => {
            let source_id = validate_source_id(&source_id)?;
            let receipt = attestation.context("Sequencer attestation retry has no receipt")?;
            let owner_channel_id = config.channel_id.clone();
            let source = config
                .sequencer_sources
                .iter_mut()
                .find(|source| source.source_id == source_id)
                .with_context(|| format!("Sequencer source `{source_id}` does not exist"))?;
            source.channel_attestation =
                sequencer_attestation(&owner_channel_id, &source.target, Some(receipt))?;
        }
        ChannelSourceConfigMutation::SetIndexer {
            label,
            target,
            allow_insecure_http,
        } => {
            reject_unused_attestation(attestation)?;
            let label = normalize_label(label)?;
            let target = target.normalized(ChannelSourceRole::Indexer, allow_insecure_http)?;
            if let Some(indexer) = config.indexer_source.as_mut() {
                indexer.label = label;
                indexer.target = target;
            } else {
                config.indexer_source = Some(ConfiguredIndexerSource {
                    source_id: fresh_source_id(source_ids)?,
                    label,
                    target,
                });
            }
        }
        ChannelSourceConfigMutation::RemoveIndexer => {
            reject_unused_attestation(attestation)?;
            if config.indexer_source.take().is_none() {
                bail!("Indexer source does not exist");
            }
        }
    }
    Ok(())
}

fn sequencer_attestation(
    owner_channel_id: &str,
    target: &super::ChannelSourceTarget,
    receipt: Option<SequencerAttestationReceipt>,
) -> Result<PersistedSequencerAttestation> {
    let Some(receipt) = receipt else {
        return Ok(PersistedSequencerAttestation::Pending);
    };
    let reported_channel_id = normalize_channel_id(&receipt.reported_channel_id)?;
    if reported_channel_id != owner_channel_id {
        bail!("Sequencer attestation reported another Channel");
    }
    let target_fingerprint = target.fingerprint();
    if receipt.target_fingerprint != target_fingerprint {
        bail!("Sequencer attestation target fingerprint is stale");
    }
    Ok(PersistedSequencerAttestation::PersistedAttested {
        channel_id: reported_channel_id,
        target_fingerprint,
        attested_at_unix: receipt.attested_at_unix,
    })
}

fn reject_unused_attestation(receipt: Option<SequencerAttestationReceipt>) -> Result<()> {
    if receipt.is_some() {
        bail!("Sequencer attestation does not apply to this mutation");
    }
    Ok(())
}

fn configured_source_ids(configs: &[ChannelSourceConfig]) -> BTreeSet<String> {
    configs
        .iter()
        .flat_map(|config| {
            config
                .sequencer_sources
                .iter()
                .map(|source| source.source_id.clone())
                .chain(
                    config
                        .indexer_source
                        .iter()
                        .map(|source| source.source_id.clone()),
                )
        })
        .collect()
}

fn fresh_source_id(source_ids: &mut BTreeSet<String>) -> Result<String> {
    for _ in 0..SOURCE_ID_GENERATION_ATTEMPTS {
        let source_id = generate_source_id()?;
        if source_ids.insert(source_id.clone()) {
            return Ok(source_id);
        }
    }
    bail!("failed to generate a unique Channel source id")
}

fn settings_version(value: Option<&Value>) -> Result<u64> {
    let Some(value) = value else {
        return Ok(1);
    };
    let version = value
        .as_u64()
        .context("settings state version must be an unsigned integer")?;
    if version > SETTINGS_VERSION {
        bail!("settings state version {version} is not supported");
    }
    Ok(version)
}

fn settings_guard(settings_path: &Path) -> Result<LocalStateSession> {
    let parent = settings_path
        .parent()
        .context("settings state path has no parent directory")?;
    if settings_path.file_name().and_then(|value| value.to_str()) != Some("settings.json") {
        bail!("settings state path must use the authoritative filename");
    }
    lock_local_state_in(parent)
}

fn saved_report(path: &Path, durability: DirectoryDurability) -> Value {
    json!({
        "saved": true,
        "path": path.display().to_string(),
        "version": SETTINGS_VERSION,
        "directory_durability": durability.as_str(),
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Barrier, Mutex},
        thread,
    };

    use super::*;
    use crate::source_routing::channel_sources::{ChannelSourceRole, ChannelSourceTarget};

    use super::super::layer::module_id_for_role;
    use super::super::probe::{ChannelSourceProbeFailure, SequencerAttestorFuture};

    enum FakeAttestation {
        Reported(String),
        Failed(ChannelSourceFailureKind),
    }

    type AttestationHook = Box<dyn FnOnce() -> Result<()> + Send>;

    struct FakeAttestor {
        replies: Mutex<VecDeque<FakeAttestation>>,
        calls: Mutex<Vec<ChannelSourceTarget>>,
        hook: Mutex<Option<AttestationHook>>,
    }

    impl FakeAttestor {
        fn new(replies: impl IntoIterator<Item = FakeAttestation>) -> Self {
            Self {
                replies: Mutex::new(replies.into_iter().collect()),
                calls: Mutex::new(Vec::new()),
                hook: Mutex::new(None),
            }
        }

        fn with_hook(
            replies: impl IntoIterator<Item = FakeAttestation>,
            hook: AttestationHook,
        ) -> Self {
            Self {
                replies: Mutex::new(replies.into_iter().collect()),
                calls: Mutex::new(Vec::new()),
                hook: Mutex::new(Some(hook)),
            }
        }

        fn call_count(&self) -> Result<usize> {
            self.calls
                .lock()
                .map(|calls| calls.len())
                .map_err(|_| anyhow::anyhow!("fake attestor calls lock poisoned"))
        }
    }

    impl SequencerTargetAttestor for FakeAttestor {
        fn attest(self: Arc<Self>, target: ChannelSourceTarget) -> SequencerAttestorFuture {
            let result = (|| {
                self.calls
                    .lock()
                    .map_err(|_| fake_probe_failure(ChannelSourceFailureKind::Protocol))?
                    .push(target);
                if let Some(hook) = self
                    .hook
                    .lock()
                    .map_err(|_| fake_probe_failure(ChannelSourceFailureKind::Protocol))?
                    .take()
                {
                    hook().map_err(|_| fake_probe_failure(ChannelSourceFailureKind::Protocol))?;
                }
                match self
                    .replies
                    .lock()
                    .map_err(|_| fake_probe_failure(ChannelSourceFailureKind::Protocol))?
                    .pop_front()
                    .ok_or_else(|| fake_probe_failure(ChannelSourceFailureKind::Protocol))?
                {
                    FakeAttestation::Reported(channel_id) => Ok(channel_id),
                    FakeAttestation::Failed(kind) => Err(fake_probe_failure(kind)),
                }
            })();
            Box::pin(async move { result })
        }
    }

    fn fake_probe_failure(kind: ChannelSourceFailureKind) -> ChannelSourceProbeFailure {
        ChannelSourceProbeFailure {
            kind,
            diagnostic: format!("fake {kind:?} attestation failure"),
        }
    }

    #[test]
    fn fresh_settings_have_no_global_l2_configuration() -> Result<()> {
        let (directory, store) = test_store("fresh-settings")?;

        let settings = store.load()?;

        if settings.get("version").and_then(Value::as_u64) != Some(SETTINGS_VERSION)
            || settings
                .get(CHANNEL_SOURCE_CONFIGS_KEY)
                .and_then(Value::as_array)
                .is_none_or(|configs| !configs.is_empty())
        {
            bail!("fresh settings shape is invalid: {settings}");
        }
        for legacy_key in [
            "sequencer_url",
            "indexer_url",
            "execution_refresh_rate",
            "indexer_refresh_rate",
        ] {
            if settings.get(legacy_key).is_some() {
                bail!("fresh settings expose legacy key `{legacy_key}`: {settings}");
            }
        }
        cleanup_test_dir(&directory)
    }

    #[test]
    fn settings_v2_preserves_rust_owned_configs() -> Result<()> {
        let (directory, store) = test_store("settings-v2")?;
        store.save_user_settings(&json!({
            "version": 1,
            "theme": "dark",
            "channel_source_configs": [{ "caller_owned": true }]
        }))?;
        let initial = store.load()?;
        if initial.get("version").and_then(Value::as_u64) != Some(SETTINGS_VERSION) {
            bail!("settings version was not normalized: {initial}");
        }
        if initial
            .get(CHANNEL_SOURCE_CONFIGS_KEY)
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
            != 0
        {
            bail!("legacy caller injected Channel source configuration: {initial}");
        }

        let config = store.apply(add_sequencer_request('1', 0, 3040))?;
        let source_id = first_sequencer_source(&config)?.source_id.clone();
        store.save_user_settings(&json!({
            "version": 2,
            "theme": "light",
            "channel_source_configs": [{ "malformed": true }]
        }))?;
        let saved = store.load()?;
        let configs = settings_configs(&saved)?;
        if saved.get("theme").and_then(Value::as_str) != Some("light")
            || configs.len() != 1
            || first_sequencer_source(configs.first().context("saved config missing")?)?.source_id
                != source_id
        {
            bail!("generic settings save overwrote Rust-owned configuration: {saved}");
        }
        cleanup_test_dir(&directory)
    }

    #[test]
    fn channel_mutations_enforce_revision_and_clear_removed_selection() -> Result<()> {
        let (directory, store) = test_store("revisions")?;
        let added = store.apply(add_sequencer_request('2', 0, 3040))?;
        let source_id = first_sequencer_source(&added)?.source_id.clone();
        if added.config_revision != 1 || added.selected_sequencer_source_id.is_some() {
            bail!("unexpected initial Channel source configuration: {added:?}");
        }
        if store
            .apply(apply_request(
                '2',
                0,
                ChannelSourceConfigMutation::SelectSequencer {
                    source_id: Some(source_id.clone()),
                },
            ))
            .is_ok()
        {
            bail!("stale Channel source revision was accepted");
        }

        let selected = store.apply(apply_request(
            '2',
            1,
            ChannelSourceConfigMutation::SelectSequencer {
                source_id: Some(source_id.clone()),
            },
        ))?;
        if selected.config_revision != 2
            || selected.selected_sequencer_source_id.as_deref() != Some(source_id.as_str())
        {
            bail!("Sequencer source selection was not persisted: {selected:?}");
        }
        let removed = store.apply(apply_request(
            '2',
            2,
            ChannelSourceConfigMutation::RemoveSequencer {
                source_id: source_id.clone(),
            },
        ))?;
        if removed.config_revision != 3
            || !removed.sequencer_sources.is_empty()
            || removed.selected_sequencer_source_id.is_some()
        {
            bail!("selected source deletion did not clear selection: {removed:?}");
        }

        let with_indexer = store.apply(apply_request(
            '2',
            3,
            ChannelSourceConfigMutation::SetIndexer {
                label: Some(" Indexer ".to_owned()),
                target: module_target(module_id_for_role(ChannelSourceRole::Indexer)),
                allow_insecure_http: false,
            },
        ))?;
        let indexer_id = with_indexer
            .indexer_source
            .as_ref()
            .context("Indexer source missing")?
            .source_id
            .clone();
        let updated = store.apply(apply_request(
            '2',
            4,
            ChannelSourceConfigMutation::SetIndexer {
                label: Some("Renamed".to_owned()),
                target: module_target(module_id_for_role(ChannelSourceRole::Indexer)),
                allow_insecure_http: false,
            },
        ))?;
        if updated
            .indexer_source
            .as_ref()
            .context("updated Indexer missing")?
            .source_id
            != indexer_id
        {
            bail!("Indexer endpoint edit changed stable source id");
        }
        cleanup_test_dir(&directory)
    }

    #[test]
    fn attested_add_persists_receipt_in_single_revision_and_rejects_mismatch() -> Result<()> {
        let (directory, store) = test_store("attested-add")?;
        let target = rpc_target(3041);
        let request = apply_request(
            '4',
            0,
            ChannelSourceConfigMutation::AddSequencer {
                label: Some("Attested".to_owned()),
                target: target.clone(),
                allow_insecure_http: false,
            },
        );
        let config = store.apply_with_attestation(
            request,
            Some(SequencerAttestationReceipt {
                reported_channel_id: channel_id('4'),
                target_fingerprint: target.fingerprint(),
                attested_at_unix: 10,
            }),
        )?;
        let source = first_sequencer_source(&config)?;
        if config.config_revision != 1
            || !matches!(
                &source.channel_attestation,
                PersistedSequencerAttestation::PersistedAttested {
                    channel_id: attested_channel_id,
                    attested_at_unix: 10,
                    ..
                } if attested_channel_id == &channel_id('4')
            )
        {
            bail!("attested source was not persisted atomically: {config:?}");
        }

        let mismatched_target = rpc_target(3042);
        let mismatched = store.apply_with_attestation(
            apply_request(
                '5',
                0,
                ChannelSourceConfigMutation::AddSequencer {
                    label: None,
                    target: mismatched_target.clone(),
                    allow_insecure_http: false,
                },
            ),
            Some(SequencerAttestationReceipt {
                reported_channel_id: channel_id('6'),
                target_fingerprint: mismatched_target.fingerprint(),
                attested_at_unix: 11,
            }),
        );
        if mismatched.is_ok()
            || store
                .load_document_unlocked()?
                .channel_source_configs
                .iter()
                .any(|config| config.channel_id == channel_id('5'))
        {
            bail!("mismatched attestation changed source settings");
        }

        let pending = store.apply(add_sequencer_request('7', 0, 3043))?;
        let pending_source = first_sequencer_source(&pending)?;
        let pending_source_id = pending_source.source_id.clone();
        let pending_fingerprint = pending_source.target.fingerprint();
        let retried = store.apply_with_attestation(
            apply_request(
                '7',
                1,
                ChannelSourceConfigMutation::RetryAttestation {
                    source_id: pending_source_id,
                },
            ),
            Some(SequencerAttestationReceipt {
                reported_channel_id: channel_id('7'),
                target_fingerprint: pending_fingerprint,
                attested_at_unix: 12,
            }),
        )?;
        if retried.config_revision != 2
            || !matches!(
                &first_sequencer_source(&retried)?.channel_attestation,
                PersistedSequencerAttestation::PersistedAttested {
                    attested_at_unix: 12,
                    ..
                }
            )
        {
            bail!("explicit attestation retry did not persist receipt: {retried:?}");
        }
        cleanup_test_dir(&directory)
    }

    #[tokio::test]
    async fn mutation_interface_rejects_stale_and_retains_receipt_without_probe() -> Result<()> {
        let (directory, store) = test_store("interface-no-probe")?;
        let initial = attested_config('8', '1', 4, 3040);
        let source_id = first_sequencer_source(&initial)?.source_id.clone();
        store.write_document_unlocked(&SettingsDocument {
            channel_source_configs: vec![initial.clone()],
            ..SettingsDocument::default()
        })?;
        let attestor = Arc::new(FakeAttestor::new(std::iter::empty()));
        let interface = SettingsChannelSourceConfigMutation::with_store(&store, attestor.clone());

        let stale = interface
            .apply_request(apply_request(
                '8',
                3,
                ChannelSourceConfigMutation::UpdateSequencer {
                    source_id: source_id.clone(),
                    label: Some("ignored".to_owned()),
                    target: rpc_target(3040),
                    allow_insecure_http: false,
                },
            ))
            .await;
        if stale.is_ok() || attestor.call_count()? != 0 {
            bail!("stale mutation reached attestation or succeeded");
        }

        let outcome = interface
            .apply_request(apply_request(
                '8',
                4,
                ChannelSourceConfigMutation::UpdateSequencer {
                    source_id: source_id.clone(),
                    label: Some(" Renamed ".to_owned()),
                    target: rpc_target(3040),
                    allow_insecure_http: false,
                },
            ))
            .await?;
        let source = first_sequencer_source(&outcome.config)?;
        if outcome.attestation != ChannelSourceAttestationOutcome::NotRequired
            || outcome.config.config_revision != 5
            || source.source_id != source_id
            || source.label.as_deref() != Some("Renamed")
            || source.channel_attestation != first_sequencer_source(&initial)?.channel_attestation
            || attestor.call_count()? != 0
        {
            bail!("label-only mutation changed attestation contract: {outcome:?}");
        }
        cleanup_test_dir(&directory)
    }

    #[tokio::test]
    async fn target_change_attests_once_and_atomically_replaces_receipt() -> Result<()> {
        let (directory, store) = test_store("interface-target-change")?;
        let initial = attested_config('9', '2', 7, 3040);
        let source_id = first_sequencer_source(&initial)?.source_id.clone();
        store.write_document_unlocked(&SettingsDocument {
            channel_source_configs: vec![initial.clone()],
            ..SettingsDocument::default()
        })?;
        let attestor = Arc::new(FakeAttestor::new([FakeAttestation::Reported(channel_id(
            '9',
        ))]));
        let interface = SettingsChannelSourceConfigMutation::with_store(&store, attestor.clone());

        let outcome = interface
            .apply_request(apply_request(
                '9',
                7,
                ChannelSourceConfigMutation::UpdateSequencer {
                    source_id: source_id.clone(),
                    label: Some("Moved".to_owned()),
                    target: rpc_target(3041),
                    allow_insecure_http: false,
                },
            ))
            .await?;
        let source = first_sequencer_source(&outcome.config)?;
        if outcome.attestation != ChannelSourceAttestationOutcome::Persisted
            || outcome.config.config_revision != 8
            || source.source_id != source_id
            || source.target != rpc_target(3041)
            || !matches!(
                &source.channel_attestation,
                PersistedSequencerAttestation::PersistedAttested {
                    channel_id: reported,
                    target_fingerprint,
                    ..
                } if reported == &channel_id('9')
                    && target_fingerprint == &rpc_target(3041).fingerprint()
            )
            || attestor.call_count()? != 1
        {
            bail!("target mutation did not replace attestation atomically: {outcome:?}");
        }
        cleanup_test_dir(&directory)
    }

    #[tokio::test]
    async fn attestation_failure_policy_distinguishes_pending_fatal_and_mismatch() -> Result<()> {
        for (index, kind) in [
            ChannelSourceFailureKind::Timeout,
            ChannelSourceFailureKind::Unavailable,
        ]
        .into_iter()
        .enumerate()
        {
            let (directory, store) = test_store(&format!("pending-{index}"))?;
            let attestor = Arc::new(FakeAttestor::new([FakeAttestation::Failed(kind)]));
            let interface = SettingsChannelSourceConfigMutation::with_store(&store, attestor);
            let outcome = interface
                .apply_request(add_sequencer_request('a', 0, 3050 + index as u16))
                .await?;
            if outcome.attestation != ChannelSourceAttestationOutcome::Pending
                || !matches!(
                    first_sequencer_source(&outcome.config)?.channel_attestation,
                    PersistedSequencerAttestation::Pending
                )
            {
                bail!("recoverable attestation failure did not persist pending: {outcome:?}");
            }
            cleanup_test_dir(&directory)?;
        }

        for (index, kind) in [
            ChannelSourceFailureKind::Protocol,
            ChannelSourceFailureKind::Incomplete,
            ChannelSourceFailureKind::Unsupported,
        ]
        .into_iter()
        .enumerate()
        {
            let (directory, store) = test_store(&format!("fatal-{index}"))?;
            let attestor = Arc::new(FakeAttestor::new([FakeAttestation::Failed(kind)]));
            let interface = SettingsChannelSourceConfigMutation::with_store(&store, attestor);
            if interface
                .apply_request(add_sequencer_request('b', 0, 3060 + index as u16))
                .await
                .is_ok()
                || !store
                    .load_document_unlocked()?
                    .channel_source_configs
                    .is_empty()
            {
                bail!("fatal attestation failure changed settings");
            }
            cleanup_test_dir(&directory)?;
        }

        let (mismatch_directory, mismatch_store) = test_store("reported-mismatch")?;
        let mismatch_attestor = Arc::new(FakeAttestor::new([FakeAttestation::Reported(
            channel_id('d'),
        )]));
        let mismatch_interface =
            SettingsChannelSourceConfigMutation::with_store(&mismatch_store, mismatch_attestor);
        let mismatch = mismatch_interface
            .apply_request(add_sequencer_request('c', 0, 3070))
            .await;
        if mismatch.is_ok()
            || !mismatch_store
                .load_document_unlocked()?
                .channel_source_configs
                .is_empty()
        {
            bail!("reported Channel mismatch changed settings");
        }
        cleanup_test_dir(&mismatch_directory)
    }

    #[tokio::test]
    async fn retry_and_non_target_mutations_keep_attestor_policy_explicit() -> Result<()> {
        let (directory, store) = test_store("interface-attestor-policy")?;
        let mut initial = persisted_config('6', valid_source_id('b'));
        initial.config_revision = 3;
        let source_id = first_sequencer_source(&initial)?.source_id.clone();
        store.write_document_unlocked(&SettingsDocument {
            channel_source_configs: vec![initial.clone()],
            ..SettingsDocument::default()
        })?;

        let retry_attestor = Arc::new(FakeAttestor::new([FakeAttestation::Failed(
            ChannelSourceFailureKind::Unavailable,
        )]));
        let retry_interface =
            SettingsChannelSourceConfigMutation::with_store(&store, retry_attestor.clone());
        if retry_interface
            .apply_request(apply_request(
                '6',
                3,
                ChannelSourceConfigMutation::RetryAttestation {
                    source_id: source_id.clone(),
                },
            ))
            .await
            .is_ok()
            || retry_attestor.call_count()? != 1
            || store
                .load_document_unlocked()?
                .channel_source_configs
                .first()
                != Some(&initial)
        {
            bail!("explicit retry converted unavailable attestation into pending mutation");
        }

        let unused_attestor = Arc::new(FakeAttestor::new(std::iter::empty()));
        let interface =
            SettingsChannelSourceConfigMutation::with_store(&store, unused_attestor.clone());
        for invalid in [
            apply_request(
                '6',
                3,
                ChannelSourceConfigMutation::UpdateSequencer {
                    source_id: source_id.clone(),
                    label: None,
                    target: ChannelSourceTarget::Rpc {
                        endpoint: "https://sequencer.example/?secret=true".to_owned(),
                    },
                    allow_insecure_http: false,
                },
            ),
            apply_request(
                '6',
                3,
                ChannelSourceConfigMutation::UpdateSequencer {
                    source_id: valid_source_id('c'),
                    label: None,
                    target: rpc_target(3041),
                    allow_insecure_http: false,
                },
            ),
        ] {
            if interface.apply_request(invalid).await.is_ok() {
                bail!("invalid target/source mutation succeeded");
            }
        }
        if unused_attestor.call_count()? != 0 {
            bail!("invalid mutation reached attestor");
        }

        let selected = interface
            .apply_request(apply_request(
                '6',
                3,
                ChannelSourceConfigMutation::SelectSequencer {
                    source_id: Some(source_id.clone()),
                },
            ))
            .await?;
        let removed = interface
            .apply_request(apply_request(
                '6',
                4,
                ChannelSourceConfigMutation::RemoveSequencer { source_id },
            ))
            .await?;
        let indexed = interface
            .apply_request(apply_request(
                '6',
                5,
                ChannelSourceConfigMutation::SetIndexer {
                    label: Some("Indexer".to_owned()),
                    target: module_target(module_id_for_role(ChannelSourceRole::Indexer)),
                    allow_insecure_http: false,
                },
            ))
            .await?;
        let unindexed = interface
            .apply_request(apply_request(
                '6',
                6,
                ChannelSourceConfigMutation::RemoveIndexer,
            ))
            .await?;
        if selected.config.config_revision != 4
            || removed.config.config_revision != 5
            || indexed.config.config_revision != 6
            || unindexed.config.config_revision != 7
            || [selected, removed, indexed, unindexed]
                .iter()
                .any(|outcome| outcome.attestation != ChannelSourceAttestationOutcome::NotRequired)
            || unused_attestor.call_count()? != 0
        {
            bail!("non-target mutation invoked attestor or drifted revisions");
        }
        cleanup_test_dir(&directory)
    }

    #[tokio::test]
    async fn exact_target_cas_conflicts_but_preserves_unrelated_concurrent_state() -> Result<()> {
        let (conflict_directory, conflict_store) = test_store("exact-cas-conflict")?;
        let initial = attested_config('d', '3', 11, 3040);
        let source_id = first_sequencer_source(&initial)?.source_id.clone();
        conflict_store.write_document_unlocked(&SettingsDocument {
            channel_source_configs: vec![initial.clone()],
            ..SettingsDocument::default()
        })?;
        let hook_store = conflict_store.clone();
        let attestor = Arc::new(FakeAttestor::with_hook(
            [FakeAttestation::Reported(channel_id('d'))],
            Box::new(move || {
                let mut raced = hook_store.load_document_unlocked()?;
                first_sequencer_source_mut(
                    raced
                        .channel_source_configs
                        .first_mut()
                        .context("raced target config missing")?,
                )?
                .label = Some("Backup race".to_owned());
                hook_store.write_document_unlocked(&raced)
            }),
        ));
        let interface =
            SettingsChannelSourceConfigMutation::with_store(&conflict_store, attestor.clone());
        let result = interface
            .apply_request(apply_request(
                'd',
                11,
                ChannelSourceConfigMutation::UpdateSequencer {
                    source_id: source_id.clone(),
                    label: Some("Target edit".to_owned()),
                    target: rpc_target(3041),
                    allow_insecure_http: false,
                },
            ))
            .await;
        let retained = conflict_store.load_document_unlocked()?;
        let retained_config = retained
            .channel_source_configs
            .first()
            .context("conflicted config missing")?;
        if result.is_ok()
            || attestor.call_count()? != 1
            || retained_config.config_revision != 11
            || first_sequencer_source(retained_config)?.source_id != source_id
            || first_sequencer_source(retained_config)?.label.as_deref() != Some("Backup race")
            || first_sequencer_source(retained_config)?.target != rpc_target(3040)
            || first_sequencer_source(retained_config)?.channel_attestation
                != first_sequencer_source(&initial)?.channel_attestation
        {
            bail!("exact-target CAS did not reject same-revision content race");
        }
        cleanup_test_dir(&conflict_directory)?;

        let (merge_directory, merge_store) = test_store("exact-cas-unrelated")?;
        let target_config = attested_config('e', '4', 5, 3040);
        let target_source_id = first_sequencer_source(&target_config)?.source_id.clone();
        merge_store.write_document_unlocked(&SettingsDocument {
            channel_source_configs: vec![target_config],
            ..SettingsDocument::default()
        })?;
        let hook_store = merge_store.clone();
        let attestor = Arc::new(FakeAttestor::with_hook(
            [FakeAttestation::Reported(channel_id('e'))],
            Box::new(move || {
                let mut concurrent = hook_store.load_document_unlocked()?;
                concurrent
                    .fields
                    .insert("theme".to_owned(), Value::String("concurrent".to_owned()));
                concurrent
                    .channel_source_configs
                    .push(attested_config('f', '5', 9, 4040));
                hook_store.write_document_unlocked(&concurrent)
            }),
        ));
        let interface = SettingsChannelSourceConfigMutation::with_store(&merge_store, attestor);
        let outcome = interface
            .apply_request(apply_request(
                'e',
                5,
                ChannelSourceConfigMutation::UpdateSequencer {
                    source_id: target_source_id,
                    label: Some("Updated".to_owned()),
                    target: rpc_target(3041),
                    allow_insecure_http: false,
                },
            ))
            .await?;
        let merged = merge_store.load_document_unlocked()?;
        if outcome.config.config_revision != 6
            || merged.fields.get("theme") != Some(&Value::String("concurrent".to_owned()))
            || merged.channel_source_configs.len() != 2
            || !merged
                .channel_source_configs
                .iter()
                .any(|config| config.channel_id == channel_id('f') && config.config_revision == 9)
        {
            bail!("successful target CAS overwrote unrelated concurrent settings");
        }
        cleanup_test_dir(&merge_directory)
    }

    #[test]
    fn backup_import_rebases_revision_matrix_and_rejects_overflow() -> Result<()> {
        let identical = attested_config('1', '6', 7, 3040);
        let changed_current = attested_config('2', '7', 4, 3041);
        let removed = attested_config('3', '8', 12, 3042);
        let mut identical_import = identical.clone();
        identical_import.config_revision = 99;
        let mut changed_import = changed_current.clone();
        changed_import.config_revision = 88;
        first_sequencer_source_mut(&mut changed_import)?.label = Some("Imported label".to_owned());
        let mut new_import = attested_config('4', '9', 45, 3043);
        new_import.config_revision = 45;
        let current = settings_value(vec![identical.clone(), changed_current.clone(), removed])?;
        let incoming = settings_value(vec![
            identical_import,
            changed_import.clone(),
            new_import.clone(),
        ])?;

        let rebased = normalized_settings_state_from_backup(&incoming, &current)?;
        let configs = settings_configs(&rebased)?;
        let revision = |channel: char| {
            configs
                .iter()
                .find(|config| config.channel_id == channel_id(channel))
                .map(|config| config.config_revision)
        };
        let changed = configs
            .iter()
            .find(|config| config.channel_id == channel_id('2'))
            .context("changed imported config missing")?;
        let added = configs
            .iter()
            .find(|config| config.channel_id == channel_id('4'))
            .context("new imported config missing")?;
        if revision('1') != Some(7)
            || revision('2') != Some(5)
            || revision('3').is_some()
            || revision('4') != Some(1)
            || first_sequencer_source(changed)?.source_id
                != first_sequencer_source(&changed_import)?.source_id
            || first_sequencer_source(changed)?.target
                != first_sequencer_source(&changed_import)?.target
            || first_sequencer_source(changed)?.channel_attestation
                != first_sequencer_source(&changed_import)?.channel_attestation
            || first_sequencer_source(added)?.source_id
                != first_sequencer_source(&new_import)?.source_id
        {
            bail!("backup revision rebase matrix drifted: {configs:?}");
        }

        let mut maximum = attested_config('5', 'a', u64::MAX, 3044);
        let maximum_current = settings_value(vec![maximum.clone()])?;
        first_sequencer_source_mut(&mut maximum)?.label = Some("Changed".to_owned());
        maximum.config_revision = 1;
        let maximum_incoming = settings_value(vec![maximum])?;
        if normalized_settings_state_from_backup(&maximum_incoming, &maximum_current).is_ok() {
            bail!("changed backup import advanced a maximum configuration revision");
        }
        Ok(())
    }

    #[test]
    fn identity_promotion_rebinds_channel_sources_once() -> Result<()> {
        let (directory, store) = test_store("identity-rebind")?;
        let old_scope = NetworkScope::FinalizedAnchor {
            genesis_time: "1000".to_owned(),
            block_slot: 5,
            block_id: channel_id('5'),
            parent_id: channel_id('4'),
        };
        let new_scope = network_scope('b');
        let mut config = persisted_config('3', valid_source_id('c'));
        config.network_scope = old_scope.clone();
        let document = SettingsDocument {
            channel_source_configs: vec![config.clone()],
            ..SettingsDocument::default()
        };
        store.write_document_unlocked(&document)?;

        store.rebind_network_scope(old_scope.clone(), new_scope.clone())?;
        let rebound = store.load_document_unlocked()?.channel_source_configs;
        let rebound_config = rebound.first().context("rebound config missing")?;
        if rebound.len() != 1
            || rebound_config.network_scope != new_scope
            || rebound_config.channel_id != config.channel_id
            || rebound_config.config_revision != config.config_revision
            || first_sequencer_source(rebound_config)?.source_id
                != first_sequencer_source(&config)?.source_id
        {
            bail!("identity rebind changed Channel source configuration: {rebound:?}");
        }

        store.rebind_network_scope(old_scope, new_scope)?;
        if store.load_document_unlocked()?.channel_source_configs != rebound {
            bail!("repeated identity rebind changed settings");
        }
        cleanup_test_dir(&directory)
    }

    #[test]
    fn sequencer_target_edits_invalidate_attestation_but_label_edits_do_not() -> Result<()> {
        let (directory, store) = test_store("attestation")?;
        let added = store.apply(add_sequencer_request('3', 0, 3040))?;
        let source = first_sequencer_source(&added)?;
        let source_id = source.source_id.clone();
        let receipt = SequencerAttestationReceipt {
            reported_channel_id: channel_id('3'),
            target_fingerprint: source.target.fingerprint(),
            attested_at_unix: 100,
        };
        let attested = store.record_attestation(
            network_scope('a'),
            &channel_id('3'),
            1,
            &source_id,
            receipt,
        )?;
        if !matches!(
            first_sequencer_source(&attested)?.channel_attestation,
            PersistedSequencerAttestation::PersistedAttested { .. }
        ) {
            bail!("matching Sequencer attestation was not persisted");
        }

        let label_edit = store.apply(apply_request(
            '3',
            2,
            ChannelSourceConfigMutation::UpdateSequencer {
                source_id: source_id.clone(),
                label: Some("Renamed".to_owned()),
                target: rpc_target(3040),
                allow_insecure_http: false,
            },
        ))?;
        if !matches!(
            first_sequencer_source(&label_edit)?.channel_attestation,
            PersistedSequencerAttestation::PersistedAttested { .. }
        ) {
            bail!("label-only edit erased Sequencer attestation");
        }

        let target_edit = store.apply(apply_request(
            '3',
            3,
            ChannelSourceConfigMutation::UpdateSequencer {
                source_id: source_id.clone(),
                label: Some("Renamed".to_owned()),
                target: rpc_target(3041),
                allow_insecure_http: false,
            },
        ))?;
        if !matches!(
            first_sequencer_source(&target_edit)?.channel_attestation,
            PersistedSequencerAttestation::Pending
        ) || first_sequencer_source(&target_edit)?.source_id != source_id
        {
            bail!("target edit did not preserve id and invalidate attestation");
        }
        let mismatch = store.record_attestation(
            network_scope('a'),
            &channel_id('3'),
            4,
            &source_id,
            SequencerAttestationReceipt {
                reported_channel_id: channel_id('4'),
                target_fingerprint: first_sequencer_source(&target_edit)?.target.fingerprint(),
                attested_at_unix: 101,
            },
        );
        if mismatch.is_ok() {
            bail!("cross-Channel Sequencer attestation was persisted");
        }
        cleanup_test_dir(&directory)
    }

    #[test]
    fn backup_restore_rejects_invalid_ids_duplicates_and_selection() -> Result<()> {
        let (directory, store) = test_store("invalid-backup")?;
        let valid = persisted_config('4', valid_source_id('1'));

        let mut malformed = valid.clone();
        first_sequencer_source_mut(&mut malformed)?.source_id = "malformed".to_owned();
        require_restore_error(&store, vec![malformed], "malformed source id")?;

        let mut duplicate = valid.clone();
        duplicate.indexer_source = Some(ConfiguredIndexerSource {
            source_id: first_sequencer_source(&duplicate)?.source_id.clone(),
            label: None,
            target: module_target(module_id_for_role(ChannelSourceRole::Indexer)),
        });
        require_restore_error(&store, vec![duplicate], "duplicate source id")?;

        let mut missing_selection = valid.clone();
        missing_selection.selected_sequencer_source_id = Some(valid_source_id('9'));
        require_restore_error(&store, vec![missing_selection], "invalid selection")?;

        let duplicate_key = valid.clone();
        require_restore_error(&store, vec![valid, duplicate_key], "duplicate Channel key")?;
        cleanup_test_dir(&directory)
    }

    #[test]
    fn interrupted_atomic_replacement_keeps_previous_settings() -> Result<()> {
        let (directory, store) = test_store("atomic")?;
        store.save_user_settings(&json!({ "version": 2, "theme": "old" }))?;
        let replacement = SettingsDocument::from_value(json!({
            "version": 2,
            "theme": "new",
            "channel_source_configs": []
        }))?;
        let result = store.write_document_with_hook(&replacement, |_| {
            bail!("injected interruption before replacement")
        });
        if result.is_ok() {
            bail!("injected atomic replacement interruption succeeded");
        }
        let loaded = store.load()?;
        if loaded.get("theme").and_then(Value::as_str) != Some("old") {
            bail!("failed replacement changed committed settings: {loaded}");
        }
        let entries = fs::read_dir(&directory)
            .with_context(|| format!("failed to read test directory {}", directory.display()))?
            .collect::<std::io::Result<Vec<_>>>()?;
        let entry_names = entries
            .iter()
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .collect::<Vec<_>>();
        if !entry_names.iter().any(|name| name == "settings.json")
            || entry_names.iter().any(|name| name.ends_with(".tmp"))
        {
            bail!("atomic replacement left temporary files: {entries:?}");
        }
        cleanup_test_dir(&directory)
    }

    #[test]
    fn concurrent_generic_save_cannot_overwrite_channel_mutation() -> Result<()> {
        let (directory, store) = test_store("concurrent")?;
        let added = store.apply(add_sequencer_request('5', 0, 3040))?;
        let source_id = first_sequencer_source(&added)?.source_id.clone();
        let barrier = Arc::new(Barrier::new(3));
        let save_store = store.clone();
        let save_barrier = Arc::clone(&barrier);
        let save_thread = thread::spawn(move || -> Result<()> {
            save_barrier.wait();
            save_store.save_user_settings(&json!({
                "version": 1,
                "theme": "concurrent-save",
                "channel_source_configs": []
            }))?;
            Ok(())
        });
        let mutation_store = store.clone();
        let mutation_barrier = Arc::clone(&barrier);
        let mutation_source_id = source_id.clone();
        let mutation_thread = thread::spawn(move || -> Result<()> {
            mutation_barrier.wait();
            mutation_store.apply(apply_request(
                '5',
                1,
                ChannelSourceConfigMutation::SelectSequencer {
                    source_id: Some(mutation_source_id),
                },
            ))?;
            Ok(())
        });
        barrier.wait();
        save_thread
            .join()
            .map_err(|_| anyhow::anyhow!("generic settings save thread panicked"))??;
        mutation_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Channel source mutation thread panicked"))??;

        let loaded = store.load()?;
        let loaded_configs = settings_configs(&loaded)?;
        let config = loaded_configs
            .first()
            .context("Channel source config missing after concurrent writes")?;
        if loaded.get("theme").and_then(Value::as_str) != Some("concurrent-save")
            || config.selected_sequencer_source_id.as_deref() != Some(source_id.as_str())
            || config.config_revision != 2
        {
            bail!("concurrent settings write lost state: {loaded}");
        }
        cleanup_test_dir(&directory)
    }

    #[test]
    fn backup_restore_round_trips_config_and_rejects_authenticated_target() -> Result<()> {
        let (source_directory, source_store) = test_store("backup-source")?;
        source_store.save_user_settings(&json!({ "version": 2, "theme": "backup" }))?;
        source_store.apply(apply_request(
            '6',
            0,
            ChannelSourceConfigMutation::AddSequencer {
                label: Some("Remote".to_owned()),
                target: ChannelSourceTarget::Rpc {
                    endpoint: "https://rpc.example.test/lez/v1".to_owned(),
                },
                allow_insecure_http: false,
            },
        ))?;
        let backup = source_store.load()?;
        let backup_text = serde_json::to_string(&backup)?;
        if backup_text.contains("userinfo") || backup_text.contains("token=") {
            bail!("backup contained credential material: {backup_text}");
        }

        let (restore_directory, restore_store) = test_store("backup-restore")?;
        restore_store.replace_from_backup(&backup)?;
        let restored = restore_store.load()?;
        if settings_configs(&restored)? != settings_configs(&backup)? {
            bail!("backup restore changed Channel source configuration");
        }

        let mut authenticated = backup;
        let endpoint = authenticated
            .pointer_mut("/channel_source_configs/0/sequencer_sources/0/target/endpoint")
            .context("backup endpoint missing")?;
        *endpoint = Value::String("https://user:secret@rpc.example.test/lez".to_owned());
        if restore_store.replace_from_backup(&authenticated).is_ok() {
            bail!("authenticated source target was restored from backup");
        }
        let retained = restore_store.load()?;
        if settings_configs(&retained)? != settings_configs(&restored)? {
            bail!("failed backup validation changed committed configuration");
        }
        cleanup_test_dir(&source_directory)?;
        cleanup_test_dir(&restore_directory)
    }

    #[test]
    fn invalid_channel_and_target_mutations_do_not_create_records() -> Result<()> {
        let (directory, store) = test_store("invalid-mutations")?;
        let mut bad_channel = add_sequencer_request('7', 0, 3040);
        bad_channel.channel_id = "not-a-channel".to_owned();
        if store.apply(bad_channel).is_ok() {
            bail!("invalid Channel id was accepted");
        }
        let bad_target = apply_request(
            '7',
            0,
            ChannelSourceConfigMutation::AddSequencer {
                label: None,
                target: ChannelSourceTarget::Rpc {
                    endpoint: "https://rpc.example.test/lez?token=secret".to_owned(),
                },
                allow_insecure_http: false,
            },
        );
        if store.apply(bad_target).is_ok() {
            bail!("credential-bearing target was accepted");
        }
        if !store
            .load_document_unlocked()?
            .channel_source_configs
            .is_empty()
        {
            bail!("failed mutation created a Channel source record");
        }
        cleanup_test_dir(&directory)
    }

    fn test_store(label: &str) -> Result<(PathBuf, SettingsStore)> {
        let mut random = [0_u8; 8];
        getrandom::fill(&mut random).context("failed to generate test directory id")?;
        let directory = std::env::temp_dir().join(format!(
            "logos-inspector-channel-sources-{label}-{}",
            hex::encode(random)
        ));
        fs::create_dir_all(&directory)
            .with_context(|| format!("failed to create test directory {}", directory.display()))?;
        Ok((
            directory.clone(),
            SettingsStore::new(directory.join("settings.json")),
        ))
    }

    fn cleanup_test_dir(directory: &Path) -> Result<()> {
        fs::remove_dir_all(directory)
            .with_context(|| format!("failed to remove test directory {}", directory.display()))
    }

    fn add_sequencer_request(
        channel_character: char,
        expected_config_revision: u64,
        port: u16,
    ) -> ChannelSourceConfigApplyRequest {
        apply_request(
            channel_character,
            expected_config_revision,
            ChannelSourceConfigMutation::AddSequencer {
                label: Some("Primary".to_owned()),
                target: rpc_target(port),
                allow_insecure_http: false,
            },
        )
    }

    fn apply_request(
        channel_character: char,
        expected_config_revision: u64,
        mutation: ChannelSourceConfigMutation,
    ) -> ChannelSourceConfigApplyRequest {
        ChannelSourceConfigApplyRequest {
            network_scope: network_scope('a'),
            channel_id: channel_id(channel_character),
            expected_config_revision,
            mutation,
        }
    }

    fn network_scope(character: char) -> NetworkScope {
        NetworkScope::GenesisId {
            genesis_id: character.to_string().repeat(64),
        }
    }

    fn channel_id(character: char) -> String {
        character.to_string().repeat(64)
    }

    fn rpc_target(port: u16) -> ChannelSourceTarget {
        ChannelSourceTarget::Rpc {
            endpoint: format!("http://127.0.0.1:{port}/"),
        }
    }

    fn module_target(module_id: &str) -> ChannelSourceTarget {
        ChannelSourceTarget::Module {
            module_id: module_id.to_owned(),
        }
    }

    fn first_sequencer_source(config: &ChannelSourceConfig) -> Result<&ConfiguredSequencerSource> {
        config
            .sequencer_sources
            .first()
            .context("Sequencer source missing")
    }

    fn first_sequencer_source_mut(
        config: &mut ChannelSourceConfig,
    ) -> Result<&mut ConfiguredSequencerSource> {
        config
            .sequencer_sources
            .first_mut()
            .context("Sequencer source missing")
    }

    fn settings_configs(settings: &Value) -> Result<Vec<ChannelSourceConfig>> {
        serde_json::from_value(
            settings
                .get(CHANNEL_SOURCE_CONFIGS_KEY)
                .cloned()
                .context("Channel source settings missing")?,
        )
        .context("failed to decode Channel source settings")
    }

    fn persisted_config(channel_character: char, source_id: String) -> ChannelSourceConfig {
        ChannelSourceConfig {
            network_scope: network_scope('a'),
            channel_id: channel_id(channel_character),
            config_revision: 1,
            sequencer_sources: vec![ConfiguredSequencerSource {
                source_id,
                label: Some("Primary".to_owned()),
                target: rpc_target(3040),
                channel_attestation: PersistedSequencerAttestation::Pending,
            }],
            selected_sequencer_source_id: None,
            indexer_source: None,
        }
    }

    fn attested_config(
        channel_character: char,
        source_character: char,
        revision: u64,
        port: u16,
    ) -> ChannelSourceConfig {
        let mut config = persisted_config(channel_character, valid_source_id(source_character));
        config.config_revision = revision;
        let target = rpc_target(port);
        if let Some(source) = config.sequencer_sources.first_mut() {
            source.target = target.clone();
            source.channel_attestation = PersistedSequencerAttestation::PersistedAttested {
                channel_id: config.channel_id.clone(),
                target_fingerprint: target.fingerprint(),
                attested_at_unix: 1,
            };
        }
        config
    }

    fn settings_value(configs: Vec<ChannelSourceConfig>) -> Result<Value> {
        SettingsDocument {
            channel_source_configs: configs,
            ..SettingsDocument::default()
        }
        .into_value()
    }

    fn valid_source_id(character: char) -> String {
        format!("src_{}", character.to_string().repeat(32))
    }

    fn require_restore_error(
        store: &SettingsStore,
        configs: Vec<ChannelSourceConfig>,
        context: &str,
    ) -> Result<()> {
        let state = json!({
            "version": SETTINGS_VERSION,
            "channel_source_configs": configs,
        });
        if store.replace_from_backup(&state).is_ok() {
            bail!("backup restore accepted {context}");
        }
        Ok(())
    }
}
