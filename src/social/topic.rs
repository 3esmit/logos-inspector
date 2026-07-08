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

impl SocialLayer {
    fn as_topic_segment(self) -> &'static str {
        match self {
            Self::Cryptarchia => "cryptarchia",
            Self::Lez => "lez",
        }
    }
}

impl SocialEntity {
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
