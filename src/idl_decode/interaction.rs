use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use ::wallet::{AccountIdentity, WalletCore};
use anyhow::{Context as _, Result, bail};
use lee::{AccountId, program::Program};
use lee_core::program::ProgramId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::{
    LOCAL_WALLET_HOME_ENV, normalize_program_id_hex, parse_account_id, wallet::unix_time_text,
};

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
    pub accounts: BTreeMap<String, String>,
    #[serde(default)]
    pub args: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalWalletInstructionReport {
    pub source: String,
    pub status: String,
    pub mode: String,
    pub instruction: String,
    pub program_id_hex: String,
    pub command: String,
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

#[derive(Debug, Clone)]
struct PreparedInstruction {
    instruction: String,
    program_id: ProgramId,
    program_id_hex: String,
    program_binary: String,
    program_binary_required: bool,
    mode: InstructionMode,
    accounts: Vec<PreparedAccount>,
    args: Vec<ResolvedInstructionArg>,
    instruction_words: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstructionMode {
    Public,
    Private,
}

impl InstructionMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

#[derive(Debug, Clone)]
struct PreparedAccount {
    name: String,
    account_id: AccountId,
    privacy: AccountPrivacy,
    signer: bool,
    rest: bool,
    pda: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountPrivacy {
    Public,
    Private,
}

impl AccountPrivacy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedValue {
    report_value: String,
    dynamic: DynamicValue,
    seed_bytes: Option<[u8; 32]>,
}

#[derive(Debug, Clone)]
enum DynamicValue {
    Bool(bool),
    U8(u8),
    U32(u32),
    U64(u64),
    U128(u128),
    Str(String),
    Tuple(Vec<DynamicValue>),
    Seq(Vec<DynamicValue>),
    None,
    Some(Box<DynamicValue>),
}

impl serde::Serialize for DynamicValue {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::U8(value) => serializer.serialize_u8(*value),
            Self::U32(value) => serializer.serialize_u32(*value),
            Self::U64(value) => serializer.serialize_u64(*value),
            Self::U128(value) => serializer.serialize_u128(*value),
            Self::Str(value) => serializer.serialize_str(value),
            Self::Tuple(items) => {
                use serde::ser::SerializeTuple as _;
                let mut tuple = serializer.serialize_tuple(items.len())?;
                for item in items {
                    tuple.serialize_element(item)?;
                }
                tuple.end()
            }
            Self::Seq(items) => {
                use serde::ser::SerializeSeq as _;
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::None => serializer.serialize_none(),
            Self::Some(value) => serializer.serialize_some(value.as_ref()),
        }
    }
}

struct InstructionData<'a> {
    variant_index: u32,
    fields: &'a [DynamicValue],
}

impl serde::Serialize for InstructionData<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeTupleVariant as _;

        let mut variant =
            serializer.serialize_tuple_variant("", self.variant_index, "", self.fields.len())?;
        for field in self.fields {
            variant.serialize_field(field)?;
        }
        variant.end()
    }
}

pub fn local_wallet_instruction_preview(request: Value) -> Result<LocalWalletInstructionReport> {
    let request: LocalWalletInstructionRequest =
        serde_json::from_value(request).context("failed to parse IDL instruction request")?;
    let prepared = prepare_instruction(&request)?;
    Ok(report_from_prepared(prepared, "previewed", None, None))
}

pub async fn local_wallet_instruction_submit(
    profile: Value,
    request: Value,
) -> Result<LocalWalletInstructionReport> {
    let wallet_home = resolve_wallet_home(profile)?;
    let request: LocalWalletInstructionRequest =
        serde_json::from_value(request).context("failed to parse IDL instruction request")?;
    let prepared = prepare_instruction(&request)?;
    let config_path = wallet_home.join("wallet_config.json");
    let storage_path = wallet_home.join("storage.json");
    let wallet = WalletCore::new_update_chain(config_path, storage_path, None)
        .context("failed to open local wallet state")?;

    let (tx_hash, shared_secret_count) = match prepared.mode {
        InstructionMode::Public => {
            let accounts = prepared
                .accounts
                .iter()
                .map(public_account_identity)
                .collect::<Vec<_>>();
            let tx_hash = wallet
                .send_pub_tx(
                    accounts,
                    prepared.instruction_words.clone(),
                    prepared.program_id,
                )
                .await
                .map_err(|error| anyhow::anyhow!("failed to submit public transaction: {error}"))?;
            (tx_hash.to_string(), None)
        }
        InstructionMode::Private => {
            let program = load_program_with_dependencies(
                Path::new(&prepared.program_binary),
                &request.dependency_binaries,
            )?;
            let accounts = prepared
                .accounts
                .iter()
                .map(private_account_identity)
                .collect::<Vec<_>>();
            let (tx_hash, shared_secrets) = wallet
                .send_privacy_preserving_tx(accounts, prepared.instruction_words.clone(), &program)
                .await
                .map_err(|error| {
                    anyhow::anyhow!("failed to submit privacy-preserving transaction: {error}")
                })?;
            (tx_hash.to_string(), Some(shared_secrets.len()))
        }
    };

    Ok(report_from_prepared(
        prepared,
        "submitted",
        Some(tx_hash),
        shared_secret_count,
    ))
}

fn public_account_identity(account: &PreparedAccount) -> AccountIdentity {
    if account.signer {
        AccountIdentity::Public(account.account_id)
    } else {
        AccountIdentity::PublicNoSign(account.account_id)
    }
}

fn private_account_identity(account: &PreparedAccount) -> AccountIdentity {
    match account.privacy {
        AccountPrivacy::Private => AccountIdentity::PrivateOwned(account.account_id),
        AccountPrivacy::Public if account.signer => AccountIdentity::Public(account.account_id),
        AccountPrivacy::Public => AccountIdentity::PublicNoSign(account.account_id),
    }
}

fn report_from_prepared(
    prepared: PreparedInstruction,
    status: &str,
    tx_hash: Option<String>,
    shared_secret_count: Option<usize>,
) -> LocalWalletInstructionReport {
    let submitted_at = tx_hash.as_ref().map(|_| unix_time_text());
    LocalWalletInstructionReport {
        source: "local_wallet_direct".to_owned(),
        status: status.to_owned(),
        mode: prepared.mode.as_str().to_owned(),
        instruction: prepared.instruction,
        program_id_hex: prepared.program_id_hex,
        command: "wallet direct IDL instruction".to_owned(),
        program_binary_required: prepared.program_binary_required,
        program_binary: prepared.program_binary,
        accounts: prepared
            .accounts
            .into_iter()
            .map(|account| ResolvedInstructionAccount {
                name: account.name,
                account_id: account.account_id.to_string(),
                privacy: account.privacy.as_str().to_owned(),
                signer: account.signer,
                rest: account.rest,
                pda: account.pda,
            })
            .collect(),
        args: prepared.args,
        instruction_words_hex: prepared
            .instruction_words
            .iter()
            .map(|word| format!("{word:08x}"))
            .collect(),
        instruction_words: prepared.instruction_words,
        tx_hash,
        shared_secret_count,
        submitted_at,
    }
}

fn prepare_instruction(request: &LocalWalletInstructionRequest) -> Result<PreparedInstruction> {
    let idl: Value = serde_json::from_str(&request.idl_json).context("failed to parse IDL JSON")?;
    let program_id_hex = normalize_program_id_hex(&request.program_id_hex)?;
    let program_id = program_id_from_hex(&program_id_hex)?;
    let instructions = idl
        .get("instructions")
        .and_then(Value::as_array)
        .context("IDL has no instructions array")?;
    let (variant_index, instruction) = select_instruction(instructions, &request.instruction)?;
    let instruction_name = instruction_name(instruction).to_owned();
    let args = instruction
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut parsed_args = BTreeMap::new();
    let mut report_args = Vec::with_capacity(args.len());
    let mut fields = Vec::with_capacity(args.len());
    for arg in &args {
        let name = arg
            .get("name")
            .and_then(Value::as_str)
            .context("IDL arg missing name")?;
        let ty = arg
            .get("type")
            .with_context(|| format!("IDL arg `{name}` missing type"))?;
        let raw = named_value(&request.args, name)
            .with_context(|| format!("argument `{name}` is required"))?;
        let parsed = parse_typed_value(raw, ty)
            .with_context(|| format!("failed to parse argument `{name}` as {}", type_label(ty)))?;
        report_args.push(ResolvedInstructionArg {
            name: name.to_owned(),
            type_label: type_label(ty),
            value: parsed.report_value.clone(),
        });
        fields.push(parsed.dynamic.clone());
        parsed_args.insert(name.to_owned(), parsed);
    }

    let instruction_words = risc0_zkvm::serde::to_vec(&InstructionData {
        variant_index: variant_index as u32,
        fields: &fields,
    })
    .map_err(|error| anyhow::anyhow!("failed to serialize instruction data: {error}"))?;

    let accounts = resolve_accounts(instruction, request, &program_id, &parsed_args)?;
    let mode = if accounts
        .iter()
        .any(|account| account.privacy == AccountPrivacy::Private)
    {
        InstructionMode::Private
    } else {
        InstructionMode::Public
    };
    let program_binary = request.program_binary.trim().to_owned();
    if mode == InstructionMode::Private {
        if program_binary.is_empty() {
            bail!("private IDL instruction requires a program binary");
        }
        if !Path::new(&program_binary).is_file() {
            bail!("program binary is not reachable");
        }
    }

    Ok(PreparedInstruction {
        instruction: instruction_name,
        program_id,
        program_id_hex,
        program_binary,
        program_binary_required: mode == InstructionMode::Private,
        mode,
        accounts,
        args: report_args,
        instruction_words,
    })
}

fn select_instruction<'a>(instructions: &'a [Value], selected: &str) -> Result<(usize, &'a Value)> {
    let selected = selected.trim();
    if selected.is_empty() {
        bail!("instruction is required");
    }
    instructions
        .iter()
        .enumerate()
        .find(|(_, instruction)| {
            let name = instruction_name(instruction);
            name == selected || kebab_name(name) == selected
        })
        .with_context(|| format!("IDL instruction `{selected}` not found"))
}

fn instruction_name(instruction: &Value) -> &str {
    instruction
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn resolve_accounts(
    instruction: &Value,
    request: &LocalWalletInstructionRequest,
    program_id: &ProgramId,
    parsed_args: &BTreeMap<String, ParsedValue>,
) -> Result<Vec<PreparedAccount>> {
    let idl_accounts = instruction
        .get("accounts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut account_map = BTreeMap::<String, AccountId>::new();
    let mut resolved = Vec::new();

    for account in idl_accounts
        .iter()
        .filter(|account| !account_has_pda(account))
    {
        let name = account
            .get("name")
            .and_then(Value::as_str)
            .context("IDL account missing name")?;
        let signer = account.get("signer").and_then(Value::as_bool) == Some(true);
        let rest = account.get("rest").and_then(Value::as_bool) == Some(true);
        if rest {
            if let Some(raw) = named_value(&request.accounts, name) {
                for (index, item) in raw
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .enumerate()
                {
                    let (account_id, privacy) =
                        parse_account_reference(item).with_context(|| {
                            format!("failed to parse rest account `{name}` entry {index}")
                        })?;
                    account_map.insert(format!("{name}[{index}]"), account_id);
                    resolved.push(PreparedAccount {
                        name: format!("{name}[{index}]"),
                        account_id,
                        privacy,
                        signer,
                        rest: true,
                        pda: false,
                    });
                }
            }
            continue;
        }
        let raw = named_value(&request.accounts, name)
            .with_context(|| format!("account `{name}` is required"))?;
        let (account_id, privacy) = parse_account_reference(raw)
            .with_context(|| format!("failed to parse account `{name}`"))?;
        account_map.insert(name.to_owned(), account_id);
        resolved.push(PreparedAccount {
            name: name.to_owned(),
            account_id,
            privacy,
            signer,
            rest: false,
            pda: false,
        });
    }

    resolve_external_pda_seed_accounts(&idl_accounts, request, &mut account_map)?;

    for account in idl_accounts
        .iter()
        .filter(|account| account_has_pda(account))
    {
        let name = account
            .get("name")
            .and_then(Value::as_str)
            .context("IDL account missing name")?;
        let pda = account
            .get("pda")
            .context("PDA account missing pda field after filter")?;
        if pda.get("private").and_then(Value::as_bool) == Some(true) {
            bail!("private PDA accounts are not supported by direct Inspector interaction");
        }
        let account_id = compute_pda(pda, program_id, &account_map, parsed_args)
            .with_context(|| format!("failed to compute PDA `{name}`"))?;
        account_map.insert(name.to_owned(), account_id);
        resolved.push(PreparedAccount {
            name: name.to_owned(),
            account_id,
            privacy: AccountPrivacy::Public,
            signer: account.get("signer").and_then(Value::as_bool) == Some(true),
            rest: false,
            pda: true,
        });
    }

    Ok(resolved)
}

fn resolve_external_pda_seed_accounts(
    idl_accounts: &[Value],
    request: &LocalWalletInstructionRequest,
    account_map: &mut BTreeMap<String, AccountId>,
) -> Result<()> {
    for account in idl_accounts {
        let Some(pda) = account.get("pda") else {
            continue;
        };
        for seed in pda
            .get("seeds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if seed.get("kind").and_then(Value::as_str) != Some("account") {
                continue;
            }
            let path = seed
                .get("path")
                .and_then(Value::as_str)
                .context("account PDA seed missing path")?;
            if account_map.contains_key(path) {
                continue;
            }
            let raw = named_value(&request.accounts, path)
                .with_context(|| format!("PDA seed account `{path}` is required"))?;
            let (account_id, _) = parse_account_reference(raw)
                .with_context(|| format!("failed to parse PDA seed account `{path}`"))?;
            account_map.insert(path.to_owned(), account_id);
        }
    }
    Ok(())
}

fn account_has_pda(account: &Value) -> bool {
    account.get("pda").is_some()
}

fn compute_pda(
    pda: &Value,
    program_id: &ProgramId,
    account_map: &BTreeMap<String, AccountId>,
    parsed_args: &BTreeMap<String, ParsedValue>,
) -> Result<AccountId> {
    let seeds = pda
        .get("seeds")
        .and_then(Value::as_array)
        .context("PDA has no seeds array")?;
    if seeds.is_empty() {
        bail!("PDA requires at least one seed");
    }
    let resolved = seeds
        .iter()
        .map(|seed| resolve_seed(seed, account_map, parsed_args))
        .collect::<Result<Vec<_>>>()?;
    let combined = if resolved.len() == 1 {
        *resolved
            .first()
            .context("PDA seeds unexpectedly empty after length check")?
    } else {
        let mut hasher = Sha256::new();
        for seed in &resolved {
            hasher.update(seed);
        }
        hasher.finalize().into()
    };
    Ok(AccountId::for_public_pda(
        program_id,
        &lee_core::program::PdaSeed::new(combined),
    ))
}

fn resolve_seed(
    seed: &Value,
    account_map: &BTreeMap<String, AccountId>,
    parsed_args: &BTreeMap<String, ParsedValue>,
) -> Result<[u8; 32]> {
    match seed.get("kind").and_then(Value::as_str) {
        Some("const") => {
            let value = seed
                .get("value")
                .and_then(Value::as_str)
                .context("const PDA seed missing value")?;
            let bytes = value.as_bytes();
            if bytes.len() > 32 {
                bail!("const PDA seed `{value}` exceeds 32 bytes");
            }
            let mut seed = [0_u8; 32];
            let Some(seed_prefix) = seed.get_mut(..bytes.len()) else {
                bail!("const PDA seed `{value}` exceeds 32 bytes");
            };
            seed_prefix.copy_from_slice(bytes);
            Ok(seed)
        }
        Some("account") => {
            let path = seed
                .get("path")
                .and_then(Value::as_str)
                .context("account PDA seed missing path")?;
            account_map
                .get(path)
                .map(|account_id| *account_id.value())
                .with_context(|| format!("PDA seed account `{path}` is not resolved"))
        }
        Some("arg") => {
            let path = seed
                .get("path")
                .and_then(Value::as_str)
                .context("arg PDA seed missing path")?;
            parsed_args
                .get(path)
                .and_then(|value| value.seed_bytes)
                .with_context(|| format!("PDA seed arg `{path}` cannot be converted to 32 bytes"))
        }
        Some(kind) => bail!("unsupported PDA seed kind `{kind}`"),
        None => bail!("PDA seed missing kind"),
    }
}

fn parse_account_reference(value: &str) -> Result<(AccountId, AccountPrivacy)> {
    let value = value.trim();
    let (trimmed, privacy) = if let Some(value) = value
        .strip_prefix("Private/")
        .or_else(|| value.strip_prefix("private/"))
    {
        (value.trim(), AccountPrivacy::Private)
    } else if let Some(value) = value
        .strip_prefix("Public/")
        .or_else(|| value.strip_prefix("public/"))
    {
        (value.trim(), AccountPrivacy::Public)
    } else {
        (value, AccountPrivacy::Public)
    };
    Ok((parse_account_id(trimmed)?, privacy))
}

fn parse_typed_value(raw: &str, ty: &Value) -> Result<ParsedValue> {
    if let Some(primitive) = ty.as_str() {
        return parse_primitive(raw, primitive);
    }
    if let Some(array) = ty.get("array") {
        return parse_array(raw, array);
    }
    if let Some(vec) = ty.get("vec") {
        return parse_vec(raw, vec);
    }
    if let Some(option) = ty.get("option") {
        if raw.trim().is_empty() || matches!(raw.trim(), "none" | "null") {
            return Ok(ParsedValue {
                report_value: "None".to_owned(),
                dynamic: DynamicValue::None,
                seed_bytes: None,
            });
        }
        let parsed = parse_typed_value(raw, option)?;
        return Ok(ParsedValue {
            report_value: format!("Some({})", parsed.report_value),
            dynamic: DynamicValue::Some(Box::new(parsed.dynamic)),
            seed_bytes: parsed.seed_bytes,
        });
    }
    if let Some(defined) = ty.get("defined").and_then(Value::as_str) {
        bail!("defined IDL arg type `{defined}` is not supported for direct interaction");
    }
    bail!("unsupported IDL arg type `{}`", ty);
}

fn parse_primitive(raw: &str, primitive: &str) -> Result<ParsedValue> {
    let raw = raw.trim();
    match primitive {
        "bool" => {
            let value = match raw {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" => false,
                _ => bail!("invalid bool `{raw}`"),
            };
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::Bool(value),
                None,
            ))
        }
        "u8" => {
            let value = raw.parse::<u8>().context("invalid u8")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::U8(value),
                None,
            ))
        }
        "u32" => {
            let value = raw.parse::<u32>().context("invalid u32")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::U32(value),
                None,
            ))
        }
        "u64" => {
            let value = raw.parse::<u64>().context("invalid u64")?;
            let mut seed = [0_u8; 32];
            seed[24..32].copy_from_slice(&value.to_be_bytes());
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::U64(value),
                Some(seed),
            ))
        }
        "u128" => {
            let value = raw.parse::<u128>().context("invalid u128")?;
            let mut seed = [0_u8; 32];
            seed[16..32].copy_from_slice(&value.to_be_bytes());
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::U128(value),
                Some(seed),
            ))
        }
        "string" | "String" => {
            let mut seed = [0_u8; 32];
            let bytes = raw.as_bytes();
            let seed_bytes = if bytes.len() <= 32 {
                let Some(seed_prefix) = seed.get_mut(..bytes.len()) else {
                    bail!("string seed exceeds 32 bytes");
                };
                seed_prefix.copy_from_slice(bytes);
                Some(seed)
            } else {
                None
            };
            Ok(parsed_value(
                raw.to_owned(),
                DynamicValue::Str(raw.to_owned()),
                seed_bytes,
            ))
        }
        "program_id" => {
            let program_id_hex = normalize_program_id_hex(raw)?;
            let program_id = program_id_from_hex(&program_id_hex)?;
            Ok(parsed_value(
                program_id_hex,
                DynamicValue::Tuple(program_id.iter().copied().map(DynamicValue::U32).collect()),
                None,
            ))
        }
        other => bail!("unsupported primitive IDL arg type `{other}`"),
    }
}

fn parse_array(raw: &str, array: &Value) -> Result<ParsedValue> {
    let items = array.as_array().context("IDL array type is not an array")?;
    let elem = items.first().context("IDL array type missing element")?;
    let size = items
        .get(1)
        .and_then(Value::as_u64)
        .context("IDL array type missing size")? as usize;
    match elem.as_str() {
        Some("u8") => parse_u8_array(raw, size),
        Some("u32") => parse_u32_array(raw, size),
        _ => bail!("unsupported array IDL arg type `{}`", array),
    }
}

fn parse_u8_array(raw: &str, size: usize) -> Result<ParsedValue> {
    let raw = raw.trim();
    let bytes = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        hex::decode(hex).context("invalid hex bytes")?
    } else if raw.len() == size * 2 && raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        hex::decode(raw).context("invalid hex bytes")?
    } else {
        let mut bytes = vec![0_u8; size];
        let raw_bytes = raw.as_bytes();
        if raw_bytes.len() > size {
            bail!("string is {} bytes, max {size}", raw_bytes.len());
        }
        let Some(bytes_prefix) = bytes.get_mut(..raw_bytes.len()) else {
            bail!("string is {} bytes, max {size}", raw_bytes.len());
        };
        bytes_prefix.copy_from_slice(raw_bytes);
        bytes
    };
    if bytes.len() != size {
        bail!("expected {size} bytes, got {}", bytes.len());
    }
    let seed_bytes = if size == 32 {
        let mut seed = [0_u8; 32];
        seed.copy_from_slice(&bytes);
        Some(seed)
    } else {
        None
    };
    Ok(parsed_value(
        format!("0x{}", hex::encode(&bytes)),
        DynamicValue::Tuple(bytes.into_iter().map(DynamicValue::U8).collect()),
        seed_bytes,
    ))
}

fn parse_u32_array(raw: &str, size: usize) -> Result<ParsedValue> {
    let parts = raw.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != size {
        bail!("expected {size} u32 values, got {}", parts.len());
    }
    let mut values = Vec::with_capacity(size);
    for part in parts {
        values.push(part.parse::<u32>().context("invalid u32 array item")?);
    }
    Ok(parsed_value(
        format!(
            "[{}]",
            values
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        DynamicValue::Tuple(values.into_iter().map(DynamicValue::U32).collect()),
        None,
    ))
}

fn parse_vec(raw: &str, elem: &Value) -> Result<ParsedValue> {
    match elem.as_str() {
        Some("u8") => {
            let values = if raw.trim().is_empty() {
                Vec::new()
            } else {
                raw.split(',')
                    .map(str::trim)
                    .map(|item| item.parse::<u8>().context("invalid u8 vector item"))
                    .collect::<Result<Vec<_>>>()?
            };
            Ok(parsed_value(
                format!("{} bytes", values.len()),
                DynamicValue::Seq(values.into_iter().map(DynamicValue::U8).collect()),
                None,
            ))
        }
        Some("u32") => {
            let values = if raw.trim().is_empty() {
                Vec::new()
            } else {
                raw.split(',')
                    .map(str::trim)
                    .map(|item| item.parse::<u32>().context("invalid u32 vector item"))
                    .collect::<Result<Vec<_>>>()?
            };
            Ok(parsed_value(
                format!("{} words", values.len()),
                DynamicValue::Seq(values.into_iter().map(DynamicValue::U32).collect()),
                None,
            ))
        }
        _ if elem.get("array").is_some() => {
            let array = elem.get("array").context("checked array type")?;
            let items = array.as_array().context("IDL array type is not an array")?;
            let array_elem = items.first().context("IDL array type missing element")?;
            let size = items
                .get(1)
                .and_then(Value::as_u64)
                .context("IDL array type missing size")? as usize;
            if array_elem.as_str() != Some("u8") {
                bail!("unsupported vector element type `{elem}`");
            }
            let values = if raw.trim().is_empty() {
                Vec::new()
            } else {
                raw.split(',')
                    .map(str::trim)
                    .map(|item| parse_u8_array(item, size).map(|parsed| parsed.dynamic))
                    .collect::<Result<Vec<_>>>()?
            };
            Ok(parsed_value(
                format!("{} items", values.len()),
                DynamicValue::Seq(values),
                None,
            ))
        }
        _ => bail!("unsupported vector IDL arg type `{elem}`"),
    }
}

fn parsed_value(
    report_value: String,
    dynamic: DynamicValue,
    seed_bytes: Option<[u8; 32]>,
) -> ParsedValue {
    ParsedValue {
        report_value,
        dynamic,
        seed_bytes,
    }
}

fn type_label(ty: &Value) -> String {
    match ty {
        Value::String(value) => value.clone(),
        Value::Object(object) if object.contains_key("array") => {
            let Some(items) = object.get("array").and_then(Value::as_array) else {
                return ty.to_string();
            };
            let elem = items
                .first()
                .map(type_label)
                .unwrap_or_else(|| "?".to_owned());
            let size = items
                .get(1)
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_owned());
            format!("[{elem}; {size}]")
        }
        Value::Object(object) if object.contains_key("vec") => {
            let elem = object
                .get("vec")
                .map(type_label)
                .unwrap_or_else(|| "?".to_owned());
            format!("Vec<{elem}>")
        }
        Value::Object(object) if object.contains_key("option") => {
            let elem = object
                .get("option")
                .map(type_label)
                .unwrap_or_else(|| "?".to_owned());
            format!("Option<{elem}>")
        }
        Value::Object(object) if object.contains_key("defined") => object
            .get("defined")
            .and_then(Value::as_str)
            .unwrap_or("defined")
            .to_owned(),
        _ => ty.to_string(),
    }
}

fn named_value<'a>(values: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    values
        .get(name)
        .or_else(|| values.get(&kebab_name(name)))
        .map(String::as_str)
}

fn kebab_name(name: &str) -> String {
    name.replace('_', "-")
}

fn program_id_from_hex(value: &str) -> Result<ProgramId> {
    let bytes = hex::decode(value).context("invalid program id hex")?;
    if bytes.len() != 32 {
        bail!("program id hex must be 32 bytes");
    }
    let mut program_id = [0_u32; 8];
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let word = u32::from_le_bytes(
            chunk
                .try_into()
                .map_err(|_| anyhow::anyhow!("program id chunk must be 4 bytes"))?,
        );
        if let Some(slot) = program_id.get_mut(index) {
            *slot = word;
        }
    }
    Ok(program_id)
}

fn load_program_with_dependencies(
    program_path: &Path,
    dependency_paths: &[String],
) -> Result<lee::privacy_preserving_transaction::circuit::ProgramWithDependencies> {
    let program = load_program(program_path)?;
    let mut dependencies = HashMap::new();
    for path in dependency_paths {
        let dependency = load_program(Path::new(path))?;
        dependencies.insert(dependency.id(), dependency);
    }
    Ok(
        lee::privacy_preserving_transaction::circuit::ProgramWithDependencies::new(
            program,
            dependencies,
        ),
    )
}

fn load_program(path: &Path) -> Result<Program> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read program binary at {}", path.display()))?;
    Program::new(bytes.into())
        .map_err(|error| anyhow::anyhow!("failed to parse program binary: {error:?}"))
}

fn resolve_wallet_home(profile: Value) -> Result<PathBuf> {
    #[derive(Deserialize)]
    struct Profile {
        #[serde(default, alias = "walletHome")]
        wallet_home: String,
    }

    let profile: Profile =
        serde_json::from_value(profile).context("failed to parse local wallet profile")?;
    let explicit = profile.wallet_home.trim();
    let home = if explicit.is_empty() {
        std::env::var(LOCAL_WALLET_HOME_ENV).unwrap_or_default()
    } else {
        explicit.to_owned()
    };
    let home = home.trim();
    if home.is_empty() {
        bail!("wallet home directory is required to send IDL instruction");
    }
    let home = PathBuf::from(home);
    if !home.is_dir() {
        bail!("wallet home directory is not reachable");
    }
    if !home.join("wallet_config.json").is_file() {
        bail!("wallet home missing wallet_config.json");
    }
    if !home.join("storage.json").is_file() {
        bail!("wallet home missing storage.json");
    }
    Ok(home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
