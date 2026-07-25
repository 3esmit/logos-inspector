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

## Complete release entry point

One confirmed dispatch from `main` publishes every Inspector-owned product
artifact for the current source version:

| Product | Tag | Assets |
| --- | --- | --- |
| Basecamp Core module | `logos_inspector-v<version>` | One merged LGX containing Linux AMD64 and Darwin ARM64 variants, plus its release sidecar |
| Basecamp UI module | `logos_inspector_ui-v<version>` | One merged LGX containing Linux AMD64 and Darwin ARM64 variants, plus its release sidecar |
| Standalone app | `standalone-v<version>` | Linux AMD64 AppImage, Darwin ARM64 `.app` archive, and `SHA256SUMS` |

Use the complete release workflow for normal publication:

```bash
gh workflow run release.yml -f confirm=true --ref main
```

`release.yml` validates source identity and release contracts, refuses any
existing Core/UI/standalone tag or release for the current version, then
publishes all three streams in parallel. A final job downloads every published
asset and verifies the complete set before the run succeeds.

Catalog tags remain independent so Basecamp can resolve each package by its
source-owned release URL. The catalog indexes those immutable URLs after the
source assets exist; publication does not require a catalog URL or prior
Basecamp install result.

## Independent stream republish

Each stream also keeps a dedicated workflow for isolated republish after a
partial failure or for targeted recovery:

| Stream | Workflow |
| --- | --- |
| Inspector Core | `release-core.yml` |
| Inspector UI | `release-ui.yml` |
| Standalone app | `release-standalone.yml` |

```bash
gh workflow run release-core.yml -f confirm=true --ref main
gh workflow run release-ui.yml -f confirm=true --ref main
gh workflow run release-standalone.yml -f confirm=true --ref main
```

Core and UI use the immutable shared release workflow. Both request exactly
the `linux-amd64` and `darwin-arm64` variants, require both builds, disable
catalog dispatch, and publish separate prereleases in this repository. Core
enables the host Metal toolchain because its proof dependency graph compiles
Metal kernels. UI does not. Standalone builds and smokes native packages on
Linux and macOS, including a Linux GUI smoke with `/nix/store` hidden.

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

Linux GPU and display libraries remain host-provided because they must match
the recipient hardware and driver stack. A supported Linux desktop must expose
the Vulkan, EGL, GLX, OpenGL, and GLVND dispatch interfaces used by Qt:
`libvulkan.so.1`, `libEGL.so.1`, `libGLX.so.0`, `libOpenGL.so.0`, and
`libGLdispatch.so.0`. Ubuntu provides them through `libvulkan1`, `libegl1`,
`libglx0`, and `libopengl0` (with `libglvnd0` pulled transitively).

The headless release runner installs those host packages only after building
the AppImage. It then audits the extracted executable with `ldd`, rejects any
unresolved library, and runs the same hidden-Nix-store smoke. GPU driver
libraries from the build runner are never copied into the release artifact.

The standalone workflow publishes a draft first, downloads and verifies every
asset and checksum, then makes the prerelease visible. A failed post-upload
check removes its draft and tag.

## Manual release checklist

1. Open one issue and pull request for release-contract or version changes.
2. Run source identity, static workflow, Rust, native, QML, and available
   native packaging checks.
3. Merge only after CI and review pass.
4. From `main`, dispatch the complete release workflow with
   `confirm=true`. Use a stream-specific workflow only for isolated
   republish after a partial failure.
5. Verify all three published release tags, merged LGX variants, standalone
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
