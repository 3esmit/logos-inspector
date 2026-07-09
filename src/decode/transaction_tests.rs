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
