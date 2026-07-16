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
- Logos blockchain circuits from `build-artifacts.json` when building Rust
  dependencies that require circuit verification keys.

## Build

Prepare the Logos blockchain circuits once per machine or CI workspace:

```bash
python3 scripts/setup-circuits.py --install-dir /tmp/logos-blockchain-circuits
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

Cargo targets are declared explicitly, so untracked `build.rs`, binary, test,
example, and benchmark files are not discovered. Rust modules explicitly named
by tracked source still resolve from the working tree; use the Nix build for a
Git-filtered source boundary.

Run the local verification profile:

```bash
python3 scripts/check-build-pipeline.py local
```

Focused profiles are available for Rust, QML, web UI, package identity, and
external artifact checks:

```bash
python3 scripts/check-build-pipeline.py --list
python3 scripts/check-build-pipeline.py rust
python3 scripts/check-build-pipeline.py qml
python3 scripts/check-build-pipeline.py web
```

Check the Nix standalone build plan before running a full build:

```bash
df -h /
nix build --dry-run .#standalone
```

Use the Git flake form shown above. `nix build .` includes tracked files and
tracked working-tree changes while excluding untracked files. Do not use
`nix build path:.`, which imports the entire working tree before filtering.
Verify the source boundary with
`python3 scripts/check-nix-tracked-source.py` on a Nix-enabled host.

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
py -3 scripts/setup-circuits.py --install-dir $env:TEMP\logos-blockchain-circuits
$env:LOGOS_BLOCKCHAIN_CIRCUITS="$env:TEMP\logos-blockchain-circuits"
```

## CLI

Run CLI mode:

```bash
cargo run -- cli --help
```

Examples:

```bash
cargo run -- cli blockchain-node
cargo run -- cli blockchain-blocks --slot-from 0 --slot-to 100
cargo run -- cli channels --slot-from 0 --slot-to 100
cargo run -- cli blockchain-module
cargo run -- cli storage --source-mode rest --cid <cid>
cargo run -- cli messaging --source-mode rest
cargo run -- cli wallet status --wallet-binary <wallet> --wallet-home <wallet-home>
cargo run -- cli wallet accounts --wallet-binary <wallet> --wallet-home <wallet-home>
cargo run -- cli wallet bedrock-balance <64-hex-public-key>
cargo run -- cli decode-account --data-hex <hex> --idl <idl.json> --idl-account <account-type>
cargo run -- cli decode-instruction --program-id <program-id> --words <u32-list> --idl <idl.json> --accounts <account-list>
cargo run -- cli program-file <program.bin>
cargo run -- cli rpc http://127.0.0.1:8080/ chain_info '[]'
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
- Bedrock node: `http://127.0.0.1:8080/`

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
crates.io. `build-artifacts.json` records the LEZ, circuits, and rapidsnark
pins used by Cargo, Nix, CI, and the circuit setup script.
