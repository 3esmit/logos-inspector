# Logos Inspector

Native CLI and Qt/QML desktop inspector for Logos networks.

Logos Inspector is aimed at developers and operators investigating Logos
Blockchain and Logos Execution Zone state. It is currently versioned as
`0.2.0-rc7` and licensed under MIT.

## What it provides

- Node, block, transaction, account, program, and channel inspection.
- Navigation between linked chain entities in a native desktop UI.
- User-supplied IDL decoding for program-owned account data and instruction
  words; Inspector does not assume knowledge of a particular program.
- Endpoint, source, module, capability, local-node, wallet, storage, and
  delivery diagnostics.
- A JSON-producing CLI for automation and focused investigations.

Most features are diagnostic. Potentially mutating wallet and backup actions
are exposed as separate, explicit operations.

## Quick start

With Nix flakes enabled, the standalone GUI is available through the project
flake:

```bash
git clone https://github.com/3esmit/logos-inspector.git
cd logos-inspector
nix run .#standalone
```

The flake provides the standalone app for x86_64 Linux, AArch64 Linux, and
AArch64 macOS.

Source-owned Alpha releases provide a Linux x86_64 AppImage, an unsigned Apple
silicon macOS app archive, and separate merged Core/UI LGX packages. See the
[release process](docs/release-process.md) for artifact names and verification
requirements.

For the CLI, Rust `1.94` is required:

```bash
cargo run -- cli --help
```

For example, inspect a reachable node with an explicit endpoint:

```bash
cargo run -- cli blockchain-node --node-url http://127.0.0.1:8080
```

Commands print JSON. Run `cargo run -- cli --help` to see the supported
inspection, decoding, wallet, backup, and source-diagnostic commands.

## Documentation

- [Operate Inspector on Testnet](docs/testnet-operations.md): source health,
  local LogosCore services, Channel Indexers, Zones, and sequencer dashboards.
- [Inspect and interact with Logos networks](docs/inspect-and-interact.md):
  Bedrock and Zone inspection, automatic IDL decoding, program interaction,
  wallets, Delivery, and backups.
- [Release process](docs/release-process.md): alpha release artifacts,
  checksums, cadence, and beta/stable promotion criteria.

## Build from source

Use Nix to build the packaged standalone application:

```bash
nix build .#standalone
```

For Cargo-based development, install Rust `1.94`, Python 3, and the platform
tools needed by the checks: a C/C++ toolchain, CMake, Qt 6 (including
`qmllint` and `qmltestrunner`), and Node.js/npm.

The CI workflow prepares the compatible Logos Blockchain circuit artifacts
before Rust checks. The same setup can be used locally:

```bash
python3 scripts/setup-circuits.py --install-dir /tmp/logos-blockchain-circuits
export LOGOS_BLOCKCHAIN_CIRCUITS=/tmp/logos-blockchain-circuits
RISC0_SKIP_BUILD=1 cargo check --workspace
```

`setup-circuits.py` replaces the selected installation directory. Choose a
disposable path or one whose contents may be overwritten.

To run the standalone GUI directly from a source checkout after installing Qt
6 and preparing the circuit artifacts:

```bash
RISC0_SKIP_BUILD=1 cargo run -p logos-inspector-standalone-gui
```

## Verification

Run the contributor verification profile before opening a pull request:

```bash
python3 scripts/check-build-pipeline.py local
```

It checks formatting, tracked build inputs and package metadata, generated
artifacts, the Rust workspace, native CMake tests, the local web helper, and
QML lint/tests. Focused profiles are available when needed:

```bash
python3 scripts/check-build-pipeline.py --list
python3 scripts/check-build-pipeline.py rust
python3 scripts/check-build-pipeline.py qml
python3 scripts/check-build-pipeline.py native
python3 scripts/check-build-pipeline.py web
```

GitHub Actions runs [the CI pipeline](.github/workflows/ci.yml) on pull
requests and pushes to `main`.

## Architecture

| Path | Purpose |
| --- | --- |
| `src/` | Shared Rust inspection library and Clap CLI entry point. |
| `crates/standalone-gui/` | CXX-Qt standalone host and bridge to the shared Rust services. |
| `qml/` | Qt Quick screens, state, services, and theme. |
| `crates/core-ffi/` | C ABI for the native core runtime integration. |
| `core/` | Core module package that loads the FFI library. |
| `flake.nix` | Nix builds for the standalone application and deployable modules. |

The Nix flake exposes `standalone` for the desktop application, `lgx` for the
QML module, and `core-lgx` for the core module. Native release bundles are
available as `standalone-appimage` on Linux x86_64 and
`standalone-macos-app` on Apple silicon:

```bash
nix build .#lgx
nix build .#core-lgx
nix build .#standalone-appimage       # Linux x86_64
nix build .#standalone-macos-app      # Apple silicon macOS
```

## Security

This is a local diagnostic tool. Treat wallet files, private keys, mnemonic
phrases, and endpoint credentials as secrets. Do not enter private keys or
mnemonics in the GUI, and do not commit them to the repository. See
[SECURITY.md](SECURITY.md) for the project security guidance.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md), run the local verification profile,
and keep changes focused. The project is released under the
[MIT License](LICENSE).
