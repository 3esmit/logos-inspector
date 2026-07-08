# Contributing

## Development checks

Run these checks before opening a pull request:

```bash
python3 scripts/check-build-pipeline.py local
```

## Guidelines

- Keep `lez-inspect` focused on current LEZ/NSSA inspection.
- Keep `lee-inspect` limited to legacy compatibility.
- Don't commit wallet files, private keys, `.env` files, local endpoints with
  credentials, or generated build outputs.
- Prefer read-only inspection features in the UI. Transaction submission
  helpers belong in the CLI and must be explicit.
