# LEZ Inspect UI

Local web UI for `lez-inspect`.

Run:

```bash
cd ui
npm start
```

Open:

```text
http://127.0.0.1:8787
```

Included views:

- Overview with sequencer head, indexer finalized head, recent block window, and built-in program IDs.
- Block inspector and bounded block range scanner.
- Transaction lookup through both sequencer-backed `lez-inspect fetch-tx` and Indexer `getTransaction`.
- Account lookup through sequencer or Indexer, with optional IDL account-type decoding.
- Program file helpers for program ID and deployment hash.
- Indexer health, block, transaction, account, and account transaction lookups.
- IDL mapping by program ID or local label so public instructions and account data can be decoded.
- Raw JSON-RPC console for sequencer or Indexer.
- Guarded CLI runner for supported `lez-inspect` commands.

The server proxies RPC calls and invokes the local `lez-inspect` binary without
using a shell. If the binary is missing, use **Build CLI** in the UI or run the
normal Cargo build:

```bash
cargo build -p lez-inspect
```

Set `LEZ_IDL_DIR` to point the UI at a directory containing `*-idl.json`
artifacts. If unset, the UI uses `artifacts/` at the repository root when it
exists.
