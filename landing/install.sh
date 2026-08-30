#!/bin/bash
set -e

echo "🎙️  Installing MicMorph for macOS..."

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

# 1. Automatically install virtual audio driver if not present
if [ ! -d "/Library/Audio/Plug-Ins/HAL/BlackHole2ch.driver" ]; then
  echo "🔌 Setting up high-performance virtual audio bridge..."
  if command -v brew &> /dev/null; then
    brew install blackhole-2ch 2>/dev/null || true
  else
    TEMP_PKG="$TEMP_DIR/BlackHole2ch.pkg"
    curl -fsSL -o "$TEMP_PKG" "https://github.com/ExistentialAudio/BlackHole/releases/download/v0.6.1/BlackHole2ch.v0.6.1.pkg" 2>/dev/null || true
    if [ -f "$TEMP_PKG" ]; then
      sudo installer -pkg "$TEMP_PKG" -target / 2>/dev/null || true
    fi
  fi
  sudo killall coreaudiod 2>/dev/null || killall coreaudiod 2>/dev/null || true
fi

echo "⬇️  Downloading latest MicMorph..."
curl -fsSL -o "$DMG_PATH" "https://micmorph.work/MicMorph_0.1.0_aarch64.dmg"

echo "📦 Mounting installer..."
mkdir -p "$MOUNT_DIR"
hdiutil attach "$DMG_PATH" -mountpoint "$MOUNT_DIR" -nobrowse -quiet

echo "🚀 Installing MicMorph to /Applications..."
rm -rf /Applications/MicMorph.app
cp -R "$MOUNT_DIR/MicMorph.app" /Applications/

echo "🛡️  Configuring system security permissions..."
xattr -cr /Applications/MicMorph.app

echo "✅ MicMorph installed successfully!"
echo "🎉 Launching MicMorph..."
open /Applications/MicMorph.app
