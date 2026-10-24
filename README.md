# agent-immune

`agent-immune` checks dependencies for known vulnerabilities, runs untrusted code in a sandbox, and measures memory growth for scripts before they reach production.

Docs: [autonomic-ai-dev.vercel.app](https://autonomic-ai-dev.vercel.app)

## Quick start

```bash
brew tap autonomic-ai-dev/tap && brew install agent-immune
agent-immune scan ./Cargo.toml
agent-immune sandbox run -- echo hello
```

## Commands

| Command | What it does |
|---------|--------------|
| `agent-immune scan <path>` | Reads the manifest and reports known vulnerabilities from OSV.dev |
| `agent-immune sandbox run -- <cmd>` | Runs a command in the configured sandbox |
| `agent-immune verify-memory <script>` | Measures peak memory and growth for a script |
| `agent-immune serve` | Runs the HTTP daemon on port 3106 |
| `agent-immune serve-mcp` | Runs the MCP stdio server only |
| `agent-immune status` | Shows the config and the active backend |

## Sandbox backends

| Backend | Isolation | Requirements |
|---------|-----------|--------------|
| `process` (default) | Subprocess with the network blocked | None |
| `seccomp` | seccomp-BPF syscall filtering | Linux |
| `firecracker` | Micro-VM | `AUTONOMIC_FC_KERNEL` and `AUTONOMIC_FC_ROOTFS` |

A `firecracker` request fails when the micro-VM can't start. It runs as a subprocess only if `AUTONOMIC_IMMUNE_ALLOW_SUBPROCESS_FALLBACK=1` is set, and the result is marked `degraded`.

## HTTP API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Daemon health |

Scanning and sandboxed execution are CLI commands. Over MCP, `immune_scan_manifest` is the only scan tool.

## Configuration

Settings live in the `[immune]` section of `~/.autonomic/config.toml`. The default port is 3106.

## Develop

```bash
cargo test -p agent-immune
cargo clippy -p agent-immune -- -D warnings
```

Feature map and knowledge base: `.verify/features/` and `.verify/knowledge-base/`.
