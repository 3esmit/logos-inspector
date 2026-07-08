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
    let id = normalized_topic_id(id)?;
    Some(format!(
        "/{}/{}/{}/comments",
        layer.as_topic_segment(),
        entity.as_topic_segment(),
        id
    ))
}

#[must_use]
pub fn lez_account_idl_topic(id: &str) -> Option<String> {
    let id = normalized_topic_id(id)?;
    Some(format!("/lez/account/{id}/idl"))
}

#[must_use]
pub fn social_topic_is_valid(topic: &str) -> bool {
    let mut segments = topic.trim().split('/');
    segments.next() == Some("")
        && segments.next().is_some_and(|segment| !segment.is_empty())
        && segments.next().is_some_and(|segment| !segment.is_empty())
        && segments.next().is_some_and(|segment| !segment.is_empty())
        && segments.next().is_some_and(|segment| !segment.is_empty())
        && segments.next().is_none()
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
