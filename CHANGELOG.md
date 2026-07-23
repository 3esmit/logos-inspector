# Changelog

All notable user-facing changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and version numbers
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Manual, fail-closed GitHub Release automation for portable Inspector core
  and UI LGX packages on Linux x86_64 and Apple silicon macOS.

## [0.2.0-rc7] - 2026-07-23

### Changed

- The release train is explicitly alpha while end-to-end coverage and direct
  host lifecycle work remain incomplete. Published builds are prereleases and
  are never marked as the latest release.
- Core packages now declare their direct Basecamp protocol module dependencies.

### Added

- A documented release process, checksum contract, and promotion criteria for
  future alpha, beta, and stable releases.
