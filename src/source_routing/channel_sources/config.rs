use std::collections::BTreeSet;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use url::{Host, Url};

use crate::{
    inspection::NetworkScope,
    source_routing::{INDEXER_MODULE, LEZ_CORE_MODULE},
};

const SOURCE_ID_PREFIX: &str = "src_";
const SOURCE_ID_RANDOM_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSourceConfig {
    pub network_scope: NetworkScope,
    pub channel_id: String,
    pub config_revision: u64,
    pub sequencer_sources: Vec<ConfiguredSequencerSource>,
    pub selected_sequencer_source_id: Option<String>,
    pub indexer_source: Option<ConfiguredIndexerSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredSequencerSource {
    pub source_id: String,
    pub label: Option<String>,
    pub target: ChannelSourceTarget,
    pub channel_attestation: PersistedSequencerAttestation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredIndexerSource {
    pub source_id: String,
    pub label: Option<String>,
    pub target: ChannelSourceTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChannelSourceTarget {
    Rpc { endpoint: String },
    Module { module_id: String },
}

impl ChannelSourceTarget {
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let mut digest = Sha256::new();
        match self {
            Self::Rpc { endpoint } => {
                digest.update(b"rpc\0");
                digest.update(endpoint.as_bytes());
            }
            Self::Module { module_id } => {
                digest.update(b"module\0");
                digest.update(module_id.as_bytes());
            }
        }
        format!("sha256:{}", hex::encode(digest.finalize()))
    }

    #[must_use]
    pub fn is_insecure_http(&self) -> bool {
        matches!(self, Self::Rpc { endpoint } if endpoint.starts_with("http://"))
    }

    pub(crate) fn normalized(
        self,
        role: ChannelSourceRole,
        allow_insecure_http: bool,
    ) -> Result<Self> {
        match self {
            Self::Rpc { endpoint } => Ok(Self::Rpc {
                endpoint: normalized_rpc_endpoint(&endpoint, allow_insecure_http)?,
            }),
            Self::Module { module_id } => Ok(Self::Module {
                module_id: normalized_module_id(&module_id, role)?,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PersistedSequencerAttestation {
    Pending,
    PersistedAttested {
        channel_id: String,
        target_fingerprint: String,
        attested_at_unix: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencerAttestationReceipt {
    pub reported_channel_id: String,
    pub target_fingerprint: String,
    pub attested_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChannelSourceConfigApplyRequest {
    pub network_scope: NetworkScope,
    pub channel_id: String,
    pub expected_config_revision: u64,
    pub mutation: ChannelSourceConfigMutation,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChannelSourceConfigMutation {
    AddSequencer {
        label: Option<String>,
        target: ChannelSourceTarget,
        #[serde(default)]
        allow_insecure_http: bool,
    },
    UpdateSequencer {
        source_id: String,
        label: Option<String>,
        target: ChannelSourceTarget,
        #[serde(default)]
        allow_insecure_http: bool,
    },
    RemoveSequencer {
        source_id: String,
    },
    SelectSequencer {
        source_id: Option<String>,
    },
    SetIndexer {
        label: Option<String>,
        target: ChannelSourceTarget,
        #[serde(default)]
        allow_insecure_http: bool,
    },
    RemoveIndexer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSourceRole {
    Sequencer,
    Indexer,
}

impl ChannelSourceConfig {
    pub(crate) fn normalized(mut self) -> Result<Self> {
        self.network_scope = normalize_network_scope(self.network_scope)?;
        self.channel_id = normalize_channel_id(&self.channel_id)?;
        if self.config_revision == 0 {
            bail!("Channel source configuration revision must be positive");
        }

        let mut source_ids = BTreeSet::new();
        for source in &mut self.sequencer_sources {
            source.source_id = validate_source_id(&source.source_id)?;
            if !source_ids.insert(source.source_id.clone()) {
                bail!("duplicate Channel source id `{}`", source.source_id);
            }
            source.label = normalize_label(source.label.take())?;
            source.target = source
                .target
                .clone()
                .normalized(ChannelSourceRole::Sequencer, true)?;
            source.channel_attestation = normalized_attestation(
                source.channel_attestation.clone(),
                &self.channel_id,
                &source.target,
            )?;
        }

        if let Some(indexer) = self.indexer_source.as_mut() {
            indexer.source_id = validate_source_id(&indexer.source_id)?;
            if !source_ids.insert(indexer.source_id.clone()) {
                bail!("duplicate Channel source id `{}`", indexer.source_id);
            }
            indexer.label = normalize_label(indexer.label.take())?;
            indexer.target = indexer
                .target
                .clone()
                .normalized(ChannelSourceRole::Indexer, true)?;
        }

        if let Some(selected) = self.selected_sequencer_source_id.as_mut() {
            *selected = validate_source_id(selected)?;
            if !self
                .sequencer_sources
                .iter()
                .any(|source| source.source_id == *selected)
            {
                bail!("selected Sequencer source `{selected}` is not configured");
            }
        }
        Ok(self)
    }
}

pub(crate) fn normalize_channel_source_configs(
    configs: Vec<ChannelSourceConfig>,
) -> Result<Vec<ChannelSourceConfig>> {
    let mut normalized = Vec::with_capacity(configs.len());
    let mut keys = Vec::with_capacity(configs.len());
    let mut source_ids = BTreeSet::new();
    for config in configs {
        let config = config.normalized()?;
        if keys.iter().any(|(network_scope, channel_id)| {
            network_scope == &config.network_scope && channel_id == &config.channel_id
        }) {
            bail!(
                "duplicate Channel source configuration for `{}`",
                config.channel_id
            );
        }
        keys.push((config.network_scope.clone(), config.channel_id.clone()));
        for source_id in config
            .sequencer_sources
            .iter()
            .map(|source| &source.source_id)
            .chain(config.indexer_source.iter().map(|source| &source.source_id))
        {
            if !source_ids.insert(source_id.clone()) {
                bail!("duplicate Channel source id `{source_id}`");
            }
        }
        normalized.push(config);
    }
    Ok(normalized)
}

pub(crate) fn normalize_channel_id(value: &str) -> Result<String> {
    normalize_hex_identity(value, "Channel id")
}

pub(crate) fn normalize_network_scope(scope: NetworkScope) -> Result<NetworkScope> {
    match scope {
        NetworkScope::GenesisId { genesis_id } => Ok(NetworkScope::GenesisId {
            genesis_id: normalize_hex_identity(&genesis_id, "genesis id")?,
        }),
        NetworkScope::FinalizedAnchor {
            genesis_time,
            block_slot,
            block_id,
            parent_id,
        } => {
            let genesis_time = genesis_time.trim();
            if genesis_time.is_empty() || genesis_time.chars().any(char::is_control) {
                bail!("finalized-anchor genesis time is invalid");
            }
            Ok(NetworkScope::FinalizedAnchor {
                genesis_time: genesis_time.to_owned(),
                block_slot,
                block_id: normalize_hex_identity(&block_id, "finalized-anchor block id")?,
                parent_id: normalize_hex_identity(&parent_id, "finalized-anchor parent id")?,
            })
        }
    }
}

pub(crate) fn normalize_label(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().any(char::is_control) {
        bail!("Channel source label cannot contain control characters");
    }
    Ok(Some(value.to_owned()))
}

pub(crate) fn validate_source_id(value: &str) -> Result<String> {
    let Some(random) = value.strip_prefix(SOURCE_ID_PREFIX) else {
        bail!("Channel source id `{value}` is invalid");
    };
    if random.len() != SOURCE_ID_RANDOM_BYTES * 2
        || !random
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        bail!("Channel source id `{value}` is invalid");
    }
    Ok(value.to_owned())
}

pub(crate) fn generate_source_id() -> Result<String> {
    let mut random = [0_u8; SOURCE_ID_RANDOM_BYTES];
    getrandom::fill(&mut random).context("failed to generate Channel source id")?;
    Ok(format!("{SOURCE_ID_PREFIX}{}", hex::encode(random)))
}

fn normalize_hex_identity(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.len() != 64 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        bail!("{label} must be 32-byte hexadecimal text");
    }
    Ok(value.to_ascii_lowercase())
}

fn normalized_attestation(
    attestation: PersistedSequencerAttestation,
    owner_channel_id: &str,
    target: &ChannelSourceTarget,
) -> Result<PersistedSequencerAttestation> {
    match attestation {
        PersistedSequencerAttestation::Pending => Ok(PersistedSequencerAttestation::Pending),
        PersistedSequencerAttestation::PersistedAttested {
            channel_id,
            target_fingerprint,
            attested_at_unix,
        } => {
            let channel_id = normalize_channel_id(&channel_id)?;
            if channel_id != owner_channel_id {
                bail!("Sequencer attestation Channel does not match its owner");
            }
            if target_fingerprint != target.fingerprint() {
                bail!("Sequencer attestation target fingerprint is stale");
            }
            Ok(PersistedSequencerAttestation::PersistedAttested {
                channel_id,
                target_fingerprint,
                attested_at_unix,
            })
        }
    }
}

fn normalized_rpc_endpoint(value: &str, allow_insecure_http: bool) -> Result<String> {
    if value.chars().any(char::is_control) {
        bail!("RPC endpoint cannot contain control characters");
    }
    let value = value.trim();
    if value.is_empty() {
        bail!("RPC endpoint is required");
    }
    let (_, authority_and_path) = value
        .split_once("://")
        .context("RPC endpoint must include a URL authority")?;
    if authority_and_path.starts_with('/') {
        bail!("RPC endpoint must include a host");
    }
    let mut endpoint = Url::parse(value).context("RPC endpoint is not a valid URL")?;
    if endpoint.cannot_be_a_base() {
        bail!("RPC endpoint must be a hierarchical URL");
    }
    if endpoint.scheme() != "http" && endpoint.scheme() != "https" {
        bail!("RPC endpoint scheme must be http or https");
    }
    if endpoint.host().is_none() {
        bail!("RPC endpoint must include a host");
    }
    if !endpoint.username().is_empty() || endpoint.password().is_some() {
        bail!("RPC endpoint cannot contain authentication");
    }
    if endpoint.query().is_some() || endpoint.fragment().is_some() {
        bail!("RPC endpoint cannot contain a query or fragment");
    }

    let default_port = match endpoint.scheme() {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    };
    if endpoint.port() == default_port {
        endpoint
            .set_port(None)
            .map_err(|()| anyhow::anyhow!("RPC endpoint port is invalid"))?;
    }
    if endpoint.scheme() == "http" && !endpoint_is_loopback(&endpoint) && !allow_insecure_http {
        bail!("non-loopback HTTP RPC endpoint requires allow_insecure_http");
    }
    Ok(endpoint.to_string())
}

fn endpoint_is_loopback(endpoint: &Url) -> bool {
    match endpoint.host() {
        Some(Host::Domain(domain)) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".localhost")
        }
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn normalized_module_id(value: &str, role: ChannelSourceRole) -> Result<String> {
    if value.chars().any(char::is_control) {
        bail!("module id cannot contain control characters");
    }
    let value = value.trim();
    let expected = match role {
        ChannelSourceRole::Sequencer => LEZ_CORE_MODULE,
        ChannelSourceRole::Indexer => INDEXER_MODULE,
    };
    if value != expected {
        bail!("module id `{value}` is not valid for this Channel source role");
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_targets_normalize_hosts_ports_and_paths() -> Result<()> {
        let target = ChannelSourceTarget::Rpc {
            endpoint: "HTTP://LOCALHOST:80/rpc/v1".to_owned(),
        }
        .normalized(ChannelSourceRole::Sequencer, false)?;
        if target
            != (ChannelSourceTarget::Rpc {
                endpoint: "http://localhost/rpc/v1".to_owned(),
            })
        {
            bail!("RPC target was not normalized: {target:?}");
        }

        let root = ChannelSourceTarget::Rpc {
            endpoint: "http://127.0.0.1:3040".to_owned(),
        }
        .normalized(ChannelSourceRole::Sequencer, false)?;
        if root
            != (ChannelSourceTarget::Rpc {
                endpoint: "http://127.0.0.1:3040/".to_owned(),
            })
        {
            bail!("RPC root path was not normalized: {root:?}");
        }
        Ok(())
    }

    #[test]
    fn remote_http_requires_explicit_confirmation() -> Result<()> {
        let target = ChannelSourceTarget::Rpc {
            endpoint: "http://rpc.example.test/lez".to_owned(),
        };
        if target
            .clone()
            .normalized(ChannelSourceRole::Sequencer, false)
            .is_ok()
        {
            bail!("remote HTTP target should require confirmation");
        }
        let normalized = target.normalized(ChannelSourceRole::Sequencer, true)?;
        if !normalized.is_insecure_http() {
            bail!("remote HTTP target lost insecure transport status");
        }

        let secure = ChannelSourceTarget::Rpc {
            endpoint: "https://rpc.example.test/lez".to_owned(),
        }
        .normalized(ChannelSourceRole::Sequencer, false)?;
        if secure.is_insecure_http() {
            bail!("HTTPS target was marked insecure: {secure:?}");
        }
        Ok(())
    }

    #[test]
    fn rpc_targets_reject_credentials_queries_fragments_and_invalid_schemes() {
        for endpoint in [
            "http://user:pass@localhost:3040/",
            "http://localhost:3040/?token=secret",
            "http://localhost:3040/#fragment",
            "ftp://localhost:3040/",
            "http:///missing-host",
            "http://localhost:3040/\n",
        ] {
            let result = ChannelSourceTarget::Rpc {
                endpoint: endpoint.to_owned(),
            }
            .normalized(ChannelSourceRole::Sequencer, true);
            assert!(result.is_err(), "accepted invalid endpoint `{endpoint}`");
        }
    }

    #[test]
    fn module_targets_are_known_and_role_specific() -> Result<()> {
        let sequencer = ChannelSourceTarget::Module {
            module_id: LEZ_CORE_MODULE.to_owned(),
        }
        .normalized(ChannelSourceRole::Sequencer, false)?;
        let indexer = ChannelSourceTarget::Module {
            module_id: INDEXER_MODULE.to_owned(),
        }
        .normalized(ChannelSourceRole::Indexer, false)?;
        if sequencer.fingerprint() == indexer.fingerprint() {
            bail!("different module targets produced one fingerprint");
        }
        if (ChannelSourceTarget::Module {
            module_id: INDEXER_MODULE.to_owned(),
        })
        .normalized(ChannelSourceRole::Sequencer, false)
        .is_ok()
        {
            bail!("Indexer module was accepted for a Sequencer source");
        }
        if (ChannelSourceTarget::Module {
            module_id: "arbitrary_module".to_owned(),
        })
        .normalized(ChannelSourceRole::Indexer, false)
        .is_ok()
        {
            bail!("unknown module target was accepted");
        }
        Ok(())
    }

    #[test]
    fn channel_and_network_identities_normalize_to_canonical_hex() -> Result<()> {
        let upper = "AB".repeat(32);
        let normalized = normalize_channel_id(&format!(" 0x{upper} "))?;
        if normalized != "ab".repeat(32) {
            bail!("Channel id was not normalized: {normalized}");
        }
        let scope = normalize_network_scope(NetworkScope::FinalizedAnchor {
            genesis_time: " 2026-07-11T00:00:00Z ".to_owned(),
            block_slot: 42,
            block_id: upper.clone(),
            parent_id: "CD".repeat(32),
        })?;
        let NetworkScope::FinalizedAnchor {
            genesis_time,
            block_id,
            parent_id,
            ..
        } = scope
        else {
            bail!("network scope changed kind");
        };
        if genesis_time != "2026-07-11T00:00:00Z"
            || block_id != "ab".repeat(32)
            || parent_id != "cd".repeat(32)
        {
            bail!("network scope was not normalized");
        }
        Ok(())
    }

    #[test]
    fn persisted_attestation_must_match_owner_and_target() -> Result<()> {
        let channel_id = "1".repeat(64);
        let target = ChannelSourceTarget::Rpc {
            endpoint: "http://127.0.0.1:3040/".to_owned(),
        };
        let attestation = PersistedSequencerAttestation::PersistedAttested {
            channel_id: channel_id.clone(),
            target_fingerprint: target.fingerprint(),
            attested_at_unix: 10,
        };
        let normalized = normalized_attestation(attestation, &channel_id, &target)?;
        if !matches!(
            normalized,
            PersistedSequencerAttestation::PersistedAttested { .. }
        ) {
            bail!("matching attestation was not retained");
        }
        let mismatch = PersistedSequencerAttestation::PersistedAttested {
            channel_id: "2".repeat(64),
            target_fingerprint: target.fingerprint(),
            attested_at_unix: 10,
        };
        if normalized_attestation(mismatch, &channel_id, &target).is_ok() {
            bail!("cross-Channel attestation was accepted");
        }
        Ok(())
    }
}
