# Logos Inspector

Logos Inspector is a native inspection toolkit for Logos networks. It currently
targets Logos Blockchain and Logos Execution Zone, with room for future
inspectors for Logos messaging, storage, and other services.

## Components

- `src/lib.rs`: shared inspection library.
- `src/main.rs`: native GUI and CLI entry point.
- `src/cli.rs`: CLI shell over the shared library.
- `src/gui.rs`: thin launcher for the standalone QML flake app.
- `crates/core-ffi`: C ABI bridge used by the Basecamp core module package.
- `crates/standalone-gui`: CXX-Qt standalone host over the shared QML UI.
- `core/`: Logos core module package named `logos_inspector`.
- `qml/Main.qml`, `qml/`: Logos QML UI plugin.

The CLI calls the package library directly. The QML GUI follows the Logos UI
plugin model and routes UI actions through the injected `logos.callModule()`
bridge when hosted by Logos Basecamp or the standalone CXX-Qt host.
The host must provide the declared runtime modules for inspection actions.

## Requirements

- Rust `1.94.0`.
- Nix with flakes enabled for the QML UI.
- Python 3 for circuit bootstrap.
- Network access to the selected sequencer and indexer endpoints.
- Logos blockchain circuits `v0.5.3` when building Rust dependencies that
  require circuit verification keys.

## Build

Prepare the Logos blockchain circuits once per machine or CI workspace:

```bash
python3 scripts/setup-circuits.py v0.5.3 /tmp/logos-blockchain-circuits
export LOGOS_BLOCKCHAIN_CIRCUITS=/tmp/logos-blockchain-circuits
export RISC0_SKIP_BUILD=1
```

Build the CLI/native launcher crate:

```bash
cargo build -p logos-inspector
```

Build the standalone QML host alongside the launcher:

```bash
cargo build -p logos-inspector -p logos-inspector-standalone-gui
```

Run the standard Rust verification set:

```bash
cargo fmt --all -- --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run QML smoke tests:

```bash
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml/tst_app_model.qml
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml/tst_common_controls.qml
qmllint qml/pages/BlocksPage.qml qml/state/AppModel.qml qml/state/appmodel/AppModelPages.js
```

Check the Nix standalone build plan before running a full build:

```bash
df -h /
nix build --dry-run .#standalone
```

Build plugin outputs:

```bash
nix build
```

Build the Basecamp core runtime module:

```bash
nix build .#core-lgx
```

This writes portable package `result/logos-inspector-lib.lgx`.

Build the Basecamp UI plugin LGX:

```bash
nix build .#lgx
```

This writes portable package `result/logos-inspector-ui-module.lgx`.

Build the standalone Nix package:

```bash
nix build --max-jobs 1 --cores 2 .#standalone
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

GUI startup does not build or restart the local indexer automatically. To opt in
to that helper, set:

```bash
LOGOS_INSPECTOR_ENABLE_INDEXER_AUTO_BOOTSTRAP=1 cargo run -- gui
```

Run the Basecamp QML plugin directly:

```bash
nix run .#qml-ui
```

The Basecamp plugin requires a matching `logos_inspector` runtime module. If
the UI is updated without rebuilding or reinstalling that module, calls can fail
with `unknown inspector method`.

Build both Basecamp packages, then install `result/logos-inspector-lib.lgx` as
the core module and `result/logos-inspector-ui-module.lgx` as the UI plugin
from Basecamp's **Install LGX Package** action.

Run the standalone QML host:

```bash
nix run .#standalone
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
