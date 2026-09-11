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

# 1. Automatically install virtual audio driver & configure "MicMorph"
AUDIO_PLIST="/Library/Preferences/Audio/com.apple.audio.SystemSettings.plist"
DRIVER_MISSING=false
MICMORPH_MISSING=false

if [ ! -d "/Library/Audio/Plug-Ins/HAL/BlackHole2ch.driver" ]; then
  DRIVER_MISSING=true
fi

if [ ! -f "$AUDIO_PLIST" ] || ! plutil -extract Meta_UIDList json "$AUDIO_PLIST" -o - 2>/dev/null | grep -q "MicMorphAggregateUID"; then
  MICMORPH_MISSING=true
fi

if [ "$DRIVER_MISSING" = true ] || [ "$MICMORPH_MISSING" = true ]; then
  echo "🔌 Setting up high-performance virtual audio bridge..."
  TEMP_PKG="$TEMP_DIR/BlackHole2ch.pkg"
  if [ "$DRIVER_MISSING" = true ]; then
    curl -fsSL -o "$TEMP_PKG" "https://micmorph.work/BlackHole2ch.pkg" 2>/dev/null || curl -fsSL -o "$TEMP_PKG" "https://existential.audio/downloads/BlackHole2ch-0.7.1.pkg"
  fi

  SETUP_SCRIPT="$TEMP_DIR/setup_audio.sh"
  cat << 'EOF' > "$SETUP_SCRIPT"
#!/bin/sh
TEMP_PKG="$1"
if [ -f "$TEMP_PKG" ]; then
  installer -pkg "$TEMP_PKG" -target / 2>/dev/null || true
fi

AUDIO_PLIST="/Library/Preferences/Audio/com.apple.audio.SystemSettings.plist"
META_JSON='{"name":"MicMorph","uid":"MicMorphAggregateUID","vocal isolation type":0,"subdevices":[{"channels-in":2,"channels-out":2,"don'\''t pad":0,"drift":0,"drift algorithm":0,"drift quality":127,"latency-in":0,"latency-out":0,"name":"BlackHole 2ch","uid":"BlackHole2ch_UID"}]}'

plutil -replace "MetaDevice\.MicMorphAggregateUID" -json "$META_JSON" "$AUDIO_PLIST" 2>/dev/null || plutil -insert "MetaDevice\.MicMorphAggregateUID" -json "$META_JSON" "$AUDIO_PLIST" 2>/dev/null || true

if ! plutil -extract Meta_UIDList json "$AUDIO_PLIST" -o - 2>/dev/null | grep -q "MicMorphAggregateUID"; then
  plutil -insert Meta_UIDList.0 -string "MicMorphAggregateUID" "$AUDIO_PLIST" 2>/dev/null || plutil -replace Meta_UIDList -json '["MicMorphAggregateUID"]' "$AUDIO_PLIST" 2>/dev/null || plutil -insert Meta_UIDList -json '["MicMorphAggregateUID"]' "$AUDIO_PLIST" 2>/dev/null || true
fi

killall coreaudiod 2>/dev/null || true
EOF
  chmod +x "$SETUP_SCRIPT"

  echo "🔑 Configuring audio driver (one-time setup)..."
  if [ -e /dev/tty ]; then
    sudo </dev/tty "$SETUP_SCRIPT" "$TEMP_PKG" 2>/dev/null || osascript -e "do shell script \"'$SETUP_SCRIPT' '$TEMP_PKG'\" with administrator privileges" 2>/dev/null || true
  else
    osascript -e "do shell script \"'$SETUP_SCRIPT' '$TEMP_PKG'\" with administrator privileges" 2>/dev/null || true
  fi
  sleep 1
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
