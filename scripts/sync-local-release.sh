#!/usr/bin/env bash
# Download the latest GitHub release tag, sign (macOS), and link MCP.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${AGENT_IMMUNE_BIN:-$HOME/.local/bin/agent-immune}"

if [[ -x "$BIN" ]]; then
  exec "$BIN" update --force --mcp-only
fi

exec "$ROOT/scripts/install.sh" --global
