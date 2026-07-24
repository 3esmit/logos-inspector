# Inspector release process

Logos Inspector owns every binary built from this repository. The release
catalog indexes these source releases; it does not rebuild or rehost them.
The Cargo workspace, Core manifest, and UI manifest must carry the same
version before any release workflow can run.

## Current channel: Alpha

The project remains in Alpha while real-network end-to-end coverage is still
being completed. Every workflow creates a GitHub prerelease and never marks it
as the latest stable release. A version bump must update Cargo, Core metadata,
UI metadata, and this changelog in one issue and pull request.

## Independent release streams

Three manual workflows publish independent GitHub Releases:

| Stream | Tag | Assets |
| --- | --- | --- |
| Inspector Core | `logos_inspector-v<version>` | One merged LGX containing Linux AMD64 and Darwin ARM64 variants, plus its release sidecar |
| Inspector UI | `logos_inspector_ui-v<version>` | One merged LGX containing Linux AMD64 and Darwin ARM64 variants, plus its release sidecar |
| Standalone app | `standalone-v<version>` | Linux AMD64 AppImage, Darwin ARM64 `.app` archive, and `SHA256SUMS` |

Core and UI use the immutable shared release workflow. Both request exactly
the `linux-amd64` and `darwin-arm64` variants, require both builds, disable
catalog dispatch, and publish separate prereleases in this repository. Core
enables the host Metal toolchain because its proof dependency graph compiles
Metal kernels. UI does not.

Publication does not require a catalog URL or prior Basecamp install result.
After source assets exist, the catalog can index their immutable URLs and run
the fresh Basecamp dependency-closure test.

## Standalone portability contract

The Linux asset is an AppImage built from the official Logos directory and
AppImage bundlers. The macOS asset is an unsigned Apple silicon app built from
the official Logos directory and macOS app bundlers.

The standalone package carries:

- the compiled GUI;
- QML and icon assets;
- Qt runtime libraries, plugins, and QML imports selected by the bundler;
- the Testnet v0.2 wallet helper under `libexec`; and
- relative launchers and dynamic-library paths.

The directory bundler fails on Nix paths in interpreters, RPATH/RUNPATH,
NEEDED or Mach-O load commands, symlink targets, launchers, shebangs, QML, and
plugin metadata. Qt and GLib can retain inert build-prefix strings in compiled
vendor binaries, including source assertion paths and unused default data
locations. A raw byte scan cannot distinguish those strings from executed
paths, so the Qt bundler reports them as warnings. Each native job records a
classified file and occurrence count in its job summary.

Functional proof remains strict: each native job extracts its final
distribution asset and starts the compiled GUI for ten seconds. The Linux
smoke runs in a private mount namespace with `/nix/store` hidden, proving that
the download cannot fall back to build-host paths. The macOS smoke verifies
the relocated app tree and launches the extracted app outside the Nix store.
Any Nix path in non-compiled bundle content still fails verification.

The standalone workflow publishes a draft first, downloads and verifies every
asset and checksum, then makes the prerelease visible. A failed post-upload
check removes its draft and tag.

## Manual release checklist

1. Open one issue and pull request for release-contract or version changes.
2. Run source identity, static workflow, Rust, native, QML, and available
   native packaging checks.
3. Merge only after CI and review pass.
4. From `main`, dispatch Core, UI, or standalone independently with its
   explicit confirmation input.
5. Verify the published release tag, merged LGX variants or standalone
   checksums, and target commit.
6. Index source release URLs in the package catalog.
7. Install the exact Inspector UI dependency closure into a fresh Basecamp
   profile and load it. Record this downstream acceptance evidence with the
   catalog change.

## Promotion

Promotion to Beta requires:

- repeatable real Testnet coverage for core user stories;
- Core and UI install/load checks on both supported platforms;
- standalone extracted-GUI smoke evidence on both supported platforms;
- direct-host and LogosCore CLI connection coverage; and
- no known data-loss, transaction-safety, or node-control release blocker.

Promotion to stable requires at least two successful Beta cycles, native
artifact evidence for both platforms, and no unresolved release blocker.
