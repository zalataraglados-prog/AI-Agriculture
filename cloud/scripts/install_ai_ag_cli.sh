#!/usr/bin/env bash
set -euo pipefail

INSTALL_ROOT="${INSTALL_ROOT:-/opt/ai-agriculture/cloud}"
TARGET_BIN="${TARGET_BIN:-/usr/local/bin/AI-ag}"
SOURCE_SCRIPT="${SOURCE_SCRIPT:-$INSTALL_ROOT/scripts/AI-ag}"

if [[ "$EUID" -ne 0 ]]; then
  SUDO="sudo"
else
  SUDO=""
fi

$SUDO mkdir -p "$INSTALL_ROOT/scripts"
$SUDO cp "scripts/AI-ag" "$SOURCE_SCRIPT"
$SUDO chmod +x "$SOURCE_SCRIPT"
$SUDO cp "$SOURCE_SCRIPT" "$TARGET_BIN"
$SUDO chmod +x "$TARGET_BIN"

if [[ ! -e /usr/local/bin/ai-ag ]]; then
  $SUDO ln -sf "$TARGET_BIN" /usr/local/bin/ai-ag
fi

echo "installed: $TARGET_BIN"
$TARGET_BIN help | head -n 30
