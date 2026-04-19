#!/bin/bash
# Download static FFmpeg/FFprobe binaries for bundling as Tauri sidecars.
# Run this before `npm run tauri build` or let CI do it.
#
# Tauri sidecar naming convention:
#   binaries/ffmpeg-{target}       e.g. ffmpeg-aarch64-apple-darwin
#   binaries/ffprobe-{target}      e.g. ffprobe-x86_64-pc-windows-msvc.exe

set -euo pipefail

BINDIR="$(cd "$(dirname "$0")/../src-tauri/binaries" && pwd)"
mkdir -p "$BINDIR"

# Detect or accept target
TARGET="${1:-}"
if [ -z "$TARGET" ]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)  TARGET="aarch64-apple-darwin" ;;
    Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
    Linux-x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    MINGW*|MSYS*)  TARGET="x86_64-pc-windows-msvc" ;;
    *)             echo "Unknown platform: $(uname -s)-$(uname -m)"; exit 1 ;;
  esac
fi

echo "Downloading FFmpeg for target: $TARGET"

case "$TARGET" in
  aarch64-apple-darwin|x86_64-apple-darwin)
    # evermeet.cx provides macOS universal static builds (LGPL)
    FFMPEG_URL="https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip"
    FFPROBE_URL="https://evermeet.cx/ffmpeg/getrelease/ffprobe/zip"

    echo "Downloading ffmpeg..."
    curl -L "$FFMPEG_URL" -o /tmp/ffmpeg.zip
    unzip -o /tmp/ffmpeg.zip -d /tmp/ffmpeg-extract
    cp /tmp/ffmpeg-extract/ffmpeg "$BINDIR/ffmpeg-$TARGET"
    chmod +x "$BINDIR/ffmpeg-$TARGET"
    rm -rf /tmp/ffmpeg.zip /tmp/ffmpeg-extract

    echo "Downloading ffprobe..."
    curl -L "$FFPROBE_URL" -o /tmp/ffprobe.zip
    unzip -o /tmp/ffprobe.zip -d /tmp/ffprobe-extract
    cp /tmp/ffprobe-extract/ffprobe "$BINDIR/ffprobe-$TARGET"
    chmod +x "$BINDIR/ffprobe-$TARGET"
    rm -rf /tmp/ffprobe.zip /tmp/ffprobe-extract
    ;;

  x86_64-pc-windows-msvc)
    # BtbN GitHub releases — LGPL shared/static builds for Windows
    RELEASE_URL="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl.zip"

    echo "Downloading ffmpeg (Windows)..."
    curl -L "$RELEASE_URL" -o /tmp/ffmpeg-win.zip
    unzip -o /tmp/ffmpeg-win.zip -d /tmp/ffmpeg-win-extract
    EXTRACTED=$(find /tmp/ffmpeg-win-extract -name "ffmpeg.exe" -type f | head -1)
    EXTRACTED_PROBE=$(find /tmp/ffmpeg-win-extract -name "ffprobe.exe" -type f | head -1)
    cp "$EXTRACTED" "$BINDIR/ffmpeg-$TARGET.exe"
    cp "$EXTRACTED_PROBE" "$BINDIR/ffprobe-$TARGET.exe"
    rm -rf /tmp/ffmpeg-win.zip /tmp/ffmpeg-win-extract
    ;;

  *)
    echo "Unsupported target: $TARGET"
    exit 1
    ;;
esac

echo "Done! Binaries in $BINDIR:"
ls -lh "$BINDIR/"
