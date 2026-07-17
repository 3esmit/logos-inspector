use anyhow::{Context as _, Result, bail};
use common::HashType;
use lee::AccountId;

pub(crate) fn normalize_block_id_text(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("block id is required");
    }
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("block ID must be 64 hexadecimal characters (optional 0x prefix)");
    }
    Ok(hex.to_ascii_lowercase())
}

pub fn normalize_program_id_hex(value: &str) -> Result<String> {
    let text = value.trim();
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        let bytes = hex::decode(hex).context("invalid program id hex")?;
        if bytes.len() != 32 {
            bail!("program id hex must be 32 bytes");
        }
        return Ok(hex::encode(bytes));
    }
    if text.len() == 64 && text.chars().all(|ch| ch.is_ascii_hexdigit()) {
        let bytes = hex::decode(text).context("invalid program id hex")?;
        return Ok(hex::encode(bytes));
    }
    let account_id = parse_account_id(text)?;
    Ok(hex::encode(account_id.value()))
}

pub(crate) fn parse_account_id(value: &str) -> Result<AccountId> {
    let value = normalized_public_account_id(value)?;
    if let Some(account_id) = parse_account_id_hex(value)? {
        return Ok(account_id);
    }
    value
        .parse()
        .with_context(|| format!("invalid account id `{value}`"))
}

fn normalized_public_account_id(value: &str) -> Result<&str> {
    let value = value.trim();
    if let Some(private) = value
        .strip_prefix("Private/")
        .or_else(|| value.strip_prefix("private/"))
    {
        let _ = private;
        bail!(
            "private account state is local wallet state; public RPC cannot fetch `Private/` accounts"
        )
    }
    Ok(value
        .strip_prefix("Public/")
        .or_else(|| value.strip_prefix("public/"))
        .unwrap_or(value)
        .trim())
}

fn parse_account_id_hex(value: &str) -> Result<Option<AccountId>> {
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    let explicit_hex = hex.len() != value.len();
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        if explicit_hex {
            bail!("invalid account id `{value}`");
        }
        return Ok(None);
    }
    let bytes = hex::decode(hex).context("invalid account id hex")?;
    let mut fixed = [0_u8; 32];
    fixed.copy_from_slice(&bytes);
    Ok(Some(AccountId::new(fixed)))
}

pub(crate) fn normalize_account_id_text(value: &str) -> Option<String> {
    parse_account_id(value)
        .ok()
        .map(|account_id| account_id.to_string())
}

pub(crate) fn parse_hash(value: &str, label: &str) -> Result<HashType> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    value
        .parse()
        .with_context(|| format!("invalid {label} `{value}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_id_normalization_matches_bedrock_header_id_text() -> Result<()> {
        let canonical = "ab".repeat(32);
        let normalized = normalize_block_id_text(&format!("  0x{}  ", canonical.to_uppercase()))?;
        if normalized != canonical {
            bail!("prefixed uppercase block id did not normalize");
        }

        for invalid in [
            "block/path",
            "block\\path",
            ".",
            "..",
            "%2e%2e",
            "a\nb",
            &"a".repeat(63),
            &"a".repeat(65),
            &"g".repeat(64),
        ] {
            let Err(error) = normalize_block_id_text(invalid) else {
                bail!("noncanonical block id {invalid:?} should fail");
            };
            if !error
                .to_string()
                .contains("block ID must be 64 hexadecimal characters (optional 0x prefix)")
            {
                bail!("unexpected error for {invalid:?}: {error:#}");
            }
        }
        Ok(())
    }

    #[test]
    fn parse_account_id_accepts_hex_with_optional_prefix() {
        let account_id = AccountId::new([7_u8; 32]);
        let hex = hex::encode(account_id.value());

        assert_eq!(parse_account_id(&hex).ok(), Some(account_id));
        assert_eq!(parse_account_id(&format!("0x{hex}")).ok(), Some(account_id));
    }

    #[test]
    fn parse_account_id_accepts_public_prefix_and_rejects_private_prefix() {
        let account_id = AccountId::new([9_u8; 32]);
        let encoded = account_id.to_string();

        assert_eq!(
            parse_account_id(&format!("Public/{encoded}")).ok(),
            Some(account_id)
        );
        let result = parse_account_id(&format!("Private/{encoded}"));
        assert!(result.is_err(), "{result:?}");
        let Err(error) = result else {
            return;
        };
        assert!(error.to_string().contains("private account state"));
    }
}
