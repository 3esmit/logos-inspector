use std::{
    cmp::Ordering,
    env, fs,
    path::PathBuf,
    process::{self, Command},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use borsh::{BorshDeserialize, BorshSerialize};
use common::{block::Block, transaction::NSSATransaction};
use k256::ecdsa::signature::hazmat::PrehashVerifier as _;
use nssa::{
    Account, AccountId, CLOCK_01_PROGRAM_ACCOUNT_ID, PrivateKey, ProgramDeploymentTransaction,
    PublicKey, PublicTransaction, Signature,
    program::Program,
    program_deployment_transaction::Message,
    public_transaction::{Message as PublicMessage, WitnessSet},
};
use nssa_core::program::{PdaSeed, ProgramId};
use rand::{RngCore as _, rngs::OsRng};
use risc0_binfmt::{ProgramBinary, compute_image_id};
use sequencer_service_rpc::{RpcClient as _, SequencerClientBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

const CONFIG_PDA_SEED: &[u8] = b"CONFIG";
const LIQUIDITY_TOKEN_PDA_SEED: &[u8] = b"LIQUIDITY_TOKEN";
const LP_LOCK_HOLDING_PDA_SEED: &[u8] = b"LP_LOCK_HOLDING";
const CURRENT_TICK_ACCOUNT_PDA_SEED: &[u8] = b"CURRENT_TICK_ACCOUNT";

#[derive(Serialize)]
#[allow(dead_code)]
enum CurrentAmmInstruction {
    Initialize {
        token_program_id: ProgramId,
        twap_oracle_program_id: ProgramId,
        authority: AccountId,
    },
    UpdateConfig {
        token_program_id: Option<ProgramId>,
        twap_oracle_program_id: Option<ProgramId>,
        new_authority: Option<AccountId>,
    },
    CreatePriceObservations {
        window_duration: u64,
    },
    CreateOraclePriceAccount {
        window_duration: u64,
    },
    NewDefinition {
        token_a_amount: u128,
        token_b_amount: u128,
        fees: u128,
        deadline: u64,
    },
    AddLiquidity {
        min_amount_liquidity: u128,
        max_amount_to_add_token_a: u128,
        max_amount_to_add_token_b: u128,
        deadline: u64,
    },
    RemoveLiquidity {
        remove_liquidity_amount: u128,
        min_amount_to_remove_token_a: u128,
        min_amount_to_remove_token_b: u128,
        deadline: u64,
    },
    SwapExactInput {
        swap_amount_in: u128,
        min_amount_out: u128,
        token_definition_id_in: AccountId,
        deadline: u64,
    },
    SwapExactOutput {
        exact_amount_out: u128,
        max_amount_in: u128,
        token_definition_id_in: AccountId,
        deadline: u64,
    },
    SyncReserves,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
struct CurrentAmmConfig {
    token_program_id: ProgramId,
    twap_oracle_program_id: ProgramId,
    authority: AccountId,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
struct CurrentPoolDefinition {
    definition_token_a_id: AccountId,
    definition_token_b_id: AccountId,
    vault_a_id: AccountId,
    vault_b_id: AccountId,
    liquidity_pool_id: AccountId,
    liquidity_pool_supply: u128,
    reserve_a: u128,
    reserve_b: u128,
    fees: u128,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
struct CurrentTickAccount {
    tick: i32,
    last_updated: u64,
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let Some(cmd) = args.get(1).map(String::as_str) else {
        usage();
    };

    match cmd {
        "hash-deploy" => {
            let Some(path) = args.get(2) else {
                usage();
            };
            let bytecode = fs::read(path).unwrap_or_else(|err| {
                eprintln!("failed to read {path}: {err}");
                process::exit(1);
            });
            let tx = ProgramDeploymentTransaction::new(Message::new(bytecode.clone()));
            let program = Program::new(bytecode).unwrap_or_else(|err| {
                eprintln!("failed to parse program: {err:?}");
                process::exit(1);
            });
            println!("program_id_hex={}", program_id_hex(program.id()));
            println!("tx_hash={}", hex::encode(tx.hash()));
        }
        "decode-block" => {
            let Some(path) = args.get(2) else {
                usage();
            };
            let raw = fs::read_to_string(path).unwrap_or_else(|err| {
                eprintln!("failed to read {path}: {err}");
                process::exit(1);
            });
            let json: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|err| {
                eprintln!("failed to parse json: {err}");
                process::exit(1);
            });
            let encoded = json
                .get("result")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| {
                    eprintln!("json result is not a base64 string");
                    process::exit(1);
                });
            let bytes = STANDARD.decode(encoded.as_bytes()).unwrap_or_else(|err| {
                eprintln!("failed to decode base64: {err}");
                process::exit(1);
            });
            decode_block_bytes(&bytes);
        }
        "decode-block-range" => {
            let Some(path) = args.get(2) else {
                usage();
            };
            let raw = fs::read_to_string(path).unwrap_or_else(|err| {
                eprintln!("failed to read {path}: {err}");
                process::exit(1);
            });
            let json: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|err| {
                eprintln!("failed to parse json: {err}");
                process::exit(1);
            });
            let blocks = json
                .get("result")
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| {
                    eprintln!("json result is not a block array");
                    process::exit(1);
                });
            for encoded in blocks {
                let encoded = encoded.as_str().unwrap_or_else(|| {
                    eprintln!("block array item is not a base64 string");
                    process::exit(1);
                });
                let bytes = STANDARD.decode(encoded.as_bytes()).unwrap_or_else(|err| {
                    eprintln!("failed to decode base64: {err}");
                    process::exit(1);
                });
                decode_block_bytes(&bytes);
            }
        }
        "fetch-tx" => run_fetch_tx(&args),
        "find-tx-block" => run_find_tx_block(&args),
        "account-json" => run_account_query(&args, AccountQueryMode::Json),
        "account-data-hex" => run_account_query(&args, AccountQueryMode::DataHex),
        "program-id" => run_program_id(&args),
        "create-public-account" => run_create_public_account(&args),
        "token-account-json" => run_token_account_query(&args),
        "amm-instruction-words" => run_amm_instruction_words(&args),
        "token-new-public" => run_token_new_public(&args),
        "amm-account-json" => run_amm_account_query(&args),
        "amm-init-public" => run_amm_init_public(&args),
        "amm-init-validate-local" => run_amm_init_validate_local(&args),
        "legacy-amm-swap-validate" => run_legacy_amm_swap_validate(&args),
        "amm-new-definition-public" => run_amm_new_definition_public(&args),
        "strip-r0bf" => run_strip_r0bf(&args),
        _ => usage(),
    }
}

fn run_find_tx_block(args: &[String]) -> ! {
    if args.len() != 5 && args.len() != 6 {
        usage();
    }

    let tx_hash = args[2].clone();
    let start = args[3].parse().unwrap_or_else(|err| {
        eprintln!("invalid start block: {err}");
        process::exit(1);
    });
    let end = args[4].parse().unwrap_or_else(|err| {
        eprintln!("invalid end block: {err}");
        process::exit(1);
    });
    let sequencer_url = args
        .get(5)
        .map(String::as_str)
        .unwrap_or("https://testnet.lez.logos.co/");

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let client = SequencerClientBuilder::default()
            .build(sequencer_url)
            .unwrap_or_else(|err| {
                eprintln!("failed to build sequencer client: {err}");
                process::exit(1);
            });
        let blocks = client
            .get_block_range(start, end)
            .await
            .unwrap_or_else(|err| {
                eprintln!("failed to fetch block range {start}..={end}: {err}");
                process::exit(1);
            });

        for block in blocks {
            for (index, tx) in block.body.transactions.iter().enumerate() {
                if tx.hash().to_string() == tx_hash {
                    println!("block_id={}", block.header.block_id);
                    println!("timestamp={}", block.header.timestamp);
                    println!("tx_index={index}");
                    println!("tx_hash={}", tx.hash());
                    match tx {
                        NSSATransaction::ProgramDeployment(tx) => {
                            println!("kind=ProgramDeployment");
                            let bytecode = tx.clone().into_message().into_bytecode();
                            let program = Program::new(bytecode).unwrap_or_else(|err| {
                                eprintln!("failed to parse deployed program: {err:?}");
                                process::exit(1);
                            });
                            println!("program_id_hex={}", program_id_hex(program.id()));
                        }
                        NSSATransaction::Public(tx) => {
                            println!("kind=Public");
                            println!("program_id_hex={}", program_id_hex(tx.message().program_id));
                        }
                        NSSATransaction::PrivacyPreserving(_) => {
                            println!("kind=PrivacyPreserving");
                        }
                    }
                    process::exit(0);
                }
            }
        }

        println!("tx=null");
        process::exit(1);
    })
}

fn decode_block_bytes(bytes: &[u8]) {
    match borsh::from_slice::<Block>(bytes) {
        Ok(block) => {
            println!("block_id={}", block.header.block_id);
            println!("timestamp={}", block.header.timestamp);
            println!("bedrock_status={:?}", block.bedrock_status);
            print_transactions(&block.body.transactions);
        }
        Err(block_err) => {
            const LEGACY_BLOCK_BODY_OFFSET: usize = 144;
            if bytes.len() < LEGACY_BLOCK_BODY_OFFSET {
                eprintln!("failed to decode block borsh: {block_err}");
                process::exit(1);
            }
            let block_id = u64::from_le_bytes(bytes[0..8].try_into().expect("slice len checked"));
            let timestamp =
                u64::from_le_bytes(bytes[72..80].try_into().expect("slice len checked"));
            let mut cursor = std::io::Cursor::new(&bytes[LEGACY_BLOCK_BODY_OFFSET..]);
            let txs =
                Vec::<NSSATransaction>::deserialize_reader(&mut cursor).unwrap_or_else(|tx_err| {
                    eprintln!("failed to decode block borsh: {block_err}");
                    eprintln!("failed to decode legacy block transactions: {tx_err}");
                    process::exit(1);
                });
            println!("block_id={block_id}");
            println!("timestamp={timestamp}");
            print_transactions(&txs);
            let status_offset = LEGACY_BLOCK_BODY_OFFSET + cursor.position() as usize;
            if let Some(status) = bytes.get(status_offset) {
                let status = match status {
                    0 => "Pending",
                    1 => "Safe",
                    2 => "Finalized",
                    _ => "Unknown",
                };
                println!("bedrock_status={status}");
            }
        }
    }
}

fn print_transactions(txs: &[NSSATransaction]) {
    println!("tx_count={}", txs.len());
    for (index, tx) in txs.iter().enumerate() {
        println!("tx[{index}].hash={}", tx.hash());
        match tx {
            NSSATransaction::ProgramDeployment(tx) => {
                let bytecode = tx.clone().into_message().into_bytecode();
                let program = Program::new(bytecode.clone()).unwrap_or_else(|err| {
                    eprintln!("failed to parse deployed program in tx[{index}]: {err:?}");
                    process::exit(1);
                });
                println!("tx[{index}].kind=ProgramDeployment");
                println!(
                    "tx[{index}].program_id_hex={}",
                    program_id_hex(program.id())
                );
                println!("tx[{index}].bytecode_len={}", bytecode.len());
            }
            NSSATransaction::Public(tx) => {
                println!("tx[{index}].kind=Public");
                println!(
                    "tx[{index}].program_id_hex={}",
                    program_id_hex(tx.message().program_id)
                );
                println!(
                    "tx[{index}].account_ids={}",
                    join_display(&tx.message().account_ids)
                );
                println!("tx[{index}].nonces={}", join_nonces(&tx.message().nonces));
                println!(
                    "tx[{index}].instruction_data={}",
                    join_u32s(&tx.message().instruction_data)
                );
            }
            NSSATransaction::PrivacyPreserving(_) => {
                println!("tx[{index}].kind=PrivacyPreserving");
            }
        }
    }
}

fn join_display<T: ToString>(values: &[T]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn join_nonces(nonces: &[nssa_core::account::Nonce]) -> String {
    nonces
        .iter()
        .map(|nonce| nonce.0.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn join_u32s(values: &[u32]) -> String {
    values
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn run_amm_instruction_words(args: &[String]) -> ! {
    if args.len() != 6 {
        usage();
    }

    let mode = args[2].as_str();
    let amount_0 = parse_u128("amount_0", &args[3]);
    let amount_1 = parse_u128("amount_1", &args[4]);
    let token_definition_id_in = parse_account_id(&args[5]);
    let instruction = match mode {
        "swap-exact-input" => amm_core::Instruction::SwapExactInput {
            swap_amount_in: amount_0,
            min_amount_out: amount_1,
            token_definition_id_in,
        },
        "swap-exact-output" => amm_core::Instruction::SwapExactOutput {
            exact_amount_out: amount_0,
            max_amount_in: amount_1,
            token_definition_id_in,
        },
        _ => usage(),
    };
    let words = Program::serialize_instruction(instruction).unwrap_or_else(|err| {
        eprintln!("failed to serialize AMM instruction: {err:?}");
        process::exit(1);
    });
    println!(
        "{}",
        words
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
    process::exit(0);
}

fn run_fetch_tx(args: &[String]) -> ! {
    if args.len() != 3 && args.len() != 4 {
        usage();
    }

    let tx_hash = args[2].clone();
    let sequencer_url = args
        .get(3)
        .map(String::as_str)
        .unwrap_or("https://testnet.lez.logos.co/");

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let client = SequencerClientBuilder::default()
            .build(sequencer_url)
            .unwrap_or_else(|err| {
                eprintln!("failed to build sequencer client: {err}");
                process::exit(1);
            });
        let parsed_hash = tx_hash.parse().unwrap_or_else(|err| {
            eprintln!("failed to parse tx hash {tx_hash}: {err}");
            process::exit(1);
        });
        let Some(tx) = client
            .get_transaction(parsed_hash)
            .await
            .unwrap_or_else(|err| {
                eprintln!("failed to fetch tx {tx_hash}: {err}");
                process::exit(1);
            })
        else {
            println!("tx=null");
            process::exit(1);
        };

        println!("hash={}", tx.hash());
        match tx {
            NSSATransaction::ProgramDeployment(tx) => {
                println!("kind=ProgramDeployment");
                println!("bytecode_len={}", tx.into_message().into_bytecode().len());
            }
            NSSATransaction::Public(tx) => {
                println!("kind=Public");
                println!("program_id_hex={}", program_id_hex(tx.message().program_id));
                println!("account_ids={}", join_display(&tx.message().account_ids));
                println!("nonces={}", join_nonces(&tx.message().nonces));
                println!(
                    "instruction_data={}",
                    join_u32s(&tx.message().instruction_data)
                );
                println!(
                    "raw_signature_valid={}",
                    tx.witness_set().is_valid_for(tx.message())
                );
                let prehash = public_message_prehash(tx.message());
                println!("message_prehash={}", hex::encode(prehash));
                println!(
                    "prehash_signature_valid={}",
                    prehash_witness_set_is_valid(tx.witness_set(), &prehash)
                );
            }
            NSSATransaction::PrivacyPreserving(_) => {
                println!("kind=PrivacyPreserving");
            }
        }

        process::exit(0);
    })
}

#[derive(Clone, Copy)]
enum AccountQueryMode {
    Json,
    DataHex,
}

fn run_account_query(args: &[String], mode: AccountQueryMode) -> ! {
    if args.len() != 3 && args.len() != 4 {
        usage();
    }

    let account_id = parse_account_id(&args[2]);
    let sequencer_url = args
        .get(3)
        .map(String::as_str)
        .unwrap_or("https://testnet.lez.logos.co/");

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let client = SequencerClientBuilder::default()
            .build(sequencer_url)
            .unwrap_or_else(|err| {
                eprintln!("failed to build sequencer client: {err}");
                process::exit(1);
            });
        let account = client.get_account(account_id).await.unwrap_or_else(|err| {
            eprintln!("failed to fetch account {account_id}: {err}");
            process::exit(1);
        });

        match mode {
            AccountQueryMode::Json => {
                let json = serde_json::to_string_pretty(&account).unwrap_or_else(|err| {
                    eprintln!("failed to serialize account JSON: {err}");
                    process::exit(1);
                });
                println!("{json}");
            }
            AccountQueryMode::DataHex => {
                println!("{}", hex::encode(account.data.into_inner()));
            }
        }

        process::exit(0);
    })
}

fn run_token_account_query(args: &[String]) -> ! {
    if args.len() != 4 && args.len() != 5 {
        usage();
    }

    let token_type = args[2].as_str();
    let account_id = parse_account_id(&args[3]);
    let sequencer_url = args
        .get(4)
        .map(String::as_str)
        .unwrap_or("https://testnet.lez.logos.co/");

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let client = SequencerClientBuilder::default()
            .build(sequencer_url)
            .unwrap_or_else(|err| {
                eprintln!("failed to build sequencer client: {err}");
                process::exit(1);
            });
        let account = client.get_account(account_id).await.unwrap_or_else(|err| {
            eprintln!("failed to fetch account {account_id}: {err}");
            process::exit(1);
        });
        let data = account.data.into_inner();
        let value = match token_type {
            "definition" => {
                let decoded = borsh::from_slice::<token_core::TokenDefinition>(&data)
                    .unwrap_or_else(|err| {
                        eprintln!("failed to decode TokenDefinition: {err}");
                        process::exit(1);
                    });
                serde_json::to_value(decoded)
            }
            "holding" => {
                let decoded =
                    borsh::from_slice::<token_core::TokenHolding>(&data).unwrap_or_else(|err| {
                        eprintln!("failed to decode TokenHolding: {err}");
                        process::exit(1);
                    });
                serde_json::to_value(decoded)
            }
            other => {
                eprintln!("invalid token account type {other:?}; expected definition or holding");
                process::exit(1);
            }
        }
        .unwrap_or_else(|err| {
            eprintln!("failed to convert decoded token account to JSON: {err}");
            process::exit(1);
        });

        let json = serde_json::to_string_pretty(&value).unwrap_or_else(|err| {
            eprintln!("failed to serialize token account JSON: {err}");
            process::exit(1);
        });
        println!("{json}");

        process::exit(0);
    })
}

fn run_amm_account_query(args: &[String]) -> ! {
    if args.len() != 4 && args.len() != 5 {
        usage();
    }

    let account_type = args[2].as_str();
    let account_id = parse_account_id(&args[3]);
    let sequencer_url = args
        .get(4)
        .map(String::as_str)
        .unwrap_or("https://testnet.lez.logos.co/");

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let client = SequencerClientBuilder::default()
            .build(sequencer_url)
            .unwrap_or_else(|err| {
                eprintln!("failed to build sequencer client: {err}");
                process::exit(1);
            });
        let account = client.get_account(account_id).await.unwrap_or_else(|err| {
            eprintln!("failed to fetch account {account_id}: {err}");
            process::exit(1);
        });
        let data = account.data.into_inner();
        let value = match account_type {
            "config" => {
                let decoded = borsh::from_slice::<CurrentAmmConfig>(&data).unwrap_or_else(|err| {
                    eprintln!("failed to decode AmmConfig: {err}");
                    process::exit(1);
                });
                serde_json::to_value(decoded)
            }
            "pool" => {
                let decoded =
                    borsh::from_slice::<CurrentPoolDefinition>(&data).unwrap_or_else(|err| {
                        eprintln!("failed to decode PoolDefinition: {err}");
                        process::exit(1);
                    });
                serde_json::to_value(decoded)
            }
            "current-tick" => {
                let decoded =
                    borsh::from_slice::<CurrentTickAccount>(&data).unwrap_or_else(|err| {
                        eprintln!("failed to decode CurrentTickAccount: {err}");
                        process::exit(1);
                    });
                serde_json::to_value(decoded)
            }
            other => {
                eprintln!(
                    "invalid AMM account type {other:?}; expected config, pool, or current-tick"
                );
                process::exit(1);
            }
        }
        .unwrap_or_else(|err| {
            eprintln!("failed to convert decoded AMM account to JSON: {err}");
            process::exit(1);
        });

        let json = serde_json::to_string_pretty(&value).unwrap_or_else(|err| {
            eprintln!("failed to serialize AMM account JSON: {err}");
            process::exit(1);
        });
        println!("{json}");

        process::exit(0);
    })
}

fn run_program_id(args: &[String]) -> ! {
    if args.len() != 3 {
        usage();
    }

    let program = read_program(&args[2]);
    let program_id = program.id();
    println!("program_id_hex={}", program_id_hex(program_id));
    println!("program_id_base58={}", program_id_base58(program_id));
    process::exit(0);
}

fn run_create_public_account(args: &[String]) -> ! {
    if args.len() != 2 {
        usage();
    }

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let mut wallet = wallet::WalletCore::from_env().unwrap_or_else(|err| {
            eprintln!("failed to load wallet: {err:#}");
            process::exit(1);
        });
        let (account_id, chain_index) = wallet.create_new_account_public(None);
        let private_key = wallet
            .get_account_public_signing_key(account_id)
            .unwrap_or_else(|| {
                eprintln!("generated public account signing key missing");
                process::exit(1);
            });
        let public_key = PublicKey::new_from_private_key(private_key);
        wallet.store_persistent_data().await.unwrap_or_else(|err| {
            eprintln!("failed to store wallet data: {err:#}");
            process::exit(1);
        });
        println!("account=Public/{account_id}");
        println!("account_id={account_id}");
        println!("chain_index={chain_index}");
        println!("public_key={}", hex::encode(public_key.value()));
        process::exit(0);
    })
}

fn run_token_new_public(args: &[String]) -> ! {
    if args.len() != 8 {
        usage();
    }

    let program_path = &args[2];
    let definition_account_id = parse_account_id(&args[3]);
    let holding_account_id = parse_account_id(&args[4]);
    let name = args[5].clone();
    let total_supply = args[6].parse::<u128>().unwrap_or_else(|err| {
        eprintln!("invalid total_supply: {err}");
        process::exit(1);
    });
    let mode = parse_public_tx_mode(&args[7]);

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let wallet = wallet::WalletCore::from_env().unwrap_or_else(|err| {
            eprintln!("failed to load wallet: {err:#}");
            process::exit(1);
        });

        let program = read_program(program_path);
        let program_id = program.id();
        let account_ids = vec![definition_account_id, holding_account_id];
        let instruction = token_core::Instruction::NewFungibleDefinition { name, total_supply };
        submit_public_transaction(
            &wallet,
            program_id,
            account_ids,
            &[definition_account_id, holding_account_id],
            instruction,
            mode,
        )
        .await;

        process::exit(0);
    })
}

fn run_amm_init_public(args: &[String]) -> ! {
    if args.len() != 7 {
        usage();
    }

    let amm_program_path = &args[2];
    let token_program_path = &args[3];
    let twap_oracle_program_path = &args[4];
    let authority = parse_account_id(&args[5]);
    let mode = parse_public_tx_mode(&args[6]);

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let wallet = wallet::WalletCore::from_env().unwrap_or_else(|err| {
            eprintln!("failed to load wallet: {err:#}");
            process::exit(1);
        });

        let amm_program_id = read_program(amm_program_path).id();
        let token_program_id = read_program(token_program_path).id();
        let twap_oracle_program_id = read_program(twap_oracle_program_path).id();
        let config = compute_config_pda(amm_program_id);

        println!("amm_program_id_hex={}", program_id_hex(amm_program_id));
        println!("token_program_id_hex={}", program_id_hex(token_program_id));
        println!(
            "twap_oracle_program_id_hex={}",
            program_id_hex(twap_oracle_program_id)
        );
        println!("config=Public/{config}");
        println!("authority=Public/{authority}");

        let instruction = CurrentAmmInstruction::Initialize {
            token_program_id,
            twap_oracle_program_id,
            authority,
        };
        submit_public_transaction(
            &wallet,
            amm_program_id,
            vec![config, authority],
            &[authority],
            instruction,
            mode,
        )
        .await;

        process::exit(0);
    })
}

fn run_amm_init_validate_local(args: &[String]) -> ! {
    if args.len() != 7 {
        usage();
    }

    let amm_program_path = &args[2];
    let token_program_path = &args[3];
    let twap_oracle_program_path = &args[4];
    let authority = parse_account_id(&args[5]);
    let mode = parse_public_tx_mode(&args[6]);

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let wallet = wallet::WalletCore::from_env().unwrap_or_else(|err| {
            eprintln!("failed to load wallet: {err:#}");
            process::exit(1);
        });

        let amm_program = read_program(amm_program_path);
        let amm_program_id = amm_program.id();
        let token_program_id = read_program(token_program_path).id();
        let twap_oracle_program_id = read_program(twap_oracle_program_path).id();
        let config = compute_config_pda(amm_program_id);

        let nonces = wallet
            .get_accounts_nonces(vec![authority])
            .await
            .unwrap_or_else(|err| {
                eprintln!("failed to fetch authority nonce: {err:#}");
                process::exit(1);
            });
        let private_key = wallet
            .get_account_public_signing_key(authority)
            .unwrap_or_else(|| {
                eprintln!("signing key missing for Public/{authority}");
                process::exit(1);
            });
        verify_key_account_match("signer", private_key, authority);

        let instruction = CurrentAmmInstruction::Initialize {
            token_program_id,
            twap_oracle_program_id,
            authority,
        };
        let message =
            PublicMessage::try_new(amm_program_id, vec![config, authority], nonces, instruction)
                .unwrap_or_else(|err| {
                    eprintln!("failed to build message: {err:?}");
                    process::exit(1);
                });

        let witness_set = if mode.uses_prehash() {
            let message_hash = public_message_prehash(&message);
            prehash_witness_set(&message_hash, &[private_key]).unwrap_or_else(|err| {
                eprintln!("failed to build prehash witness set: {err}");
                process::exit(1);
            })
        } else {
            WitnessSet::for_message(&message, &[private_key])
        };

        let tx = PublicTransaction::new(message, witness_set);
        println!("amm_program_id_hex={}", program_id_hex(amm_program_id));
        println!("config=Public/{config}");
        println!("authority=Public/{authority}");
        println!("tx_hash={}", hex::encode(tx.hash()));
        println!(
            "local_signature_valid={}",
            tx.witness_set().is_valid_for(tx.message())
        );

        let deployment = ProgramDeploymentTransaction::new(Message::new(
            fs::read(amm_program_path).unwrap_or_else(|err| {
                eprintln!("failed to read {amm_program_path}: {err}");
                process::exit(1);
            }),
        ));
        let mut state = nssa::V03State::new_with_genesis_accounts(&[], vec![], 0);
        state
            .transition_from_program_deployment_transaction(&deployment)
            .unwrap_or_else(|err| {
                eprintln!("local AMM deployment failed: {err:#?}");
                process::exit(1);
            });

        match state.transition_from_public_transaction(&tx, 1, 0) {
            Ok(()) => {
                println!("local_validation=ok");
                let account = state.get_account_by_id(config);
                let json = serde_json::to_string_pretty(&account).unwrap_or_else(|err| {
                    eprintln!("failed to serialize local config account: {err}");
                    process::exit(1);
                });
                println!("config_account={json}");
            }
            Err(err) => {
                println!("local_validation=err");
                println!("error={err:#?}");
                process::exit(1);
            }
        }

        process::exit(0);
    })
}

fn run_legacy_amm_swap_validate(args: &[String]) -> ! {
    if args.len() != 13 {
        usage();
    }

    let amm_program_path = &args[2];
    let token_program_path = &args[3];
    let pool = parse_account_id(&args[4]);
    let vault_a = parse_account_id(&args[5]);
    let vault_b = parse_account_id(&args[6]);
    let user_holding_a = parse_account_id(&args[7]);
    let user_holding_b = parse_account_id(&args[8]);
    let amount_in = parse_u128("amount_in", &args[9]);
    let min_out = parse_u128("min_out", &args[10]);
    let token_definition_id_in = parse_account_id(&args[11]);
    let signer = parse_account_id(&args[12]);

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let wallet = wallet::WalletCore::from_env().unwrap_or_else(|err| {
            eprintln!("failed to load wallet: {err:#}");
            process::exit(1);
        });

        let amm_program = read_program(amm_program_path);
        let amm_program_id = amm_program.id();
        let instruction = amm_core::Instruction::SwapExactInput {
            swap_amount_in: amount_in,
            min_amount_out: min_out,
            token_definition_id_in,
        };
        let instruction_data = Program::serialize_instruction(instruction).unwrap_or_else(|err| {
            eprintln!("failed to serialize AMM instruction: {err:?}");
            process::exit(1);
        });
        let account_ids = vec![pool, vault_a, vault_b, user_holding_a, user_holding_b];
        let nonces = wallet
            .get_accounts_nonces(vec![signer])
            .await
            .unwrap_or_else(|err| {
                eprintln!("failed to fetch signer nonce: {err:#}");
                process::exit(1);
            });
        let private_key = wallet
            .get_account_public_signing_key(signer)
            .unwrap_or_else(|| {
                eprintln!("signing key missing for Public/{signer}");
                process::exit(1);
            });
        verify_key_account_match("signer", private_key, signer);

        let message = PublicMessage::new_preserialized(
            amm_program_id,
            account_ids.clone(),
            nonces,
            instruction_data,
        );
        let message_hash = public_message_prehash(&message);
        let witness_set =
            prehash_witness_set(&message_hash, &[private_key]).unwrap_or_else(|err| {
                eprintln!("failed to build prehash witness set: {err}");
                process::exit(1);
            });
        let tx = PublicTransaction::new(message, witness_set);

        println!("amm_program_id_hex={}", program_id_hex(amm_program_id));
        println!("message_prehash={}", hex::encode(message_hash));
        println!("tx_hash={}", hex::encode(tx.hash()));
        println!(
            "prehash_signature_valid={}",
            prehash_witness_set_is_valid(tx.witness_set(), &message_hash)
        );

        let mut state = nssa::V03State::new_with_genesis_accounts(&[], vec![], 0);
        for path in [token_program_path, amm_program_path] {
            let deployment = ProgramDeploymentTransaction::new(Message::new(
                fs::read(path).unwrap_or_else(|err| {
                    eprintln!("failed to read {path}: {err}");
                    process::exit(1);
                }),
            ));
            state
                .transition_from_program_deployment_transaction(&deployment)
                .unwrap_or_else(|err| {
                    eprintln!("local program deployment failed for {path}: {err:#?}");
                    process::exit(1);
                });
        }

        for account_id in account_ids {
            let account: Account =
                wallet
                    .get_account_public(account_id)
                    .await
                    .unwrap_or_else(|err| {
                        eprintln!("failed to fetch Public/{account_id}: {err:#}");
                        process::exit(1);
                    });
            state.force_insert_account(account_id, account);
        }

        match state.transition_from_public_transaction(&tx, 1, 0) {
            Ok(()) => {
                println!("local_validation=ok");
                println!(
                    "pool_post={}",
                    serde_json::to_string_pretty(&state.get_account_by_id(pool)).unwrap()
                );
            }
            Err(err) => {
                println!("local_validation=err");
                println!("error={err:#?}");
                process::exit(1);
            }
        }

        process::exit(0);
    })
}

fn run_amm_new_definition_public(args: &[String]) -> ! {
    if args.len() != 14 {
        usage();
    }

    let amm_program_path = &args[2];
    let twap_oracle_program_path = &args[3];
    let token_a_definition = parse_account_id(&args[4]);
    let token_b_definition = parse_account_id(&args[5]);
    let user_holding_a = parse_account_id(&args[6]);
    let user_holding_b = parse_account_id(&args[7]);
    let user_holding_lp = parse_account_id(&args[8]);
    let token_a_amount = parse_u128("token_a_amount", &args[9]);
    let token_b_amount = parse_u128("token_b_amount", &args[10]);
    let fees = parse_u128("fees", &args[11]);
    let deadline = args[12].parse::<u64>().unwrap_or_else(|err| {
        eprintln!("invalid deadline: {err}");
        process::exit(1);
    });
    let mode = parse_public_tx_mode(&args[13]);

    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|err| {
        eprintln!("failed to create tokio runtime: {err}");
        process::exit(1);
    });

    runtime.block_on(async move {
        let wallet = wallet::WalletCore::from_env().unwrap_or_else(|err| {
            eprintln!("failed to load wallet: {err:#}");
            process::exit(1);
        });

        let amm_program_id = read_program(amm_program_path).id();
        let twap_oracle_program_id = read_program(twap_oracle_program_path).id();
        let config = compute_config_pda(amm_program_id);
        let pool = compute_pool_pda(amm_program_id, token_a_definition, token_b_definition);
        let vault_a = compute_vault_pda(amm_program_id, pool, token_a_definition);
        let vault_b = compute_vault_pda(amm_program_id, pool, token_b_definition);
        let pool_definition_lp = compute_liquidity_token_pda(amm_program_id, pool);
        let lp_lock_holding = compute_lp_lock_holding_pda(amm_program_id, pool);
        let current_tick_account = compute_current_tick_account_pda(twap_oracle_program_id, pool);
        let clock = CLOCK_01_PROGRAM_ACCOUNT_ID;

        println!("amm_program_id_hex={}", program_id_hex(amm_program_id));
        println!(
            "twap_oracle_program_id_hex={}",
            program_id_hex(twap_oracle_program_id)
        );
        println!("config=Public/{config}");
        println!("pool=Public/{pool}");
        println!("vault_a=Public/{vault_a}");
        println!("vault_b=Public/{vault_b}");
        println!("pool_definition_lp=Public/{pool_definition_lp}");
        println!("lp_lock_holding=Public/{lp_lock_holding}");
        println!("current_tick_account=Public/{current_tick_account}");
        println!("clock=Public/{clock}");

        let instruction = CurrentAmmInstruction::NewDefinition {
            token_a_amount,
            token_b_amount,
            fees,
            deadline,
        };
        let account_ids = vec![
            config,
            pool,
            vault_a,
            vault_b,
            pool_definition_lp,
            lp_lock_holding,
            user_holding_a,
            user_holding_b,
            user_holding_lp,
            current_tick_account,
            clock,
        ];
        let signer_ids = [user_holding_a, user_holding_b, user_holding_lp];
        submit_public_transaction(
            &wallet,
            amm_program_id,
            account_ids,
            &signer_ids,
            instruction,
            mode,
        )
        .await;

        process::exit(0);
    })
}

#[derive(Clone, Copy)]
enum PublicTxMode {
    LegacyDryRun,
    LegacySubmit,
    PrehashDryRun,
    PrehashSubmit,
}

impl PublicTxMode {
    const fn uses_prehash(self) -> bool {
        matches!(self, Self::PrehashDryRun | Self::PrehashSubmit)
    }

    const fn should_submit(self) -> bool {
        matches!(self, Self::LegacySubmit | Self::PrehashSubmit)
    }
}

fn parse_public_tx_mode(raw: &str) -> PublicTxMode {
    match raw {
        "dry-run" => PublicTxMode::LegacyDryRun,
        "submit" => PublicTxMode::LegacySubmit,
        "dry-run-prehash" => PublicTxMode::PrehashDryRun,
        "submit-prehash" => PublicTxMode::PrehashSubmit,
        other => {
            eprintln!(
                "invalid mode {other:?}; expected dry-run, submit, dry-run-prehash, or submit-prehash"
            );
            process::exit(1);
        }
    }
}

async fn submit_public_transaction<T: Serialize>(
    wallet: &wallet::WalletCore,
    program_id: ProgramId,
    account_ids: Vec<AccountId>,
    signer_ids: &[AccountId],
    instruction: T,
    mode: PublicTxMode,
) {
    let nonces = wallet
        .get_accounts_nonces(signer_ids.to_vec())
        .await
        .unwrap_or_else(|err| {
            eprintln!("failed to fetch signer nonces: {err:#}");
            process::exit(1);
        });
    let private_keys = signer_ids
        .iter()
        .map(|account_id| {
            let key = wallet
                .get_account_public_signing_key(*account_id)
                .unwrap_or_else(|| {
                    eprintln!("signing key missing for Public/{account_id}");
                    process::exit(1);
                });
            verify_key_account_match("signer", key, *account_id);
            key
        })
        .collect::<Vec<_>>();

    let message = PublicMessage::try_new(program_id, account_ids, nonces, instruction)
        .unwrap_or_else(|err| {
            eprintln!("failed to build message: {err:?}");
            process::exit(1);
        });
    let witness_set = if mode.uses_prehash() {
        let message_hash = public_message_prehash(&message);
        let witness_set = prehash_witness_set(&message_hash, &private_keys).unwrap_or_else(|err| {
            eprintln!("failed to build prehash witness set: {err}");
            process::exit(1);
        });
        println!("signature_scheme=prehash");
        println!("message_prehash={}", hex::encode(message_hash));
        println!(
            "prehash_signature_valid={}",
            prehash_witness_set_is_valid(&witness_set, &message_hash)
        );
        witness_set
    } else {
        println!("signature_scheme=legacy-raw");
        WitnessSet::for_message(&message, &private_keys)
    };
    println!("program_id_hex={}", program_id_hex(program_id));
    println!(
        "local_signature_valid={}",
        witness_set.is_valid_for(&message)
    );
    println!(
        "tx_hash={}",
        hex::encode(PublicTransaction::new(message.clone(), witness_set.clone()).hash())
    );

    if mode.should_submit() {
        let tx = PublicTransaction::new(message, witness_set);
        let hash = wallet
            .sequencer_client
            .send_transaction(NSSATransaction::Public(tx))
            .await
            .unwrap_or_else(|err| {
                eprintln!("submit failed: {err:?}");
                process::exit(1);
            });
        println!("submitted_tx_hash={hash}");
        let poller =
            wallet::poller::TxPoller::new(wallet.config(), wallet.sequencer_client.clone());
        poller.poll_tx(hash).await.unwrap_or_else(|err| {
            eprintln!("poll failed: {err:#}");
            process::exit(1);
        });
        println!("confirmed=true");
    }
}

fn public_message_prehash(message: &PublicMessage) -> [u8; 32] {
    const PREFIX: &[u8; 32] = b"/LEE/v0.3/Message/Public/\x00\x00\x00\x00\x00\x00\x00";

    let message_bytes = borsh::to_vec(message).unwrap_or_else(|err| {
        eprintln!("failed to serialize public message: {err}");
        process::exit(1);
    });
    let mut bytes = Vec::with_capacity(PREFIX.len() + message_bytes.len());
    bytes.extend_from_slice(PREFIX);
    bytes.extend_from_slice(&message_bytes);

    Sha256::digest(bytes).into()
}

fn prehash_witness_set(
    message_hash: &[u8; 32],
    private_keys: &[&PrivateKey],
) -> Result<WitnessSet, String> {
    private_keys
        .iter()
        .map(|&key| {
            let signature = prehash_signature(key, message_hash)?;
            let public_key = PublicKey::new_from_private_key(key);
            Ok((signature, public_key))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(WitnessSet::from_raw_parts)
}

fn prehash_signature(key: &PrivateKey, message_hash: &[u8; 32]) -> Result<Signature, String> {
    let signing_key =
        k256::schnorr::SigningKey::from_bytes(key.value()).map_err(|err| err.to_string())?;
    let mut aux_random = [0_u8; 32];
    OsRng.fill_bytes(&mut aux_random);
    let signature = signing_key
        .sign_prehash_with_aux_rand(message_hash, &aux_random)
        .map_err(|err| err.to_string())?;

    Ok(Signature {
        value: signature.to_bytes(),
    })
}

fn prehash_witness_set_is_valid(witness_set: &WitnessSet, message_hash: &[u8; 32]) -> bool {
    witness_set
        .signatures_and_public_keys()
        .iter()
        .all(|(signature, public_key)| {
            prehash_signature_is_valid(signature, public_key, message_hash)
        })
}

fn prehash_signature_is_valid(
    signature: &Signature,
    public_key: &PublicKey,
    message_hash: &[u8; 32],
) -> bool {
    let Ok(verifying_key) = k256::schnorr::VerifyingKey::from_bytes(public_key.value()) else {
        return false;
    };
    let Ok(signature) = k256::schnorr::Signature::try_from(signature.value.as_slice()) else {
        return false;
    };

    verifying_key
        .verify_prehash(message_hash, &signature)
        .is_ok()
}

fn verify_key_account_match(label: &str, key: &PrivateKey, account_id: AccountId) {
    let public_key = PublicKey::new_from_private_key(key);
    let derived_account_id = AccountId::from(&public_key);
    println!("{label}_derived_account_id={derived_account_id}");
    println!("{label}_account_match={}", derived_account_id == account_id);
}

fn parse_account_id(value: &str) -> AccountId {
    let stripped = value
        .strip_prefix("Public/")
        .or_else(|| value.strip_prefix("Private/"))
        .unwrap_or(value);
    stripped.parse().unwrap_or_else(|err| {
        eprintln!("invalid account id {value}: {err}");
        process::exit(1);
    })
}

fn parse_u128(label: &str, value: &str) -> u128 {
    value.parse::<u128>().unwrap_or_else(|err| {
        eprintln!("invalid {label}: {err}");
        process::exit(1);
    })
}

fn read_program(path: &str) -> Program {
    let bytecode = fs::read(path).unwrap_or_else(|err| {
        eprintln!("failed to read {path}: {err}");
        process::exit(1);
    });
    Program::new(bytecode).unwrap_or_else(|err| {
        eprintln!("failed to parse program {path}: {err:?}");
        process::exit(1);
    })
}

fn program_id_hex(program_id: nssa_core::program::ProgramId) -> String {
    program_id
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn program_id_base58(program_id: ProgramId) -> String {
    AccountId::new(program_id_bytes(program_id)).to_string()
}

fn program_id_bytes(program_id: ProgramId) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (chunk, word) in bytes.chunks_exact_mut(4).zip(program_id.iter()) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn compute_config_pda(amm_program_id: ProgramId) -> AccountId {
    AccountId::for_public_pda(&amm_program_id, &pda_seed_from_bytes(CONFIG_PDA_SEED))
}

fn compute_pool_pda(
    amm_program_id: ProgramId,
    definition_token_a_id: AccountId,
    definition_token_b_id: AccountId,
) -> AccountId {
    AccountId::for_public_pda(
        &amm_program_id,
        &compute_pool_pda_seed(definition_token_a_id, definition_token_b_id),
    )
}

fn compute_pool_pda_seed(
    definition_token_a_id: AccountId,
    definition_token_b_id: AccountId,
) -> PdaSeed {
    let (token_1, token_2) = match definition_token_a_id
        .value()
        .cmp(definition_token_b_id.value())
    {
        Ordering::Less => (definition_token_b_id, definition_token_a_id),
        Ordering::Greater => (definition_token_a_id, definition_token_b_id),
        Ordering::Equal => {
            eprintln!("token definitions must differ");
            process::exit(1);
        }
    };

    let mut bytes = [0_u8; 64];
    let (token_1_bytes, token_2_bytes) = bytes.split_at_mut(32);
    token_1_bytes.copy_from_slice(token_1.value());
    token_2_bytes.copy_from_slice(token_2.value());
    pda_seed_from_bytes(&bytes)
}

fn compute_vault_pda(
    amm_program_id: ProgramId,
    pool_id: AccountId,
    definition_token_id: AccountId,
) -> AccountId {
    AccountId::for_public_pda(
        &amm_program_id,
        &compute_vault_pda_seed(pool_id, definition_token_id),
    )
}

fn compute_vault_pda_seed(pool_id: AccountId, definition_token_id: AccountId) -> PdaSeed {
    let mut bytes = [0_u8; 64];
    let (pool_bytes, definition_bytes) = bytes.split_at_mut(32);
    pool_bytes.copy_from_slice(pool_id.value());
    definition_bytes.copy_from_slice(definition_token_id.value());
    pda_seed_from_bytes(&bytes)
}

fn compute_liquidity_token_pda(amm_program_id: ProgramId, pool_id: AccountId) -> AccountId {
    AccountId::for_public_pda(&amm_program_id, &compute_liquidity_token_pda_seed(pool_id))
}

fn compute_liquidity_token_pda_seed(pool_id: AccountId) -> PdaSeed {
    let mut bytes = Vec::with_capacity(32 + LIQUIDITY_TOKEN_PDA_SEED.len());
    bytes.extend_from_slice(pool_id.value());
    bytes.extend_from_slice(LIQUIDITY_TOKEN_PDA_SEED);
    pda_seed_from_bytes(&bytes)
}

fn compute_lp_lock_holding_pda(amm_program_id: ProgramId, pool_id: AccountId) -> AccountId {
    AccountId::for_public_pda(&amm_program_id, &compute_lp_lock_holding_pda_seed(pool_id))
}

fn compute_lp_lock_holding_pda_seed(pool_id: AccountId) -> PdaSeed {
    let mut bytes = Vec::with_capacity(32 + LP_LOCK_HOLDING_PDA_SEED.len());
    bytes.extend_from_slice(pool_id.value());
    bytes.extend_from_slice(LP_LOCK_HOLDING_PDA_SEED);
    pda_seed_from_bytes(&bytes)
}

fn compute_current_tick_account_pda(
    twap_oracle_program_id: ProgramId,
    price_source_id: AccountId,
) -> AccountId {
    AccountId::for_public_pda(
        &twap_oracle_program_id,
        &compute_current_tick_account_pda_seed(price_source_id),
    )
}

fn compute_current_tick_account_pda_seed(price_source_id: AccountId) -> PdaSeed {
    let mut bytes = Vec::with_capacity(32 + CURRENT_TICK_ACCOUNT_PDA_SEED.len());
    bytes.extend_from_slice(price_source_id.value());
    bytes.extend_from_slice(CURRENT_TICK_ACCOUNT_PDA_SEED);
    pda_seed_from_bytes(&bytes)
}

fn pda_seed_from_bytes(bytes: &[u8]) -> PdaSeed {
    PdaSeed::new(Sha256::digest(bytes).into())
}

fn run_strip_r0bf(args: &[String]) -> ! {
    if args.len() != 4 && args.len() != 5 {
        usage();
    }

    let input_path = &args[2];
    let output_path = &args[3];
    let strip_bin = args
        .get(4)
        .map(String::as_str)
        .unwrap_or("riscv32-unknown-elf-strip");

    let input = fs::read(input_path).unwrap_or_else(|err| {
        eprintln!("failed to read {input_path}: {err}");
        process::exit(1);
    });
    let binary = ProgramBinary::decode(&input).unwrap_or_else(|err| {
        eprintln!("failed to decode R0BF binary {input_path}: {err:#}");
        process::exit(1);
    });
    let before_image_id = compute_image_id(&input).unwrap_or_else(|err| {
        eprintln!("failed to compute input ImageID: {err:#}");
        process::exit(1);
    });

    let temp_prefix = env::temp_dir().join(format!("lez-inspect-strip-{}", process::id()));
    let user_elf_path = with_extension(&temp_prefix, "user.elf");
    let stripped_user_elf_path = with_extension(&temp_prefix, "user.stripped.elf");
    fs::write(&user_elf_path, binary.user_elf).unwrap_or_else(|err| {
        eprintln!("failed to write temporary user ELF: {err}");
        process::exit(1);
    });

    let status = Command::new(strip_bin)
        .arg("--strip-all")
        .arg("-o")
        .arg(&stripped_user_elf_path)
        .arg(&user_elf_path)
        .status()
        .unwrap_or_else(|err| {
            eprintln!("failed to run strip command {strip_bin}: {err}");
            process::exit(1);
        });
    if !status.success() {
        eprintln!("strip command failed with status {status}");
        process::exit(1);
    }

    let stripped_user_elf = fs::read(&stripped_user_elf_path).unwrap_or_else(|err| {
        eprintln!("failed to read stripped user ELF: {err}");
        process::exit(1);
    });
    let repacked = ProgramBinary::new(&stripped_user_elf, binary.kernel_elf).encode();
    let after_image_id = compute_image_id(&repacked).unwrap_or_else(|err| {
        eprintln!("failed to compute output ImageID: {err:#}");
        process::exit(1);
    });

    fs::write(output_path, &repacked).unwrap_or_else(|err| {
        eprintln!("failed to write {output_path}: {err}");
        process::exit(1);
    });

    let _ = fs::remove_file(&user_elf_path);
    let _ = fs::remove_file(&stripped_user_elf_path);

    println!("input_size={}", input.len());
    println!("output_size={}", repacked.len());
    println!("user_elf_size={}", binary.user_elf.len());
    println!("stripped_user_elf_size={}", stripped_user_elf.len());
    println!("image_id_before={before_image_id}");
    println!("image_id_after={after_image_id}");
    println!("image_id_unchanged={}", before_image_id == after_image_id);

    process::exit(0);
}

fn with_extension(path: &std::path::Path, extension: &str) -> PathBuf {
    let mut path = path.to_path_buf();
    path.set_extension(extension);
    path
}

fn usage() -> ! {
    eprintln!(
        "usage: lez-inspect hash-deploy <program.bin> | program-id <program.bin> | decode-block <block-json> | decode-block-range <range-json> | fetch-tx <tx-hash> [sequencer-url] | find-tx-block <tx-hash> <start-block> <end-block> [sequencer-url] | create-public-account | account-json <account-id> [sequencer-url] | account-data-hex <account-id> [sequencer-url] | token-account-json <definition|holding> <account-id> [sequencer-url] | token-new-public <program.bin> <definition-account> <holding-account> <name> <total-supply> <dry-run|submit|dry-run-prehash|submit-prehash> | amm-account-json <config|pool|current-tick> <account-id> [sequencer-url] | amm-init-public <amm.bin> <token.bin> <twap_oracle.bin> <authority-account> <dry-run|submit|dry-run-prehash|submit-prehash> | amm-init-validate-local <amm.bin> <token.bin> <twap_oracle.bin> <authority-account> <dry-run|submit|dry-run-prehash|submit-prehash> | amm-new-definition-public <amm.bin> <twap_oracle.bin> <token-a-definition> <token-b-definition> <user-holding-a> <user-holding-b> <user-holding-lp> <token-a-amount> <token-b-amount> <fees> <deadline-ms> <dry-run|submit|dry-run-prehash|submit-prehash> | strip-r0bf <input.bin> <output.bin> [strip-bin]"
    );
    process::exit(2);
}
