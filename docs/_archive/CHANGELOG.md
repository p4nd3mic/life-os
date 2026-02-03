## [Unreleased] - 2026-01-25

### Fixed
- CODEX_HOME resolution now consistently falls back to ~/.codex
- Removed redundant fallback chains in codex.rs, daemon.rs, prompts.rs

### Added
- User Input Collection: Agents can now prompt users for input
  - New event: `item/tool/requestUserInput`
  - New UI component: RequestUserInputMessage
  - Supports both multiple-choice and free-text questions
