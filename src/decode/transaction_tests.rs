use super::*;
use lee::AccountId;
use serde::Serialize;

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
fn decode_instruction_words_with_idl_matches_risc0_account_id_wire() {
    #[derive(Serialize)]
    enum ReferenceInstruction {
        SetOwner(AccountId, u64),
    }

    let account_id = AccountId::new([7_u8; 32]);
    let words = risc0_zkvm::serde::to_vec(&ReferenceInstruction::SetOwner(account_id, 42));
    assert!(words.is_ok(), "{words:?}");
    let Ok(words) = words else {
        return;
    };
    let idl = r#"{
        "name": "test_program",
        "instructions": [{
            "name": "set_owner",
            "args": [
                { "name": "owner", "type": "account_id" },
                { "name": "value", "type": "u64" }
            ]
        }]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &words, &[]);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.decode_error, None);
    assert_eq!(report.remaining_words, Vec::<u32>::new());
    assert_eq!(
        report.args,
        vec![
            DecodedField {
                path: "owner: account_id".to_owned(),
                value: account_id.to_string(),
            },
            DecodedField {
                path: "value: u64".to_owned(),
                value: "42".to_owned(),
            },
        ]
    );
}

#[test]
fn decode_instruction_words_with_idl_matches_risc0_optional_account_id_wire() {
    #[derive(Serialize)]
    enum ReferenceInstruction {
        SetAuthority(Option<AccountId>),
    }

    let account_id = AccountId::new([9_u8; 32]);
    let words = risc0_zkvm::serde::to_vec(&ReferenceInstruction::SetAuthority(Some(account_id)));
    assert!(words.is_ok(), "{words:?}");
    let Ok(words) = words else {
        return;
    };
    let idl = r#"{
        "name": "test_program",
        "instructions": [{
            "name": "set_authority",
            "args": [{ "name": "authority", "type": { "option": "account_id" } }]
        }]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &words, &[]);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.decode_error, None);
    assert_eq!(report.remaining_words, Vec::<u32>::new());
    assert_eq!(
        report.args.first(),
        Some(&DecodedField {
            path: "authority: option<account_id>".to_owned(),
            value: format!("Some({account_id})"),
        })
    );
}

#[test]
fn decode_instruction_words_with_idl_matches_risc0_signed_scalar_wire() {
    #[derive(Serialize)]
    enum ReferenceInstruction {
        SetSigned(i8, i8, i16, i16, i32, i32, i64, i64, i128, i128, u32),
    }

    let words = risc0_zkvm::serde::to_vec(&ReferenceInstruction::SetSigned(
        i8::MIN,
        i8::MAX,
        i16::MIN,
        i16::MAX,
        i32::MIN,
        i32::MAX,
        i64::MIN,
        i64::MAX,
        i128::MIN,
        i128::MAX,
        42,
    ));
    assert!(words.is_ok(), "{words:?}");
    let Ok(words) = words else {
        return;
    };
    let idl = r#"{
        "name": "test_program",
        "instructions": [{
            "name": "set_signed",
            "args": [
                { "name": "i8_min", "type": "i8" },
                { "name": "i8_max", "type": "i8" },
                { "name": "i16_min", "type": "i16" },
                { "name": "i16_max", "type": "i16" },
                { "name": "i32_min", "type": "i32" },
                { "name": "i32_max", "type": "i32" },
                { "name": "i64_min", "type": "i64" },
                { "name": "i64_max", "type": "i64" },
                { "name": "i128_min", "type": "i128" },
                { "name": "i128_max", "type": "i128" },
                { "name": "following", "type": "u32" }
            ]
        }]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &words, &[]);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.decode_error, None);
    assert_eq!(report.remaining_words, Vec::<u32>::new());
    assert_eq!(
        report
            .args
            .iter()
            .map(|field| field.value.clone())
            .collect::<Vec<_>>(),
        vec![
            i8::MIN.to_string(),
            i8::MAX.to_string(),
            i16::MIN.to_string(),
            i16::MAX.to_string(),
            i32::MIN.to_string(),
            i32::MAX.to_string(),
            i64::MIN.to_string(),
            i64::MAX.to_string(),
            i128::MIN.to_string(),
            i128::MAX.to_string(),
            "42".to_owned(),
        ]
    );
}

#[test]
fn decode_instruction_words_with_idl_rejects_out_of_range_narrow_signed_wire() {
    for (ty, malformed_word, expected_error) in [
        ("i8", 0x0000_0080, "i8 value 128 is out of range"),
        ("i16", 0x0000_8000, "i16 value 32768 is out of range"),
    ] {
        let idl = format!(
            r#"{{
                "name": "test_program",
                "instructions": [{{
                    "name": "set_value",
                    "args": [{{ "name": "value", "type": "{ty}" }}]
                }}]
            }}"#
        );

        let report = decode_instruction_words_with_idl(&idl, "program", &[0, malformed_word], &[]);

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            continue;
        };
        assert_eq!(report.decode_error.as_deref(), Some(expected_error));
        assert_eq!(report.remaining_words, vec![malformed_word]);
    }
}

#[test]
fn decode_instruction_words_with_idl_matches_risc0_narrow_unsigned_wire() {
    #[derive(Serialize)]
    enum ReferenceInstruction {
        SetUnsigned(u8, u8, u16, u16, u32),
    }

    let words = risc0_zkvm::serde::to_vec(&ReferenceInstruction::SetUnsigned(
        u8::MIN,
        u8::MAX,
        u16::MIN,
        u16::MAX,
        42,
    ));
    assert!(words.is_ok(), "{words:?}");
    let Ok(words) = words else {
        return;
    };
    let idl = r#"{
        "name": "test_program",
        "instructions": [{
            "name": "set_unsigned",
            "args": [
                { "name": "u8_min", "type": "u8" },
                { "name": "u8_max", "type": "u8" },
                { "name": "u16_min", "type": "u16" },
                { "name": "u16_max", "type": "u16" },
                { "name": "following", "type": "u32" }
            ]
        }]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &words, &[]);

    assert!(report.is_ok(), "{report:?}");
    let Ok(report) = report else {
        return;
    };
    assert_eq!(report.decode_error, None);
    assert_eq!(report.remaining_words, Vec::<u32>::new());
    assert_eq!(
        report
            .args
            .iter()
            .map(|field| field.value.clone())
            .collect::<Vec<_>>(),
        vec![
            u8::MIN.to_string(),
            u8::MAX.to_string(),
            u16::MIN.to_string(),
            u16::MAX.to_string(),
            "42".to_owned(),
        ]
    );
}

#[test]
fn decode_instruction_words_with_idl_rejects_out_of_range_narrow_unsigned_wire() {
    for (ty, malformed_word, expected_error) in [
        ("u8", 0x0000_0100, "u8 value 256 is out of range"),
        ("u16", 0x0001_0000, "u16 value 65536 is out of range"),
    ] {
        let idl = format!(
            r#"{{
                "name": "test_program",
                "instructions": [{{
                    "name": "set_value",
                    "args": [{{ "name": "value", "type": "{ty}" }}]
                }}]
            }}"#
        );

        let report = decode_instruction_words_with_idl(&idl, "program", &[0, malformed_word], &[]);

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            continue;
        };
        assert_eq!(report.decode_error.as_deref(), Some(expected_error));
        assert_eq!(report.remaining_words, vec![malformed_word]);
    }
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
fn decode_instruction_words_with_idl_rejects_external_type_without_variant_map() {
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

    assert!(report.is_err(), "{report:?}");
    assert!(report.err().is_some_and(|error| {
        error
            .to_string()
            .contains("instruction `set_value` must declare a u32 variant_index")
    }));
}

#[test]
fn decode_instruction_words_with_idl_accepts_structured_external_type_with_variant_map() {
    let idl = r#"{
        "name": "test_program",
        "instruction_type": { "defined": "Instruction" },
        "instructions": [
            { "name": "set_value", "variant_index": 0, "args": [] }
        ]
    }"#;

    let report = decode_instruction_words_with_idl(idl, "program", &[0], &[]);

    assert!(report.is_ok(), "{report:?}");
    assert_eq!(
        report.ok().map(|report| report.instruction),
        Some("set_value".to_owned())
    );
}

#[test]
fn decode_instruction_words_with_idl_uses_explicit_external_variant_indices() {
    #[derive(Serialize)]
    enum ReferenceInstruction {
        Transfer,
        NewFungibleDefinition,
        NewDefinitionWithMetadata,
        InitializeAccount,
        Burn,
        Mint,
        MintWithAuthority,
        PrintNft,
        SetAuthority(Option<AccountId>),
        SetAuthorityWithAuthority(Option<AccountId>),
    }

    let prefix = [
        ReferenceInstruction::Transfer,
        ReferenceInstruction::NewFungibleDefinition,
        ReferenceInstruction::NewDefinitionWithMetadata,
        ReferenceInstruction::InitializeAccount,
        ReferenceInstruction::Burn,
        ReferenceInstruction::Mint,
        ReferenceInstruction::MintWithAuthority,
    ];
    for (expected, instruction) in prefix.into_iter().enumerate() {
        let words = risc0_zkvm::serde::to_vec(&instruction);
        assert!(words.is_ok(), "{words:?}");
        assert_eq!(
            words.ok().and_then(|words| words.first().copied()),
            Some(expected as u32)
        );
    }

    let idl = r#"{
        "name": "token",
        "instruction_type": "token_core::Instruction",
        "instructions": [
            { "name": "transfer", "variant_index": 0, "args": [] },
            { "name": "new_fungible_definition", "variant_index": 1, "args": [] },
            { "name": "new_definition_with_metadata", "variant_index": 2, "args": [] },
            { "name": "initialize_account", "variant_index": 3, "args": [] },
            { "name": "burn", "variant_index": 4, "args": [] },
            { "name": "mint", "variant_index": 5, "args": [] },
            { "name": "mint_with_authority", "variant_index": 6, "args": [] },
            {
                "name": "set_authority",
                "variant_index": 8,
                "args": [{ "name": "new_authority", "type": { "option": "account_id" } }]
            },
            {
                "name": "set_authority_with_authority",
                "variant_index": 9,
                "args": [{ "name": "new_authority", "type": { "option": "account_id" } }]
            },
            { "name": "print_nft", "variant_index": 7, "args": [] }
        ]
    }"#;
    let cases = [
        ("print_nft", ReferenceInstruction::PrintNft),
        ("set_authority", ReferenceInstruction::SetAuthority(None)),
        (
            "set_authority_with_authority",
            ReferenceInstruction::SetAuthorityWithAuthority(None),
        ),
    ];
    for (expected_name, instruction) in cases {
        let words = risc0_zkvm::serde::to_vec(&instruction);
        assert!(words.is_ok(), "{words:?}");
        let Ok(words) = words else {
            continue;
        };
        let report = decode_instruction_words_with_idl(idl, "program", &words, &[]);
        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            continue;
        };
        assert_eq!(report.instruction, expected_name);
        assert_eq!(report.decode_error, None);
        assert_eq!(report.remaining_words, Vec::<u32>::new());
    }
}
