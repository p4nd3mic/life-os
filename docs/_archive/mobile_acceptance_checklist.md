# ✅ CodexMonitor Mobile — Acceptance Checklist

Use this list when testing on iPhone/iPad over Tailscale. Mark ✅ as each step passes.

## 🔐 Connection + Auth
- [ ] App connects to daemon on Mac mini via Tailscale
- [ ] Auth token accepted (no unauthorized errors)
- [ ] Ping succeeds

## 🧩 Workspaces
- [ ] List workspaces loads
- [ ] Add workspace (path) works
- [ ] Connect workspace works
- [ ] Add clone / worktree works

## 🧵 Threads + Conversation
- [ ] Threads list loads (running/unread states visible)
- [ ] Resume thread opens conversation
- [ ] Start new thread works
- [ ] Streaming responses render correctly
- [ ] Tool output + diffs render correctly

## ✅ Approvals + Reviews
- [ ] Approval prompt shows Approve / Deny actions
- [ ] Approve/Deny sends response to server
- [ ] Start review flow works

## ✍️ Composer
- [ ] Send message works
- [ ] Queue mode works (if enabled)
- [ ] Attachments: Photos, Files, Pasteboard image
- [ ] Autocomplete: skills ($), prompts (/prompts:), files (@)

## 🌿 Git
- [ ] Git status loads
- [ ] Stage / unstage / revert works
- [ ] Diff viewer renders correctly
- [ ] Commit flow (prompt + generate + commit)
- [ ] Pull / push / sync works
- [ ] Branch list + checkout + create
- [ ] GitHub issues + PRs list
- [ ] PR diff + comments load

## 📂 Files + Prompts
- [ ] Files tree loads
- [ ] File contents open
- [ ] Share file works (optional)
- [ ] Prompts list/create/update/delete/move works
- [ ] Run prompt in thread works

## 💻 Terminal
- [ ] Open terminal session
- [ ] Streaming output renders
- [ ] Send input works
- [ ] Resize works on rotation

## 📊 Usage + Settings
- [ ] Local usage snapshot loads
- [ ] Settings persist (Keychain token)
- [ ] Reconnect after app background/foreground works

## 🧊 UI / Liquid Glass
- [ ] Feels native + smooth scrolling
- [ ] Glass effects on cards / chips / badges
- [ ] iPad split view works

