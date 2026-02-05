# Branch Lanes (Desktop vs iPad WebView)

**Last updated:** 2026-02-05  
**Repo:** `/Volumes/YouTube 4TB/CodexMonitor-lifeos`

This file documents the branch split so desktop stabilization and iPad WebView experiments stay isolated.

## Active lanes

| Branch | Purpose | Anchor commit |
|---|---|---|
| `codex/desktop-stable` | Known-good desktop Life Stream UI + harness-aligned event pipeline | `a35d193` |
| `codex/ipad-webview-wip` | iPad WebView shell + bridge/bundling WIP | `8463a83` |
| `feature/life-os-main-rebuild` | Integration lane tracking `life-os/feature/life-os-main-rebuild` | moving |

## Quick usage

- **Desktop fixes only:** branch from `codex/desktop-stable`.
- **iPad WebView iteration:** branch from `codex/ipad-webview-wip`.
- **Shared merge lane:** use `feature/life-os-main-rebuild`.

## Build commands

- Desktop: `just app`
- iPad simulator (WebView): `just app-ipad`

> `just app` does not boot iPad sim; simulator runs through `just app-ipad` or Xcode MCP tools.
