#!/usr/bin/env bash
# Install agent-immune MCP server and register it with Cursor.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/autonomic-ai-dev/agent-immune/master/scripts/install.sh | bash
#   curl -fsSL ... | bash -s -- --from-source
#   curl -fsSL ... | bash -s -- --global
#
set -euo pipefail

REPO="${AGENT_IMMUNE_REPO:-autonomic-ai-dev/agent-immune}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
FROM_SOURCE=0
GLOBAL=0
PRINT_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --from-source) FROM_SOURCE=1; shift ;;
    --global) GLOBAL=1; shift ;;
    --print-only) PRINT_ONLY=1; shift ;;
    -h|--help)
      cat <<'EOF'
Install agent-immune for Cursor MCP.

Options:
  --from-source   Build with cargo instead of downloading a release binary
  --global        Write ~/.cursor/mcp.json (default: ./.cursor/mcp.json)
  --print-only    Print MCP config JSON without writing files
  --help          Show this help

Environment:
  INSTALL_DIR     Binary install location (default: ~/.local/bin)
  AGENT_IMMUNE_REPO  GitHub repo (default: autonomic-ai-dev/agent-immune)
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

detect_target() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "$os-$arch" in
    darwin-arm64|darwin-aarch64) echo "aarch64-apple-darwin" ;;
    darwin-x86_64) echo "x86_64-apple-darwin" ;;
    linux-x86_64|linux-amd64) echo "x86_64-unknown-linux-gnu" ;;
    linux-aarch64|linux-arm64) echo "aarch64-unknown-linux-gnu" ;;
    mingw*|msys*|cygwin*) echo "x86_64-pc-windows-msvc" ;;
    *) echo "unsupported" ;;
  esac
}

artifact_name() {
  local target="$1"
  if [[ "$target" == *"windows"* ]]; then
    echo "agent-immune-${target}.exe"
  else
    echo "agent-immune-${target}"
  fi
}

sign_macos_binary() {
  local bin="$1"
  if [[ "$(uname -s)" != "Darwin" ]] || [[ ! -f "$bin" ]]; then
    return 0
  fi
  if ! command -v codesign >/dev/null 2>&1; then
    echo "ERROR: macOS requires Xcode Command Line Tools to run agent-immune under Cursor." >&2
    echo "  xcode-select --install" >&2
    echo "  Then run:" >&2
    echo "    xattr -cr \"$bin\"" >&2
    echo "    codesign --force --sign - \"$bin\"" >&2
    return 1
  fi
  xattr -cr "$bin"
  codesign --force --sign - "$bin"
  if ! codesign --verify --verbose "$bin" >/dev/null 2>&1; then
    echo "ERROR: codesign verify failed for $bin" >&2
    return 1
  fi
  echo "macOS: cleared download quarantine and adhoc-signed $bin"
}

install_from_release() {
  local target asset url tmp
  target="$(detect_target)"
  if [[ "$target" == "unsupported" ]]; then
    echo "Unsupported platform. Use --from-source or install Rust and run:" >&2
    echo "  cargo install --git https://github.com/${REPO} agent-immune" >&2
    exit 1
  fi

  asset="$(artifact_name "$target")"
  url="https://github.com/${REPO}/releases/latest/download/${asset}"

  mkdir -p "$INSTALL_DIR"
  tmp="$(mktemp)"
  echo "Downloading ${url} ..."
  if ! curl -fsSL "$url" -o "$tmp"; then
    echo "Release download failed for ${asset} (${target})." >&2
    echo "Try: bash -s -- --from-source" >&2
    echo "Or:  cargo install --git https://github.com/${REPO} agent-immune" >&2
    rm -f "$tmp"
    exit 1
  fi
  chmod +x "$tmp"
  mv "$tmp" "${INSTALL_DIR}/agent-immune"
  sign_macos_binary "${INSTALL_DIR}/agent-immune"
  echo "Installed to ${INSTALL_DIR}/agent-immune"
}

install_from_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found. Install Rust from https://rustup.rs or download a release binary." >&2
    exit 1
  fi
  cargo install --git "https://github.com/${REPO}" --locked --force agent-immune
  local bin
  bin="$(command -v agent-immune)"
  sign_macos_binary "$bin"
}

ensure_path() {
  case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      echo "Add ${INSTALL_DIR} to your PATH, e.g.:"
      echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
      ;;
  esac
}

main() {
  if [[ "$FROM_SOURCE" -eq 1 ]]; then
    install_from_cargo
  else
    if ! install_from_release; then
      echo "Release download failed; trying --from-source ..." >&2
      install_from_cargo
    fi
  fi

  ensure_path

  local bin
  if command -v agent-immune >/dev/null 2>&1; then
    bin="$(command -v agent-immune)"
  else
    bin="${INSTALL_DIR}/agent-immune"
  fi

  sign_macos_binary "$bin"

  if [[ "$PRINT_ONLY" -eq 0 ]]; then
    echo ""
    echo "agent-immune installed"
    echo "  Dependency fuzzing and vulnerability scanning daemon."
    echo "  Run: agent-immune scan <manifest>"
  fi
}

main "$@"
