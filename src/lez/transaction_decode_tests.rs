use super::{
    TransactionSummary, inspect_transaction_summary_with_idl, trace_transaction_summary_with_idl,
};

#[test]
fn inspect_transaction_summary_with_idl_adds_instruction_decode() {
    let summary = TransactionSummary {
        hash: "abcd1234".to_owned(),
        kind: "Public".to_owned(),
        program_id_hex: Some(
            "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
        ),
        account_ids: vec!["acct-a".to_owned()],
        nonces: vec![],
        instruction_data: vec![0, 9],
        bytecode_len: None,
        raw_signature_valid: None,
        message_prehash: None,
        prehash_signature_valid: None,
    };
    let idl = r#"{
        "name": "test_program",
        "instructions": [
            {
                "name": "set_value",
                "accounts": [
                    { "name": "target" }
                ],
                "args": [
                    { "name": "value", "type": "u32" }
                ]
            }
        ]
    }"#;

    let report = inspect_transaction_summary_with_idl(&summary, idl);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.inspection.hash, "abcd1234");
    assert_eq!(report.inspection.kind, "Public");

    let decoded = report.decoded_instruction.as_ref();
    assert!(decoded.is_some(), "missing instruction decode");
    let Some(decoded) = decoded else {
        return;
    };
    assert_eq!(decoded.instruction, "set_value");
    assert_eq!(decoded.variant_index, 0);

    let target = decoded.accounts.iter().find(|row| row.path == "target");
    assert!(target.is_some(), "missing target account");
    let Some(target) = target else {
        return;
    };
    assert_eq!(target.value, "acct-a");

    let value = decoded.args.iter().find(|row| row.path == "value: u32");
    assert!(value.is_some(), "missing value arg");
    let Some(value) = value else {
        return;
    };
    assert_eq!(value.value, "9");
}

#[test]
fn trace_transaction_summary_with_idl_adds_decode_steps() {
    let summary = TransactionSummary {
        hash: "abcd1234".to_owned(),
        kind: "Public".to_owned(),
        program_id_hex: Some(
            "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
        ),
        account_ids: vec!["acct-a".to_owned()],
        nonces: vec![],
        instruction_data: vec![0, 9],
        bytecode_len: None,
        raw_signature_valid: None,
        message_prehash: None,
        prehash_signature_valid: None,
    };
    let idl = r#"{
        "name": "test_program",
        "instructions": [
            {
                "name": "set_value",
                "accounts": [
                    { "name": "target" }
                ],
                "args": [
                    { "name": "value", "type": "u32" }
                ]
            }
        ]
    }"#;

    let report = trace_transaction_summary_with_idl(&summary, idl);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(
        report.source,
        "sequencer transaction summary + user supplied IDL"
    );
    assert!(
        report.decoded_instruction.is_some(),
        "missing decode report"
    );

    let decode = report
        .steps
        .iter()
        .find(|step| step.label == "IDL instruction decode");
    assert!(decode.is_some(), "missing decode step");
    let Some(decode) = decode else {
        return;
    };
    assert_eq!(decode.phase, "decode");
    assert_eq!(decode.status.as_deref(), Some("decoded"));

    let decoded_account = report
        .steps
        .iter()
        .find(|step| step.label == "decoded instruction account");
    assert!(decoded_account.is_some(), "missing decoded account step");
    let Some(decoded_account) = decoded_account else {
        return;
    };
    assert_eq!(
        decoded_account
            .refs
            .as_ref()
            .and_then(|refs| refs.decode_path.as_deref()),
        Some("target")
    );
    assert_eq!(
        decoded_account
            .refs
            .as_ref()
            .and_then(|refs| refs.account_id.as_deref()),
        Some("acct-a")
    );

    let decoded_arg = report
        .steps
        .iter()
        .find(|step| step.label == "decoded instruction arg");
    assert!(decoded_arg.is_some(), "missing decoded arg step");
    let Some(decoded_arg) = decoded_arg else {
        return;
    };
    assert!(
        decoded_arg
            .details
            .iter()
            .any(|detail| detail.contains("value: u32 9")),
        "{decoded_arg:?}"
    );
}

#[test]
fn trace_transaction_summary_with_invalid_idl_preserves_raw_trace() {
    let summary = TransactionSummary {
        hash: "abcd1234".to_owned(),
        kind: "Public".to_owned(),
        program_id_hex: Some(
            "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
        ),
        account_ids: vec!["acct-a".to_owned()],
        nonces: vec![],
        instruction_data: vec![3, 9],
        bytecode_len: None,
        raw_signature_valid: None,
        message_prehash: None,
        prehash_signature_valid: None,
    };

    let report = trace_transaction_summary_with_idl(&summary, "{");

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert!(report.decoded_instruction.is_none());
    assert!(
        report
            .limitations
            .iter()
            .any(|item| item.contains("IDL decode failed; raw instruction trace preserved")),
        "{report:?}"
    );

    let raw_word = report.steps.iter().find(|step| {
        step.label == "instruction word"
            && step
                .refs
                .as_ref()
                .and_then(|refs| refs.instruction_word_index)
                == Some(1)
    });
    assert!(raw_word.is_some(), "missing raw instruction word step");

    let decode_warning = report
        .steps
        .iter()
        .find(|step| step.label == "IDL instruction decode unavailable");
    assert!(decode_warning.is_some(), "missing decode warning step");
    let Some(decode_warning) = decode_warning else {
        return;
    };
    assert_eq!(decode_warning.phase, "decode");
    assert_eq!(decode_warning.status.as_deref(), Some("error"));
    assert_eq!(decode_warning.severity.as_deref(), Some("warning"));
    assert!(
        decode_warning
            .details
            .iter()
            .any(|detail| detail.contains("failed to parse IDL JSON")),
        "{decode_warning:?}"
    );
}

#[test]
fn trace_transaction_summary_with_idl_omits_placeholder_account_refs() {
    let summary = TransactionSummary {
        hash: "abcd1234".to_owned(),
        kind: "Public".to_owned(),
        program_id_hex: Some(
            "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
        ),
        account_ids: vec![],
        nonces: vec![],
        instruction_data: vec![0, 9],
        bytecode_len: None,
        raw_signature_valid: None,
        message_prehash: None,
        prehash_signature_valid: None,
    };
    let idl = r#"{
        "name": "test_program",
        "instructions": [
            {
                "name": "set_value",
                "accounts": [
                    { "name": "target" }
                ],
                "args": [
                    { "name": "value", "type": "u32" }
                ]
            }
        ]
    }"#;

    let report = trace_transaction_summary_with_idl(&summary, idl);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    let decoded_account = report
        .steps
        .iter()
        .find(|step| step.label == "decoded instruction account");
    assert!(decoded_account.is_some(), "missing decoded account step");
    let Some(decoded_account) = decoded_account else {
        return;
    };
    assert_eq!(
        decoded_account
            .refs
            .as_ref()
            .and_then(|refs| refs.decode_path.as_deref()),
        Some("target")
    );
    assert_eq!(
        decoded_account
            .refs
            .as_ref()
            .and_then(|refs| refs.account_id.as_deref()),
        None
    );
}

#[test]
fn inspect_transaction_summary_with_idl_skips_decode_without_public_program_invocation() {
    let summary = TransactionSummary {
        hash: "abcd1234".to_owned(),
        kind: "Public".to_owned(),
        program_id_hex: None,
        account_ids: vec!["acct-a".to_owned()],
        nonces: vec![],
        instruction_data: vec![0, 9],
        bytecode_len: None,
        raw_signature_valid: None,
        message_prehash: None,
        prehash_signature_valid: None,
    };

    let report = inspect_transaction_summary_with_idl(&summary, "{}");

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert!(report.decoded_instruction.is_none());

    let non_public_summary = TransactionSummary {
        kind: "ProgramDeployment".to_owned(),
        program_id_hex: Some(
            "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
        ),
        ..summary
    };
    let report = inspect_transaction_summary_with_idl(&non_public_summary, "{}");

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert!(report.decoded_instruction.is_none());
}
