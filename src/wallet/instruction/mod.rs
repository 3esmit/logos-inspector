mod accounts;
mod model;
mod plan;
mod prepare;
mod report;
mod submit;
mod values;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::profile::resolve_instruction_wallet_home;
use plan::instruction_plan;
use prepare::prepare_instruction;
use report::report_from_prepared;
use submit::submit_instruction;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LocalWalletInstructionRequest {
    #[serde(default, alias = "idlJson")]
    pub idl_json: String,
    #[serde(default, alias = "programIdHex")]
    pub program_id_hex: String,
    #[serde(default, alias = "programBinary")]
    pub program_binary: String,
    #[serde(default, alias = "dependencyBinaries")]
    pub dependency_binaries: Vec<String>,
    #[serde(default)]
    pub instruction: String,
    #[serde(default)]
    pub accounts: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub args: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalWalletInstructionReport {
    pub source: String,
    pub status: String,
    pub mode: String,
    pub instruction: String,
    pub program_id_hex: String,
    pub command: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub operation_detail: String,
    pub program_binary_required: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub program_binary: String,
    pub accounts: Vec<ResolvedInstructionAccount>,
    pub args: Vec<ResolvedInstructionArg>,
    pub instruction_words: Vec<u32>,
    pub instruction_words_hex: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_secret_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_at: Option<String>,
}

pub use plan::{InstructionPlanField, LocalWalletInstructionPlanReport};

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedInstructionAccount {
    pub name: String,
    pub account_id: String,
    pub privacy: String,
    pub signer: bool,
    pub rest: bool,
    pub pda: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedInstructionArg {
    pub name: String,
    pub type_label: String,
    pub value: String,
}

pub fn local_wallet_instruction_preview(request: Value) -> Result<LocalWalletInstructionReport> {
    let request: LocalWalletInstructionRequest =
        serde_json::from_value(request).context("failed to parse IDL instruction request")?;
    let prepared = prepare_instruction(&request)?;
    Ok(report_from_prepared(prepared, "previewed", None, None))
}

pub fn local_wallet_instruction_plan(request: Value) -> Result<LocalWalletInstructionPlanReport> {
    let request: LocalWalletInstructionRequest =
        serde_json::from_value(request).context("failed to parse IDL instruction request")?;
    instruction_plan(&request)
}

pub async fn local_wallet_instruction_submit(
    profile: Value,
    request: Value,
) -> Result<LocalWalletInstructionReport> {
    local_wallet_instruction_submit_inner(profile, request, None).await
}

pub(crate) async fn local_wallet_instruction_submit_to(
    profile: Value,
    request: Value,
    sequencer_endpoint: String,
) -> Result<LocalWalletInstructionReport> {
    local_wallet_instruction_submit_inner(profile, request, Some(sequencer_endpoint)).await
}

async fn local_wallet_instruction_submit_inner(
    profile: Value,
    request: Value,
    sequencer_endpoint: Option<String>,
) -> Result<LocalWalletInstructionReport> {
    let wallet_home = resolve_instruction_wallet_home(profile)?;
    let request: LocalWalletInstructionRequest =
        serde_json::from_value(request).context("failed to parse IDL instruction request")?;
    let prepared = prepare_instruction(&request)?;
    let (tx_hash, shared_secret_count) = submit_instruction(
        wallet_home,
        &request,
        &prepared,
        sequencer_endpoint.as_deref(),
    )
    .await?;
    Ok(report_from_prepared(
        prepared,
        "submitted",
        Some(tx_hash),
        shared_secret_count,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;

    fn sample_request(account: &str) -> LocalWalletInstructionRequest {
        LocalWalletInstructionRequest {
            idl_json: json!({
                "name": "sample",
                "instructions": [{
                    "name": "set_value",
                    "accounts": [{"name": "target", "signer": true}],
                    "args": [{"name": "value", "type": "u32"}]
                }]
            })
            .to_string(),
            program_id_hex: "11".repeat(32),
            instruction: "set_value".to_owned(),
            accounts: BTreeMap::from([("target".to_owned(), account.to_owned())]),
            args: BTreeMap::from([("value".to_owned(), "7".to_owned())]),
            ..Default::default()
        }
    }

    #[test]
    fn preview_serializes_public_instruction_words() -> Result<()> {
        let account = format!("0x{}", "22".repeat(32));
        let request = serde_json::to_value(sample_request(&account))?;
        let report = local_wallet_instruction_preview(request)?;

        if report.mode != "public" {
            bail!("unexpected mode: {}", report.mode);
        }
        if report.instruction != "set_value" {
            bail!("unexpected instruction: {}", report.instruction);
        }
        if report.instruction_words != vec![0, 7] {
            bail!(
                "unexpected instruction words: {:?}",
                report.instruction_words
            );
        }
        let privacy = report
            .accounts
            .first()
            .map(|account| account.privacy.as_str());
        if privacy != Some("public") {
            bail!("unexpected account privacy: {privacy:?}");
        }
        Ok(())
    }

    #[test]
    fn private_instruction_requires_program_binary() -> Result<()> {
        let account = format!("Private/0x{}", "33".repeat(32));
        let request = serde_json::to_value(sample_request(&account))?;
        let result = local_wallet_instruction_preview(request);

        if result.is_ok() {
            bail!("expected private preview without program binary to fail");
        }
        let error = result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        if !error.contains("program binary") {
            bail!("unexpected error: {error}");
        }
        Ok(())
    }

    #[test]
    fn instruction_plan_reports_fields_and_completion() -> Result<()> {
        let account = format!("Private/0x{}", "33".repeat(32));
        let mut request = sample_request(&account);
        request.program_binary = "/tmp/program.bin".to_owned();
        let report = local_wallet_instruction_plan(serde_json::to_value(request)?)?;

        if report.instruction != "set_value" {
            bail!("unexpected instruction: {}", report.instruction);
        }
        if !report.private_mode {
            bail!("private mode was not detected");
        }
        if !report.program_binary_required {
            bail!("program binary was not required");
        }
        if !report.inputs_complete {
            bail!("complete request was reported incomplete");
        }
        let account_name = report.accounts.first().map(|field| field.name.as_str());
        if account_name != Some("target") {
            bail!("unexpected account field: {account_name:?}");
        }
        let arg_type = report.args.first().map(|field| field.type_label.as_str());
        if arg_type != Some("u32") {
            bail!("unexpected arg type: {arg_type:?}");
        }
        Ok(())
    }

    #[test]
    fn blank_instruction_plan_bootstraps_instruction_choices() -> Result<()> {
        let mut request = sample_request(&format!("Private/0x{}", "33".repeat(32)));
        request.idl_json = json!({
            "name": "sample",
            "instructions": [
                {
                    "name": "set_value",
                    "accounts": [{"name": "target", "signer": true}],
                    "args": [{"name": "value", "type": "u32"}]
                },
                {
                    "name": "close",
                    "accounts": [{"name": "target", "signer": true}],
                    "args": []
                }
            ]
        })
        .to_string();
        request.instruction.clear();
        request.program_binary = "/tmp/stale-program.bin".to_owned();

        let report = local_wallet_instruction_plan(serde_json::to_value(request)?)?;

        if !report.instruction.is_empty()
            || report.instructions != ["set_value", "close"]
            || !report.accounts.is_empty()
            || !report.args.is_empty()
            || report.private_mode
            || report.program_binary_required
            || report.inputs_complete
        {
            bail!("blank plan did not return selector bootstrap state");
        }
        Ok(())
    }
}
