use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::inspection::{
    NetworkScope,
    l2::{ZoneL2EntityKind, ZoneL2EntityRef, canonical_hash},
};

const LEZ_TOPIC_DOMAIN: &[u8] = b"logos.lez.collaboration.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialLayer {
    Cryptarchia,
    Lez,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialEntity {
    Transaction,
    Block,
    Account,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneSocialScope {
    pub network_scope: NetworkScope,
    pub zone_id: String,
    pub entity_kind: ZoneL2EntityKind,
    pub canonical_entity_key: String,
}

#[must_use]
pub fn comment_topic_from_parts(layer: &str, entity: &str, id: &str) -> Option<String> {
    comment_topic(
        SocialLayer::from_text(layer)?,
        SocialEntity::from_text(entity)?,
        id,
    )
}

#[must_use]
pub fn comment_topic(layer: SocialLayer, entity: SocialEntity, id: &str) -> Option<String> {
    if layer == SocialLayer::Lez {
        return None;
    }
    let id = normalized_topic_id(id)?;
    Some(format!(
        "/{}/{}/{}/comments",
        layer.as_topic_segment(),
        entity.as_topic_segment(),
        id
    ))
}

#[must_use]
pub fn zone_comment_topic(entity: &ZoneL2EntityRef) -> Option<String> {
    let scope = ZoneSocialScope::from_entity_ref(entity)?;
    let segment = scope.entity_segment()?;
    Some(format!("/lez/{segment}/{}/comments", scope.digest()?))
}

#[must_use]
pub fn zone_account_idl_topic(entity: &ZoneL2EntityRef) -> Option<String> {
    let scope = ZoneSocialScope::from_entity_ref(entity)?;
    if scope.entity_kind != ZoneL2EntityKind::Account {
        return None;
    }
    Some(format!("/lez/account/{}/idl", scope.digest()?))
}

#[must_use]
pub fn zone_topic_matches_scope(topic: &str, scope: &ZoneSocialScope) -> bool {
    let Some(segment) = scope.entity_segment() else {
        return false;
    };
    let Some(digest) = scope.digest() else {
        return false;
    };
    topic == format!("/lez/{segment}/{digest}/comments")
        || (scope.entity_kind == ZoneL2EntityKind::Account
            && topic == format!("/lez/account/{digest}/idl"))
}

#[must_use]
pub fn social_topic_is_valid(topic: &str) -> bool {
    let mut segments = topic.trim().split('/');
    let (Some(""), Some(layer), Some(entity), Some(identity), Some(suffix)) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return false;
    };
    if segments.next().is_some() {
        return false;
    }
    if layer == "lez" {
        return matches!(entity, "block" | "transaction" | "account")
            && identity.len() == 64
            && identity
                .chars()
                .all(|character| character.is_ascii_hexdigit())
            && matches!(suffix, "comments" | "idl")
            && (suffix != "idl" || entity == "account");
    }
    layer == "cryptarchia" && !entity.is_empty() && !identity.is_empty() && suffix == "comments"
}

impl ZoneSocialScope {
    #[must_use]
    pub fn from_entity_ref(entity: &ZoneL2EntityRef) -> Option<Self> {
        Self {
            network_scope: entity.network_scope.clone(),
            zone_id: entity.channel_id.clone(),
            entity_kind: entity.entity_kind,
            canonical_entity_key: entity.canonical_key.clone(),
        }
        .canonicalized()
    }

    pub(crate) fn canonicalized(&self) -> Option<Self> {
        let NetworkScope::GenesisId { genesis_id } = &self.network_scope else {
            return None;
        };
        Some(Self {
            network_scope: NetworkScope::GenesisId {
                genesis_id: canonical_hex_identity(genesis_id)?,
            },
            zone_id: canonical_hex_identity(&self.zone_id)?,
            entity_kind: self.entity_kind,
            canonical_entity_key: canonical_entity_key(
                self.entity_kind,
                &self.canonical_entity_key,
            )?,
        })
    }

    #[must_use]
    pub fn digest(&self) -> Option<String> {
        let canonical = self.canonicalized()?;
        let NetworkScope::GenesisId { genesis_id } = &canonical.network_scope else {
            return None;
        };
        let entity_kind = canonical.entity_segment()?;
        let mut digest = Sha256::new();
        digest_component(&mut digest, LEZ_TOPIC_DOMAIN);
        digest_component(&mut digest, genesis_id.as_bytes());
        digest_component(&mut digest, canonical.zone_id.as_bytes());
        digest_component(&mut digest, entity_kind.as_bytes());
        digest_component(&mut digest, canonical.canonical_entity_key.as_bytes());
        Some(hex::encode(digest.finalize()))
    }

    fn entity_segment(&self) -> Option<&'static str> {
        match self.entity_kind {
            ZoneL2EntityKind::Block => Some("block"),
            ZoneL2EntityKind::Transaction => Some("transaction"),
            ZoneL2EntityKind::Account => Some("account"),
            ZoneL2EntityKind::Program => None,
        }
    }
}

impl SocialLayer {
    fn from_text(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "cryptarchia" | "bedrock" | "l1" => Some(Self::Cryptarchia),
            "lez" | "l2" => Some(Self::Lez),
            _ => None,
        }
    }

    fn as_topic_segment(self) -> &'static str {
        match self {
            Self::Cryptarchia => "cryptarchia",
            Self::Lez => "lez",
        }
    }
}

impl SocialEntity {
    fn from_text(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "transaction" | "tx" => Some(Self::Transaction),
            "block" => Some(Self::Block),
            "account" => Some(Self::Account),
            _ => None,
        }
    }

    fn as_topic_segment(self) -> &'static str {
        match self {
            Self::Transaction => "transaction",
            Self::Block => "block",
            Self::Account => "account",
        }
    }
}

fn normalized_topic_id(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() || trimmed.contains('/') {
        return None;
    }
    Some(trimmed.to_owned())
}

fn canonical_hex_identity(value: &str) -> Option<String> {
    let value = value.trim().strip_prefix("0x").unwrap_or(value.trim());
    (value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

fn canonical_entity_key(entity_kind: ZoneL2EntityKind, value: &str) -> Option<String> {
    match entity_kind {
        ZoneL2EntityKind::Block => {
            let (block_id, block_hash) = value.trim().strip_prefix("block:")?.split_once(':')?;
            let block_id = block_id.parse::<u64>().ok()?;
            let block_hash = canonical_hash(block_hash).ok()?;
            Some(format!("block:{block_id}:{block_hash}"))
        }
        ZoneL2EntityKind::Transaction => canonical_hash(value).ok(),
        ZoneL2EntityKind::Account => crate::parse_account_id(value)
            .ok()
            .map(|account_id| account_id.to_string()),
        ZoneL2EntityKind::Program => None,
    }
}

fn digest_component(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}
