use anyhow::{Context as _, Result};
use sequencer_service_rpc::RpcClient as _;
use serde::Serialize;
use serde_json::{Value, json};

use super::{
    programs::{program_id_base58, program_id_hex},
    sequencer::sequencer_client,
    transaction_facts::{
        AccountTransactionSummary, summarize_indexer_transaction, with_account_direction,
    },
};
use crate::{
    ACCOUNT_TRANSACTION_LIMIT, AccountIdlDecodeReport, decode_account_data_hex_with_idl,
    json_rpc_result, parse_account_id, raw_json_rpc,
};

#[derive(Debug, Clone, Serialize)]
pub struct AccountReport {
    pub account_id: String,
    pub account_id_base58: String,
    pub account_id_hex: String,
    pub account: Value,
    pub balance: String,
    pub nonce: String,
    pub owner_base58: String,
    pub owner_hex: String,
    pub data_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_transactions: Option<Vec<AccountTransactionSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_transactions_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SequencerAccountIdlReport {
    pub account: AccountReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode: Option<AccountIdlDecodeReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_error: Option<String>,
}

pub async fn sequencer_account(endpoint: &str, account_id: &str) -> Result<AccountReport> {
    let parsed_account_id = parse_account_id(account_id)?;
    let account = sequencer_client(endpoint)?
        .get_account(parsed_account_id)
        .await
        .with_context(|| format!("failed to fetch sequencer account {account_id}"))?;
    let account_json = serde_json::to_value(&account).context("failed to serialize account")?;
    let data_hex = hex::encode(account.data.into_inner());
    let account_id_base58 = parsed_account_id.to_string();
    let account_id_hex = hex::encode(parsed_account_id.value());
    let owner_base58 = program_id_base58(account.program_owner);
    let owner_hex = program_id_hex(account.program_owner);
    Ok(AccountReport {
        account_id: account_id_base58.clone(),
        account_id_base58,
        account_id_hex,
        account: account_json,
        balance: account.balance.to_string(),
        nonce: account.nonce.0.to_string(),
        owner_base58,
        owner_hex,
        data_hex,
        related_transactions: None,
        related_transactions_error: None,
    })
}

pub async fn account_lookup(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
    account_id: &str,
) -> Result<AccountReport> {
    let mut account = sequencer_account(sequencer_endpoint, account_id).await?;
    match account_transactions_by_account(
        indexer_endpoint,
        &account.account_id_base58,
        0,
        ACCOUNT_TRANSACTION_LIMIT,
    )
    .await
    {
        Ok(transactions) => {
            account.related_transactions = Some(transactions);
        }
        Err(error) => {
            account.related_transactions = Some(Vec::new());
            account.related_transactions_error = Some(format!("{error:#}"));
        }
    }
    Ok(account)
}

pub async fn sequencer_account_with_idl(
    endpoint: &str,
    account_id: &str,
    idl_json: &str,
    account_type: Option<&str>,
) -> Result<SequencerAccountIdlReport> {
    let account = sequencer_account(endpoint, account_id).await?;
    Ok(account_report_with_optional_idl_decode(
        account,
        idl_json,
        account_type,
    ))
}

pub async fn account_lookup_with_idl(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
    account_id: &str,
    idl_json: &str,
    account_type: Option<&str>,
) -> Result<SequencerAccountIdlReport> {
    let account = account_lookup(sequencer_endpoint, indexer_endpoint, account_id).await?;
    Ok(account_report_with_optional_idl_decode(
        account,
        idl_json,
        account_type,
    ))
}

pub(crate) fn account_report_with_optional_idl_decode(
    account: AccountReport,
    idl_json: &str,
    account_type: Option<&str>,
) -> SequencerAccountIdlReport {
    let decode = decode_account_data_hex_with_idl(
        idl_json,
        account_type,
        &account.data_hex,
        Some(&account.account_id),
    );
    match decode {
        Ok(decode) => SequencerAccountIdlReport {
            account,
            decode: Some(decode),
            decode_error: None,
        },
        Err(error) => SequencerAccountIdlReport {
            account,
            decode: None,
            decode_error: Some(format!("{error:#}")),
        },
    }
}

pub async fn account_transactions_by_account(
    indexer_endpoint: &str,
    account_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<AccountTransactionSummary>> {
    let account_id = parse_account_id(account_id)?.to_string();
    let response = raw_json_rpc(
        indexer_endpoint,
        "getTransactionsByAccount",
        json!([account_id.as_str(), offset, limit]),
    )
    .await
    .with_context(|| format!("failed to fetch transactions for account {account_id}"))?;
    let Some(result) = json_rpc_result(&response, "getTransactionsByAccount")? else {
        return Ok(Vec::new());
    };
    let transactions = result
        .as_array()
        .context("getTransactionsByAccount result was not an array")?;
    Ok(transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| {
            summarize_account_transaction(transaction, offset + index, &account_id)
        })
        .collect())
}

pub(crate) fn summarize_account_transaction(
    value: &Value,
    index: usize,
    account_id: &str,
) -> AccountTransactionSummary {
    with_account_direction(summarize_indexer_transaction(value, index), account_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_report_with_optional_idl_decode_preserves_account_when_decode_fails() {
        let account = AccountReport {
            account_id: "acct".to_owned(),
            account_id_base58: "acct".to_owned(),
            account_id_hex: "00".to_owned(),
            account: serde_json::json!({ "balance": "0" }),
            balance: "0".to_owned(),
            nonce: "0".to_owned(),
            owner_base58: "owner".to_owned(),
            owner_hex: "00".to_owned(),
            data_hex: "ff".to_owned(),
            related_transactions: Some(Vec::new()),
            related_transactions_error: None,
        };
        let idl = r#"{
            "accounts": [
                {
                    "name": "TooLong",
                    "type": {
                        "kind": "struct",
                        "fields": [
                            { "name": "amount", "type": "u64" }
                        ]
                    }
                }
            ]
        }"#;

        let report = account_report_with_optional_idl_decode(account, idl, Some("TooLong"));

        assert_eq!(report.account.account_id, "acct");
        assert!(report.decode.is_none());
        assert!(
            report
                .decode_error
                .as_deref()
                .is_some_and(|error| error.contains("failed to decode as `TooLong`"))
        );
    }
}
