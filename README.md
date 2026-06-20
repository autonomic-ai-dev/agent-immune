# agent-immune

**Dependency fuzzing and vulnerability scanning daemon — parse manifests, query OSV.dev, report findings.**

agent-immune scans project dependency manifests (Cargo.toml, package.json) and queries the Open Source Vulnerabilities (OSV.dev) database to find known vulnerabilities in your dependencies.

---

## Why agent-immune?

AI agents frequently install dependencies without checking for known vulnerabilities. agent-immune provides a fast, local first-line defense.

| Problem | agent-immune answer |
|---------|-------------------|
| "Did that new dependency have a recent CVE?" | **OSV.dev query** — checks each dependency against the open vulnerability database |
| "I have 200 crates in my project" | **Manifest parsing** — reads Cargo.toml and package.json automatically |
| "I want CI to catch this" | **JSON output** — structured results for pipeline integration |

---

## Commands

| Command | Description |
|---------|-------------|
| `agent-immune scan <path>` | Parse manifest and query OSV.dev for vulnerabilities |
| `agent-immune serve` | Start daemon (future: automated scanning) |
| `agent-immune status` | Show config path and current settings |

---

## Quick Install

```bash
curl -fsSL https://raw.githubusercontent.com/autonomic-ai-dev/agent-immune/master/scripts/install.sh | bash
```

## Development

```bash
cargo build --release -p agent-immune
cargo test --release -p agent-immune
```

## License

MIT
