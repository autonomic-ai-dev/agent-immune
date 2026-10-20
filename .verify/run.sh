#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DIR/.." && pwd)"

echo "==> [agent-immune] Starting verification suite..."
cd "$REPO_ROOT"

echo "==> Running cargo test -p agent-immune..."
cargo test -p agent-immune

echo "==> Running cargo clippy -p agent-immune (warnings are errors)..."
cargo clippy -p agent-immune --all-targets -- -D warnings

echo "==> [agent-immune] Verification passed cleanly (exit 0)."
