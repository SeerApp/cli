#!/usr/bin/env bash
set -e

# Error and dependency check helpers
err() {
    echo "[ERROR] $1" >&2
    exit 1
}

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        err "need '$1' (command not found)"
    fi
}

# Guaranteed cleanup of TMPDIR
TMPDIR=""
trap 'if [ -n "$TMPDIR" ] && [ -d "$TMPDIR" ]; then rm -rf "$TMPDIR"; fi' EXIT

# Detect OS and ARCH
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)
    case $ARCH in
        x86_64|amd64)
            ARCH=amd64
            ;;
        aarch64|arm64)
            ARCH=arm64
            ;;
        armv7l)
            ARCH=armv7
            ;;
        *)
            echo "[ERROR] Unsupported architecture: $ARCH" >&2
            exit 1
            ;;
    esac
    case $OS in
        linux|darwin)
            ;;
        msys*|mingw*|cygwin*)
            OS=windows
            ;;
        *)
            echo "[ERROR] Unsupported OS: $OS" >&2
            exit 1
            ;;
    esac
    echo "$OS" "$ARCH"
}

# Get latest release tag from GitHub
get_latest_release() {
    curl -fsSL "https://api.github.com/repos/SeerApp/cli/releases/latest" | grep '"tag_name"' | head -1 | sed -E 's/.*: "([^"]+)".*/\1/'
}

# Download and install binary
install_cli() {
    OS="$1"
    ARCH="$2"
    TAG=$(get_latest_release)
    echo "Detected OS: $OS, ARCH: $ARCH, Latest version: $TAG"
    FILENAME="seer-$OS-$ARCH.tar.gz"
    URL="https://github.com/SeerApp/cli/releases/download/$TAG/$FILENAME"
    TMPDIR=$(mktemp -d)
    echo "Downloading $URL ..."
    if ! curl -fsSL "$URL" -o "$TMPDIR/$FILENAME"; then
        err "Download failed. Check if this platform is supported."
    fi
    tar -xzf "$TMPDIR/$FILENAME" -C "$TMPDIR"
    BIN_PATH="$TMPDIR/seer"
    if [ ! -f "$BIN_PATH" ]; then
        err "seer binary not found in archive."
    fi
    # Choose install dir
    if [ -w "$HOME/.local/bin" ]; then
        INSTALL_DIR="$HOME/.local/bin"
    elif [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    else
        INSTALL_DIR="$HOME/.local/bin"
        mkdir -p "$INSTALL_DIR"
    fi
    mv "$BIN_PATH" "$INSTALL_DIR/seer"
    chmod +x "$INSTALL_DIR/seer"
    echo "Installed to $INSTALL_DIR/seer"
    if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
        echo "[INFO] $INSTALL_DIR is not in your PATH."
        if [ "$INSTALL_DIR" = "$HOME/.local/bin" ] && [ "$(id -u)" != "0" ]; then
            add_path_if_needed "$INSTALL_DIR"
            echo "[INFO] Restart your terminal or run: source \"$HOME/.${SHELL##*/}rc\""
        else
            echo "[INFO] Add $INSTALL_DIR to your PATH to use 'seer' command."
            echo "export PATH=\"$INSTALL_DIR:\$PATH\""
        fi
    fi
    "$INSTALL_DIR/seer" --version || true
}

add_path_if_needed() {
  TARGET_DIR="$1"
  SHELL_RC=""

  # Detect user's shell rc file
  case "$SHELL" in
    */zsh)  SHELL_RC="$HOME/.zshrc" ;;
    */bash) SHELL_RC="$HOME/.bashrc" ;;
    */fish) SHELL_RC="$HOME/.config/fish/config.fish" ;;
    *)      SHELL_RC="$HOME/.profile" ;;
  esac

  mkdir -p "$(dirname "$SHELL_RC")"
  touch "$SHELL_RC"

  # Only add if missing
  if ! grep -q "$TARGET_DIR" "$SHELL_RC"; then
    echo "export PATH=\"$TARGET_DIR:\$PATH\"" >> "$SHELL_RC"
    echo "[INFO] Added $TARGET_DIR to PATH in $SHELL_RC"
  fi
}

main() {
    set -e
    # Test deps
    for cmd in curl tar mktemp chmod mv grep uname; do
        need_cmd "$cmd"
    done
    read OS ARCH < <(detect_platform)
    install_cli "$OS" "$ARCH"
}

main "$@"
