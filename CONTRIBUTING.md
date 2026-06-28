# Contributing

## Development checks

Run these checks before opening a pull request:

```bash
cargo fmt --all -- --check
RISC0_SKIP_BUILD=1 cargo check --workspace
node --check ui/server.js
node --check ui/public/app.js
```

## Guidelines

- Keep `lez-inspect` focused on current LEZ/NSSA inspection.
- Keep `lee-inspect` limited to legacy compatibility.
- Don't commit wallet files, private keys, `.env` files, local endpoints with
  credentials, or generated build outputs.
- Prefer read-only inspection features in the UI. Transaction submission
  helpers belong in the CLI and must be explicit.

