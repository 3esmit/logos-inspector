# Changelog

All notable user-facing changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and version numbers
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Source-owned, fail-closed GitHub Release automation for separate merged
  Inspector Core and UI LGX packages on Linux x86_64 and Apple silicon macOS.
- Self-contained Linux AppImage and unsigned Apple silicon macOS standalone
  bundles, with native extracted-GUI smoke tests before publication. Release
  jobs classify inert vendor build-prefix strings and reject executable
  build-host paths; Linux smoke hides the Nix store entirely.

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
