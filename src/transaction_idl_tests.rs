use super::*;

#[test]
fn decode_event_data_hex_with_idl_decodes_single_event_without_name() {
    let idl = r#"{
        "name": "test_program",
        "x-logos-inspector-events": true,
        "events": [
            {
                "name": "LogEntry",
                "fields": [
                    { "name": "amount", "type": "u64" },
                    { "name": "memo", "type": "string" }
                ]
            }
        ]
    }"#;

    let report = decode_event_data_hex_with_idl(idl, None, "2a00000000000000020000006f6b");

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.event, "LogEntry");
    assert_eq!(report.consumed_bytes, 14);
    assert_eq!(report.total_bytes, 14);

    let amount = report.rows.iter().find(|row| row.path == "amount");
    assert!(amount.is_some(), "missing amount row");
    let Some(amount) = amount else {
        return;
    };
    assert_eq!(amount.value, "42");

    let memo = report.rows.iter().find(|row| row.path == "memo");
    assert!(memo.is_some(), "missing memo row");
    let Some(memo) = memo else {
        return;
    };
    assert_eq!(memo.value, "ok");
}

#[test]
fn decode_account_data_hex_with_idl_preserves_remaining_account_data() {
    let idl = r#"{
        "name": "test_program",
        "accounts": [
            {
                "name": "ShortAccount",
                "type": {
                    "kind": "struct",
                    "fields": [
                        { "name": "tag", "type": "u8" }
                    ]
                }
            }
        ]
    }"#;

    let report =
        decode_account_data_hex_with_idl(idl, Some("ShortAccount"), "010203", Some("acct"));

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.account_id.as_deref(), Some("acct"));
    assert_eq!(report.account_type, "ShortAccount");
    assert_eq!(report.consumed_bytes, 1);
    assert_eq!(report.total_bytes, 3);
    assert_eq!(report.remaining_bytes, 2);
    assert_eq!(report.remaining_data_hex.as_deref(), Some("0203"));

    let tag = report.rows.iter().find(|row| row.path == "tag");
    assert!(tag.is_some(), "missing tag row");
    let Some(tag) = tag else {
        return;
    };
    assert_eq!(tag.value, "1");

    let remaining = report
        .rows
        .iter()
        .find(|row| row.path == "remaining_data_hex");
    assert!(remaining.is_some(), "missing remaining data row");
}

#[test]
fn decode_account_data_hex_with_idl_decodes_fixed_arrays() {
    let idl = r#"{
        "name": "test_program",
        "accounts": [
            {
                "name": "ArrayAccount",
                "type": {
                    "kind": "struct",
                    "fields": [
                        { "name": "bytes", "type": { "array": ["u8", 3] } },
                        { "name": "tail", "type": { "array": { "type": "u8", "len": 2 } } }
                    ]
                }
            }
        ]
    }"#;

    let report =
        decode_account_data_hex_with_idl(idl, Some("ArrayAccount"), "0102030405", Some("acct"));

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.consumed_bytes, 5);
    assert_eq!(
        report.rows.iter().find(|row| row.path == "bytes[0]"),
        Some(&DecodedField {
            path: "bytes[0]".to_owned(),
            value: "1".to_owned()
        })
    );
    assert_eq!(
        report.rows.iter().find(|row| row.path == "tail[1]"),
        Some(&DecodedField {
            path: "tail[1]".to_owned(),
            value: "5".to_owned()
        })
    );
}

#[test]
fn decode_event_data_hex_with_idl_selects_named_event_type_shape() {
    let idl = r#"{
        "name": "test_program",
        "extensions": { "logos_inspector_events": true },
        "events": [
            {
                "name": "Ignored",
                "fields": [
                    { "name": "value", "type": "u8" }
                ]
            },
            {
                "name": "ValueChanged",
                "type": {
                    "kind": "struct",
                    "fields": [
                        { "name": "value", "type": "u16" },
                        { "name": "enabled", "type": "bool" }
                    ]
                }
            }
        ]
    }"#;

    let report = decode_event_data_hex_with_idl(idl, Some("ValueChanged"), "010201");

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.event, "ValueChanged");

    let value = report.rows.iter().find(|row| row.path == "value");
    assert!(value.is_some(), "missing value row");
    let Some(value) = value else {
        return;
    };
    assert_eq!(value.value, "513");

    let enabled = report.rows.iter().find(|row| row.path == "enabled");
    assert!(enabled.is_some(), "missing enabled row");
    let Some(enabled) = enabled else {
        return;
    };
    assert_eq!(enabled.value, "true");
}

#[test]
fn decode_event_data_hex_with_idl_rejects_standard_spel_idl_without_event_extension() {
    let idl = r#"{
        "name": "test_program",
        "events": [
            {
                "name": "LogEntry",
                "fields": [
                    { "name": "amount", "type": "u64" }
                ]
            }
        ]
    }"#;

    let result = decode_event_data_hex_with_idl(idl, None, "2a00000000000000");
    assert!(result.is_err(), "{result:?}");
    let Err(error) = result else {
        return;
    };

    assert!(
        error.to_string().contains("nonstandard events extension"),
        "{error:#}"
    );
}

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
fn decode_instruction_words_with_idl_decodes_fixed_array_args() {
    let idl = r#"{
        "name": "test_program",
        "instructions": [
            {
                "name": "set_values",
                "args": [
                    { "name": "values", "type": { "array": ["u32", 3] } }
                ]
            }
        ]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &[0, 10, 20, 30], &[]);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.instruction, "set_values");
    assert_eq!(
        report.args.first(),
        Some(&DecodedField {
            path: "values: array<u32, 3>".to_owned(),
            value: "[10, 20, 30]".to_owned()
        })
    );
}

#[test]
fn decode_instruction_words_with_idl_reports_arg_decode_error() {
    let idl = r#"{
        "name": "test_program",
        "instructions": [
            {
                "name": "set_program",
                "args": [
                    { "name": "program", "type": { "option": "program_id" } }
                ]
            }
        ]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &[0, 7, 42], &[]);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.instruction, "set_program");
    assert_eq!(report.decode_error.as_deref(), Some("invalid option tag 7"));
    assert_eq!(report.remaining_words, vec![7, 42]);
    assert_eq!(
        report.args.first(),
        Some(&DecodedField {
            path: "program: option<program_id>".to_owned(),
            value: "unsupported (invalid option tag 7); raw words 1..2".to_owned()
        })
    );
}

#[test]
fn decode_instruction_words_with_idl_allows_string_instruction_type() {
    let idl = r#"{
        "name": "test_program",
        "instruction_type": "test_program::Instruction",
        "instructions": [
            {
                "name": "set_value",
                "args": [
                    { "name": "value", "type": "u32" }
                ]
            }
        ]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &[0, 9], &[]);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.instruction, "set_value");
    assert_eq!(
        report.args.first(),
        Some(&DecodedField {
            path: "value: u32".to_owned(),
            value: "9".to_owned()
        })
    );
}

#[test]
fn decode_instruction_words_with_idl_rejects_structured_external_instruction_type() {
    let idl = r#"{
        "name": "test_program",
        "instruction_type": { "defined": "Instruction" },
        "instructions": [
            { "name": "set_value", "args": [] }
        ]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &[0], &[]);

    assert!(report.is_err());
    assert!(report.err().is_some_and(|error| {
        error
            .to_string()
            .contains("positional instruction decode is unsafe")
    }));
}

#[test]
fn trace_transaction_summary_with_idl_adds_decode_steps() {
    // Arrange
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

    // Act
    let report = trace_transaction_summary_with_idl(&summary, idl);

    // Assert
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
    // Arrange
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

    // Act
    let report = trace_transaction_summary_with_idl(&summary, "{");

    // Assert
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
    // Arrange
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

    // Act
    let report = trace_transaction_summary_with_idl(&summary, idl);

    // Assert
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
