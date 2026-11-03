#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
APP="$HOME/Applications/ClaudeSessions.app"
mkdir -p "$APP/Contents/MacOS"
swift build -c release --product ClaudeSessions
cp .build/release/ClaudeSessions "$APP/Contents/MacOS/ClaudeSessions"
cat > "$APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>local.claude-sessions</string>
  <key>CFBundleName</key><string>ClaudeSessions</string>
  <key>CFBundleExecutable</key><string>ClaudeSessions</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSUIElement</key><true/>
</dict>
</plist>
EOF
codesign --force -s - "$APP" >/dev/null
echo "$APP"
