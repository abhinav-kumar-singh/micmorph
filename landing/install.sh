#!/bin/bash
set -e

echo "🎙️ Installing MicMorph for macOS..."

if [ "$(uname)" != "Darwin" ]; then
  echo "❌ This installer is for macOS only."
  exit 1
fi

TEMP_DIR=$(mktemp -d)
DMG_PATH="$TEMP_DIR/MicMorph.dmg"
MOUNT_DIR="$TEMP_DIR/mount"

cleanup() {
  if [ -d "$MOUNT_DIR" ]; then
    hdiutil detach "$MOUNT_DIR" -quiet 2>/dev/null || true
  fi
  rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

echo "⬇️  Downloading MicMorph..."
curl -fsSL -o "$DMG_PATH" "https://micmorph.work/MicMorph_0.1.0_aarch64.dmg"

echo "📦 Mounting installer..."
mkdir -p "$MOUNT_DIR"
hdiutil attach "$DMG_PATH" -mountpoint "$MOUNT_DIR" -nobrowse -quiet

echo "🚀 Installing MicMorph to /Applications..."
rm -rf /Applications/MicMorph.app
cp -R "$MOUNT_DIR/MicMorph.app" /Applications/

echo "🛡️  Configuring macOS permissions..."
xattr -cr /Applications/MicMorph.app

echo "✅ MicMorph installed successfully!"
echo "🎉 Launching MicMorph..."
open /Applications/MicMorph.app
