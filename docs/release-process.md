# Inspector release process

Logos Inspector releases are source-versioned: the Cargo workspace, core LGX
manifest, and UI LGX manifest must carry the same version. The manual release
workflow tags that exact source version as `v<version>` and publishes all
artifacts to one GitHub Release.

## Current channel: Alpha

The project remains in the Alpha channel. The full real-network end-to-end
matrix, including direct-host lifecycle coverage, is still being completed.
Each release is therefore a GitHub prerelease and is explicitly prevented from
becoming the latest release.

The existing `0.2.0-rcN` build identity tracks the compatible protocol train;
it does not by itself promote product readiness. Alpha status is determined by
the acceptance criteria below. A coordinated version-bump issue and pull
request must update every version authority before a new source version is
released.

## Release artifacts

Every release contains exactly these files for each supported platform:

| Artifact | Linux x86_64 | Apple silicon macOS |
| --- | --- | --- |
| Core package | `logos-inspector-core-<version>-linux-amd64.lgx` | `logos-inspector-core-<version>-darwin-arm64.lgx` |
| UI package | `logos-inspector-ui-<version>-linux-amd64.lgx` | `logos-inspector-ui-<version>-darwin-arm64.lgx` |

`SHA256SUMS` covers every artifact. The workflow builds the core and UI LGX
packages on their native target runners; checks each manifest, direct module
dependencies, and target variant; verifies all hashes; uploads a draft GitHub
Release; downloads the assets again; and verifies them before publishing the
draft. If any build, validation, upload, or post-upload verification fails, the
workflow removes the draft release and tag instead of leaving a partial public
release.

No standalone downloadable artifact is published yet. The current Nix package
is not a self-contained distribution; a closure-aware bundle or installer and
a clean-environment launch check are required before standalone publication.

## Manual release checklist

1. Open one issue and one pull request for the versioned release changes.
   Update `CHANGELOG.md` with the source version and user-visible changes.
2. Merge only after the relevant real end-to-end checks, source validation,
   package identity check, and release workflow static check pass.
3. Install the exact core/UI package pair into a fresh Basecamp profile using
   the published catalog, resolve its three direct module dependencies, and
   load the Inspector UI. Record the successful check at an HTTPS evidence URL.
4. From `main`, run **Publish alpha release** with the confirmation input and
   the evidence URL.
   The workflow refuses a non-main ref, an existing tag, an existing release,
   mismatched manifests, a missing platform variant, bad checksums, or an
   incomplete artifact set.
5. Smoke-test the downloaded release artifacts on their native supported
   platforms before using them for any wider testnet audience.
6. Record the release tag, validation evidence, and any known limitations in
   the issue before closing it.

## Cadence and promotion

Publish an Alpha prerelease after a cohesive batch of user-visible fixes has
passed its focused real end-to-end checks, or at least once per active month
when there are validated changes. Do not publish empty cadence releases.

Promotion from Alpha to Beta requires:

- green, repeatable real Testnet coverage for the core Inspector user stories;
- core/UI LGX artifact install and smoke checks on both release platforms;
- a self-contained standalone artifact and clean-environment smoke checks on
  both release platforms before standalone publication;
- direct-host and LogosCore CLI connection paths exercised without a release
  blocker; and
- no known data-loss, transaction-safety, or node-control release blocker.

Promotion from Beta to stable requires at least two successful Beta release
cycles, release artifact smoke evidence for both platforms, and no unresolved
release-blocking regression. Changing the channel or source version always
goes through its own issue and pull request.
