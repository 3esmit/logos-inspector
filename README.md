# Logos Inspector

Logos Inspector is a native inspection toolkit for Logos networks. It currently
targets Logos Blockchain and Logos Execution Zone, with room for future
inspectors for Logos messaging, storage, and other services.

## Components

- `src/lib.rs`: shared inspection library.
- `src/main.rs`: native GUI and CLI entry point.
- `src/cli.rs`, `src/gui.rs`: mode-specific shells over the shared library.

The GUI and CLI both call the package library directly. The GUI does not shell
out to the CLI.

## Requirements

- Rust `1.94.0`.
- Python 3 for circuit bootstrap.
- Network access to the selected sequencer and indexer endpoints.
- Logos blockchain circuits `v0.5.3` when building Rust dependencies that
  require circuit verification keys.

## Build

Build the tool:

```bash
cargo build
```

Run checks:

```bash
python3 scripts/setup-circuits.py v0.5.3 /tmp/logos-blockchain-circuits
export LOGOS_BLOCKCHAIN_CIRCUITS=/tmp/logos-blockchain-circuits
cargo fmt --all -- --check
RISC0_SKIP_BUILD=1 cargo check
RISC0_SKIP_BUILD=1 cargo clippy --all-targets -- -D warnings
```

On Windows, run the same script with Python and set the environment variable in
PowerShell:

```powershell
py -3 scripts/setup-circuits.py v0.5.3 $env:TEMP\logos-blockchain-circuits
$env:LOGOS_BLOCKCHAIN_CIRCUITS="$env:TEMP\logos-blockchain-circuits"
```

## CLI

Run CLI mode:

```bash
cargo run -- cli overview
```

Examples:

```bash
cargo run -- cli head
cargo run -- cli programs
cargo run -- cli block <block-id>
cargo run -- cli tx <tx-hash>
cargo run -- cli account <account-id>
cargo run -- cli account <account-id> --idl <idl.json> --idl-account <account-type>
cargo run -- cli decode-account --data-hex <hex> --idl <idl.json> --idl-account <account-type>
cargo run -- cli decode-instruction --program-id <program-id> --words <u32-list> --idl <idl.json> --accounts <account-list>
cargo run -- cli program-file <program.bin>
cargo run -- cli rpc http://127.0.0.1:8779/ getLastFinalizedBlockId '[]'
```

## GUI

Run native GUI mode:

```bash
cargo run -- gui
```

Running without arguments also starts the GUI:

```bash
cargo run
```

## Configuration

Environment variables:

- `LOGOS_BLOCKCHAIN_CIRCUITS`: Directory containing the compatible Logos
  circuits release required by upstream Rust dependencies.

Default endpoints:

- Sequencer: `https://testnet.lez.logos.co/`
- Indexer: `http://127.0.0.1:8779/`

Both CLI and GUI allow endpoint override.

## IDL Decode

Logos Inspector is program-agnostic. To decode program-owned account data or
instruction words, provide the account or program address plus that program's
IDL JSON. The core library handles Borsh account decoding and LEZ instruction
word decoding, and both CLI and GUI use the same implementation.

The GUI exposes IDL inputs in the `Account` and `IDL` tabs. No built-in Token,
TokenDefinition, or AMM knowledge is required.

## Dependency pins

The core library depends on internal LEZ crates that are not published on
crates.io. The manifests pin those crates to
`logos-blockchain/logos-execution-zone` tag `v0.2.0-rc6`, matching the LEZ
program workspace release line.
