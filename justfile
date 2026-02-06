set shell := ["bash", "-cu"]

SIM_NAME := "iPad mini (6th generation)"
IOS_SCHEME := "CodexMonitorMobile"
IOS_PROJECT := "ios/CodexMonitorMobile/CodexMonitorMobile.xcodeproj"
IOS_DERIVED := "ios/build/DerivedData"
IOS_APP := "{{IOS_DERIVED}}/Build/Products/Debug-iphonesimulator/CodexMonitorMobile.app"
IOS_BUNDLE_ID := "com.codexmonitor.mobile"

default: app

# Build + open desktop app (dev-signed for stable macOS file permissions).
# iPad sim handled manually via MCP when needed.
app: app-desktop

app-desktop:
  pgrep -x codex-monitor >/dev/null && pkill -x codex-monitor || true
  for i in {1..25}; do pgrep -x codex-monitor >/dev/null || break; sleep 0.2; done
  pgrep -x codex-monitor >/dev/null && pkill -9 -x codex-monitor || true
  npm run doctor:strict && npx tauri build --bundles app
  scripts/sign-dev-app.sh "src-tauri/target/release/bundle/macos/CodexMonitor.app"
  open "src-tauri/target/release/bundle/macos/CodexMonitor.app"

app-ipad:
  npm run build:webview
  xcrun simctl boot "{{SIM_NAME}}" || true
  open -a Simulator
  xcodebuild -project "{{IOS_PROJECT}}" -scheme "{{IOS_SCHEME}}" -configuration Debug -destination "platform=iOS Simulator,name={{SIM_NAME}}" -derivedDataPath "{{IOS_DERIVED}}" build
  xcrun simctl install booted "{{IOS_APP}}"
  xcrun simctl launch booted "{{IOS_BUNDLE_ID}}"
