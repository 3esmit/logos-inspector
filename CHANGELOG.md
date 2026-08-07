# Changelog

All notable user-facing changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and version numbers
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Managed Channel Indexer start no longer builds Basecamp instance IDs that
  exceed the LogosCore 128-byte runtime-address limit (`module@instance`),
  which previously failed with `Cannot load module with invalid runtime
  address` and `MODULE_INSTANCE_LOAD_FAILED`.

## [0.2.0-rc12] - 2026-07-28

### Fixed

- macOS release checks now exclude Linux-only standalone watcher lease helpers.

## [0.2.0-rc11] - 2026-07-28

### Added

- Independent headless CLI release automation for Linux x86_64 and Apple
  silicon macOS, with source-commit metadata, checksums, and extracted-artifact
  smoke tests.

### Fixed

- Standalone shutdown now terminates its owned LogosCore event-watch processes
  on normal application exit and reclaims stale watches left by an interrupted
  prior run.

## [0.2.0-rc10] - 2026-07-27

### Fixed

- Basecamp Local Nodes now consumes the Bedrock managed-node lifecycle V1
  contract and confirms terminal lifecycle events.
- Local Nodes can safely control already-running local service modules through
  their advertised lifecycle actions without adopting their configuration.
- Basecamp Storage lifecycle acknowledgements reconcile with the module's
  asynchronous terminal state.

## [0.2.0-rc9] - 2026-07-26

### Fixed

- Local Nodes now parses with the Qt 6.9.2 runtime bundled by Basecamp.
- Release builds keep raw circuit archive digests separate from the unpacked
  source digests required by Nix.

## [0.2.0-rc8] - 2026-07-25

### Added

- Source-owned, fail-closed GitHub Release automation for separate merged
  Inspector Core and UI LGX packages on Linux x86_64 and Apple silicon macOS.
- Self-contained Linux AppImage and unsigned Apple silicon macOS standalone
  bundles, with native extracted-GUI smoke tests before publication. Release
  jobs classify inert vendor build-prefix strings and reject executable
  build-host paths; Linux smoke hides the Nix store entirely.
- Complete release entry point that publishes Core LGX, UI LGX, and standalone
  artifacts from one confirmed `main` dispatch, then verifies the full asset
  set.

### Changed

- Inspector release automation no longer requires catalog installation
  evidence before publishing source-owned packages. Catalog indexing and
  Basecamp installation remain downstream acceptance checks.
- The Basecamp package label is now the human-facing `Logos Inspector`.
- Linux standalone smoke now provisions the complete host graphics-interface
  runtime after building the AppImage, audits dynamic dependencies, and
  preserves the host graphics-driver ABI while testing with the Nix store
  hidden.

## [0.2.0-rc7] - 2026-07-23

### Changed

- The release train is explicitly alpha while end-to-end coverage and direct
  host lifecycle work remain incomplete. Published builds are prereleases and
  are never marked as the latest release.
- Core packages now declare their direct Basecamp protocol module dependencies.

### Added

- A documented release process, checksum contract, and promotion criteria for
  future alpha, beta, and stable releases.
