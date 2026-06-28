# LEZ Inspect

LEZ Inspect is a local inspection toolkit for Logos Execution Zone networks.
It provides command-line tools and a web UI for reading sequencer, indexer,
transaction, block, account, and program binary data.

## Components

- `crates/lez-inspect`: Current LEZ/NSSA CLI.
- `crates/lee-inspect`: Legacy LEE compatibility CLI.
- `ui`: Local web UI backed by `lez-inspect`.

Use `lez-inspect` for current LEZ testnet work. Use `lee-inspect` only when
you need to inspect or submit legacy LEE-format public transactions.

## Requirements

- Rust `1.94.0`.
- Node.js `20` or newer for the UI.
- Network access to the selected sequencer and indexer endpoints.
- Logos blockchain circuits `v0.4.2` when building Rust dependencies that
  require circuit verification keys.

## Build

Build the current CLI:

```bash
cargo build -p lez-inspect
```

Build the legacy CLI:

```bash
cargo build -p lee-inspect
```

Run static checks:

```bash
./scripts/setup-circuits.sh v0.4.2 /tmp/logos-blockchain-circuits
export LOGOS_BLOCKCHAIN_CIRCUITS=/tmp/logos-blockchain-circuits
cargo fmt --all -- --check
RISC0_SKIP_BUILD=1 cargo check --workspace
node --check ui/server.js
node --check ui/public/app.js
```

## CLI

Show available commands:

```bash
cargo run -p lez-inspect --
```

Examples:

```bash
cargo run -p lez-inspect -- fetch-tx <tx-hash> https://testnet.lez.logos.co/
cargo run -p lez-inspect -- account-json <account-id> https://testnet.lez.logos.co/
cargo run -p lez-inspect -- program-id <program.bin>
```

Commands that submit transactions require a configured LEZ wallet. Set the
wallet environment variables expected by the upstream wallet crate before
running submit commands.

## UI

Start the web UI:

```bash
cd ui
npm start
```

Open:

```text
http://127.0.0.1:8787
```

The UI builds `lez-inspect` on demand when the CLI binary is missing.

## Configuration

Environment variables:

- `HOST`: HTTP bind host for the UI. Default: `127.0.0.1`.
- `PORT`: HTTP bind port for the UI. Default: `8787`.
- `LEZ_SEQUENCER_ENDPOINT`: Default sequencer JSON-RPC endpoint.
- `LEZ_INDEXER_ENDPOINT`: Default indexer JSON-RPC endpoint.
- `LEZ_IDL_DIR`: Directory containing `*-idl.json` files for UI loading.
- `LEZ_INSPECT_CLI`: Optional path to a prebuilt `lez-inspect` binary.
- `LOGOS_BLOCKCHAIN_CIRCUITS`: Directory containing the compatible Logos
  circuits release required by upstream Rust dependencies.

If `LEZ_IDL_DIR` is unset, the UI looks for IDLs in `artifacts/` at the repo
root. You can also paste an IDL directly into the UI.

## Dependency pins

The tools depend on internal LEZ crates that are not published on crates.io.
The manifests pin those crates to public git revisions:

- `lez-inspect`: `logos-blockchain/logos-execution-zone` at
  `cf3639d8252040d13b3d4e933feb19b42c76e14a`.
- `lee-inspect`: `logos-blockchain/logos-execution-zone` at
  `27360cb7d6ccb2bfbcca7d171bab8a3938490264`.

These pins make a public clone reproducible without private local paths.
