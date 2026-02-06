#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <path-to-app-bundle>"
  exit 1
fi

APP_PATH="$1"
if [[ ! -d "$APP_PATH" ]]; then
  echo "sign-dev-app: app bundle not found at $APP_PATH"
  exit 1
fi

if [[ "${SKIP_DEV_SIGNING:-0}" == "1" ]]; then
  echo "sign-dev-app: SKIP_DEV_SIGNING=1, skipping codesign."
  exit 0
fi

STATE_DIR="${CODEX_SIGNING_STATE_DIR:-$HOME/.codexmonitor}"
IDENTITY_CACHE_FILE="${CODEX_SIGNING_IDENTITY_CACHE_FILE:-$STATE_DIR/dev-signing-identity.txt}"
mkdir -p "$STATE_DIR" >/dev/null 2>&1 || true

read_cached_identity() {
  if [[ -f "$IDENTITY_CACHE_FILE" ]]; then
    sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' "$IDENTITY_CACHE_FILE" || true
  fi
}

write_cached_identity() {
  local value="$1"
  if [[ -n "$value" ]]; then
    printf "%s\n" "$value" >"$IDENTITY_CACHE_FILE" 2>/dev/null || true
  fi
}

identity_list="$(security find-identity -v -p codesigning 2>/dev/null || true)"

resolve_identity() {
  local list="$1"
  local explicit="${CODEX_DEV_SIGNING_IDENTITY:-}"
  if [[ -n "$explicit" ]]; then
    printf "%s\n" "$explicit"
    return
  fi

  local cached
  cached="$(read_cached_identity)"
  if [[ -n "$cached" ]] && printf "%s\n" "$list" | grep -Fq "$cached"; then
    printf "%s\n" "$cached"
    return
  fi

  local preferred
  preferred="$(
    printf "%s\n" "$list" | awk '/Apple Development:/{print $2; exit}'
  )"
  if [[ -z "$preferred" ]]; then
    preferred="$(
      printf "%s\n" "$list" | awk '/Developer ID Application:/{print $2; exit}'
    )"
  fi
  printf "%s\n" "$preferred"
}

IDENTITY="$(resolve_identity "$identity_list")"

if [[ -z "$IDENTITY" ]]; then
  echo "sign-dev-app: no signing identity found. Set CODEX_DEV_SIGNING_IDENTITY or install an Apple Development cert."
  exit 0
fi

if ! printf "%s\n" "$identity_list" | grep -Fq "$IDENTITY"; then
  echo "sign-dev-app: identity '$IDENTITY' not found in keychain; skipping."
  exit 0
fi

write_cached_identity "$IDENTITY"

echo "sign-dev-app: signing '$APP_PATH' with identity: $IDENTITY"
codesign --force --deep --sign "$IDENTITY" --timestamp=none "$APP_PATH"
codesign --verify --deep --strict "$APP_PATH"
echo "sign-dev-app: done (cached identity: $IDENTITY_CACHE_FILE)."
