use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value, json};

use crate::{inspection::NetworkScope, support::config_path::settings_state_path};

use super::config::{
    ChannelSourceConfig, ChannelSourceConfigApplyRequest, ChannelSourceConfigMutation,
    ChannelSourceRole, ConfiguredIndexerSource, ConfiguredSequencerSource,
    PersistedSequencerAttestation, SequencerAttestationReceipt, generate_source_id,
    normalize_channel_id, normalize_channel_source_configs, normalize_label,
    normalize_network_scope, validate_source_id,
};

const SETTINGS_VERSION: u64 = 2;
const CHANNEL_SOURCE_CONFIGS_KEY: &str = "channel_source_configs";
const SOURCE_ID_GENERATION_ATTEMPTS: usize = 8;

static SETTINGS_WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn load_channel_source_configs() -> Result<Vec<ChannelSourceConfig>> {
    let store = SettingsStore::new(settings_state_path()?);
    let _guard = settings_guard(&store.path)?;
    Ok(store.load_document_unlocked()?.channel_source_configs)
}

pub fn apply_channel_source_config(
    request: ChannelSourceConfigApplyRequest,
) -> Result<ChannelSourceConfig> {
    SettingsStore::new(settings_state_path()?).apply(request)
}

pub(crate) fn apply_channel_source_config_with_attestation(
    request: ChannelSourceConfigApplyRequest,
    attestation: Option<SequencerAttestationReceipt>,
) -> Result<ChannelSourceConfig> {
    SettingsStore::new(settings_state_path()?).apply_with_attestation(request, attestation)
}

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

pub(crate) fn restore_settings_state_from_backup(state: &Value) -> Result<Value> {
    SettingsStore::new(settings_state_path()?).replace_from_backup(state)
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
        let _guard = settings_guard(&self.path)?;
        self.load_document_unlocked()?.into_value()
    }

    fn save_user_settings(&self, state: &Value) -> Result<Value> {
        let _guard = settings_guard(&self.path)?;
        let current = self.load_document_unlocked()?;
        let incoming = SettingsDocument::from_user_value(state.clone())?;
        let document = SettingsDocument {
            fields: incoming.fields,
            channel_source_configs: current.channel_source_configs,
        };
        self.write_document_unlocked(&document)?;
        Ok(saved_report(&self.path))
    }

    fn replace_from_backup(&self, state: &Value) -> Result<Value> {
        let _guard = settings_guard(&self.path)?;
        let document = SettingsDocument::from_value(state.clone())?;
        self.write_document_unlocked(&document)?;
        Ok(saved_report(&self.path))
    }

    fn apply(&self, request: ChannelSourceConfigApplyRequest) -> Result<ChannelSourceConfig> {
        self.apply_with_attestation(request, None)
    }

    fn apply_with_attestation(
        &self,
        request: ChannelSourceConfigApplyRequest,
        attestation: Option<SequencerAttestationReceipt>,
    ) -> Result<ChannelSourceConfig> {
        let _guard = settings_guard(&self.path)?;
        let mut document = self.load_document_unlocked()?;
        let network_scope = normalize_network_scope(request.network_scope)?;
        let channel_id = normalize_channel_id(&request.channel_id)?;
        let existing = document
            .channel_source_configs
            .iter()
            .find(|config| config.network_scope == network_scope && config.channel_id == channel_id)
            .cloned();
        let current_revision = existing.as_ref().map_or(0, |config| config.config_revision);
        if current_revision != request.expected_config_revision {
            bail!(
                "Channel source configuration revision conflict: expected {}, current {current_revision}",
                request.expected_config_revision
            );
        }

        let mut config = existing.unwrap_or_else(|| ChannelSourceConfig {
            network_scope: network_scope.clone(),
            channel_id: channel_id.clone(),
            config_revision: 0,
            sequencer_sources: Vec::new(),
            selected_sequencer_source_id: None,
            indexer_source: None,
        });
        let mut source_ids = configured_source_ids(&document.channel_source_configs);
        apply_mutation(&mut config, request.mutation, &mut source_ids, attestation)?;
        config.config_revision = current_revision
            .checked_add(1)
            .context("Channel source configuration revision overflow")?;
        config = config.normalized()?;

        if let Some(current) = document.channel_source_configs.iter_mut().find(|current| {
            current.network_scope == network_scope && current.channel_id == channel_id
        }) {
            *current = config.clone();
        } else {
            document.channel_source_configs.push(config.clone());
        }
        self.write_document_unlocked(&document)?;
        Ok(config)
    }

    fn record_attestation(
        &self,
        network_scope: NetworkScope,
        channel_id: &str,
        expected_config_revision: u64,
        source_id: &str,
        receipt: SequencerAttestationReceipt,
    ) -> Result<ChannelSourceConfig> {
        let _guard = settings_guard(&self.path)?;
        let mut document = self.load_document_unlocked()?;
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
        self.write_document_unlocked(&document)?;
        Ok(updated)
    }

    fn rebind_network_scope(&self, old_scope: NetworkScope, new_scope: NetworkScope) -> Result<()> {
        let _guard = settings_guard(&self.path)?;
        let old_scope = normalize_network_scope(old_scope)?;
        let new_scope = normalize_network_scope(new_scope)?;
        if old_scope == new_scope {
            return Ok(());
        }
        let mut document = self.load_document_unlocked()?;
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
        self.write_document_unlocked(&document)
    }

    fn load_document_unlocked(&self) -> Result<SettingsDocument> {
        if !self.path.is_file() {
            return Ok(SettingsDocument::default());
        }
        let text = fs::read_to_string(&self.path).with_context(|| {
            format!("failed to read settings state from {}", self.path.display())
        })?;
        let value = serde_json::from_str(&text).with_context(|| {
            format!(
                "failed to parse settings state from {}",
                self.path.display()
            )
        })?;
        SettingsDocument::from_value(value)
    }

    fn write_document_unlocked(&self, document: &SettingsDocument) -> Result<()> {
        self.write_document_with_hook(document, |_| Ok(()))
    }

    fn write_document_with_hook<F>(
        &self,
        document: &SettingsDocument,
        before_replace: F,
    ) -> Result<()>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
        let value = document.clone().into_value()?;
        atomic_write_json(&self.path, &value, before_replace)
    }
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

struct SettingsGuard {
    _process_guard: MutexGuard<'static, ()>,
    _file_guard: File,
}

fn settings_guard(settings_path: &Path) -> Result<SettingsGuard> {
    let process_guard = SETTINGS_WRITE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("settings state lock is poisoned"))?;
    let parent = settings_path
        .parent()
        .context("settings state path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    let lock_path = parent.join(".settings.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open settings lock {}", lock_path.display()))?;
    lock_file
        .lock()
        .with_context(|| format!("failed to lock settings state at {}", lock_path.display()))?;
    Ok(SettingsGuard {
        _process_guard: process_guard,
        _file_guard: lock_file,
    })
}

fn saved_report(path: &Path) -> Value {
    json!({
        "saved": true,
        "path": path.display().to_string(),
        "version": SETTINGS_VERSION,
    })
}

fn atomic_write_json<F>(path: &Path, value: &Value, before_replace: F) -> Result<()>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let parent = path
        .parent()
        .context("settings state path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("settings state filename is invalid")?;
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!(".{file_name}."))
        .suffix(".tmp")
        .tempfile_in(parent)
        .with_context(|| {
            format!(
                "failed to create settings temporary file in {}",
                parent.display()
            )
        })?;
    let text = serde_json::to_vec_pretty(value).context("failed to serialize settings state")?;
    temporary.write_all(&text).with_context(|| {
        format!(
            "failed to write settings temporary file {}",
            temporary.path().display()
        )
    })?;
    temporary.as_file().sync_all().with_context(|| {
        format!(
            "failed to sync settings temporary file {}",
            temporary.path().display()
        )
    })?;
    before_replace(temporary.path())?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace settings state at {}", path.display()))?;
    sync_parent_directory(parent)?;
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> Result<()> {
    File::open(parent)
        .with_context(|| format!("failed to open config directory {}", parent.display()))?
        .sync_all()
        .with_context(|| format!("failed to sync config directory {}", parent.display()))
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Barrier},
        thread,
    };

    use super::*;
    use crate::source_routing::channel_sources::{ChannelSourceRole, ChannelSourceTarget};

    use super::super::layer::module_id_for_role;

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
        let result = {
            let _guard = settings_guard(&store.path)?;
            store.write_document_with_hook(&replacement, |_| {
                bail!("injected interruption before replacement")
            })
        };
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
