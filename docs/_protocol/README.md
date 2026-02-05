# Codex App Server Protocol Artifacts

This folder is the **source-of-truth** for Codex app-server protocol schemas.

Use the sync script to regenerate:

```bash
./scripts/sync-codex-schema.sh
```

Outputs:
- `codex-app-server.ts` — generated TypeScript types
- `codex-app-server.schema.json` — generated JSON schema

> Note: These files are generated from `codex app-server generate-ts` and
> `codex app-server generate-json-schema`. Keep them pinned to the same Codex
> binary/version the app is using to avoid protocol drift.
