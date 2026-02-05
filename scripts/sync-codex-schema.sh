#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${ROOT_DIR}/docs/_protocol"
TS_OUT="${OUT_DIR}/codex-app-server.ts"
SCHEMA_OUT="${OUT_DIR}/codex-app-server.schema.json"

if ! command -v codex >/dev/null 2>&1; then
  echo "[sync-codex-schema] codex CLI not found. Install Codex or set PATH to a pinned binary."
  exit 1
fi

mkdir -p "${OUT_DIR}"

echo "[sync-codex-schema] Generating TypeScript schema..."
codex app-server generate-ts > "${TS_OUT}"

echo "[sync-codex-schema] Generating JSON schema..."
codex app-server generate-json-schema > "${SCHEMA_OUT}"

echo "[sync-codex-schema] ✅ Wrote:"
echo "  - ${TS_OUT}"
echo "  - ${SCHEMA_OUT}"
