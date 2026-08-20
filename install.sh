#!/bin/bash

# install.sh - Installs bloom-rust (Native Rust) on the system
# Author: iapizarro
# Licensed under the GNU General Public License v3

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PREFIX=""
UNINSTALL=0

for arg in "$@"; do
    case "$arg" in
        --prefix=*) PREFIX="${arg#*=}" ;;
        --uninstall) UNINSTALL=1 ;;
        *) echo "Unknown option: $arg"; exit 1 ;;
    esac
done

if [ -z "$PREFIX" ]; then
    if [ "$EUID" -ne 0 ]; then
        PREFIX="$HOME/.local"
    else
        PREFIX="/usr/local"
    fi
fi

DEST_DIR="$PREFIX/share/bloom-rust"
BIN_DIR="$PREFIX/bin"
DESKTOP_DIR="$PREFIX/share/applications"

do_uninstall() {
    echo "[bloom-rust] Uninstalling bloom-rust by iapizarro from $PREFIX..."
    rm -f "$BIN_DIR/bloom-rust"
    rm -f "$DESKTOP_DIR/bloom-rust.desktop"
    rm -rf "$DEST_DIR"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    fi
    echo "======================================================="
    echo " Uninstallation successful!"
    echo "======================================================="
    exit 0
}

if [ "$UNINSTALL" -eq 1 ]; then
    do_uninstall
fi

check_deps() {
    if ! command -v cargo &> /dev/null; then
        echo "Error: Cargo is required but not installed."
        echo "Please install Rust & Cargo (e.g. curl https://sh.rustup.rs -sSf | sh)."
        exit 1
    fi
}

check_deps

echo "[bloom-rust] Building release binary with Cargo..."
(cd "$SCRIPT_DIR" && cargo build --release)

echo "Installing bloom-rust binary to $BIN_DIR..."
mkdir -p "$BIN_DIR"
cp "$SCRIPT_DIR/target/release/bloom-rust" "$BIN_DIR/bloom-rust.new"
chmod 755 "$BIN_DIR/bloom-rust.new"
mv -f "$BIN_DIR/bloom-rust.new" "$BIN_DIR/bloom-rust"

if [ -d "$HOME/.cargo/bin" ] && [ "$BIN_DIR" != "$HOME/.cargo/bin" ]; then
    install -m 755 "$SCRIPT_DIR/target/release/bloom-rust" "$HOME/.cargo/bin/bloom-rust" 2>/dev/null || true
fi

# Ensure any legacy desktop launcher shortcut is cleaned up
rm -f "$DESKTOP_DIR/bloom-rust.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi

echo "Cleaning build target cache to conserve disk space..."
(cd "$SCRIPT_DIR" && cargo clean)

echo "======================================================="
echo " Installation successful! bloom-rust v0.3.0 is ready."
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo " Note: $BIN_DIR is not in your PATH. You may add it via:"
    echo "   export PATH=\"\$PATH:$BIN_DIR\""
fi
echo " Run 'bloom-rust' in your terminal."
echo "======================================================="
