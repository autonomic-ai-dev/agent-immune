# agent-immune

**Security and sandboxing layer for AI agents — dependency fuzzing, real-time AST linting, and sub-second Firecracker microVM execution.**

agent-immune is the strict defense mechanism of the organism. Before any generated code is merged or executed by the orchestrator (`agent-spine`), this Rust daemon intercepts the payload to ensure it is memory-safe, dependency-secure, and strictly isolated.

Rust is the immune system; untrusted generated code is the pathogen.

```bash
curl -fsSL https://raw.githubusercontent.com/autonomic-ai-dev/agent-immune/master/scripts/install.sh | bash -s -- --global
agent-immune run --sandbox ./untrusted-script.sh
```

**MCP is live immediately** — it exposes endpoints for the agent to request secure test environments on demand.

---

## Why agent-immune?

Delegating raw terminal access to an LLM is a massive security risk. 

1. **Supply Chain Attacks:** Agents often hallucinate package names that don't exist. Malicious actors register these typo-squatted names to execute arbitrary code on developer laptops.
2. **Infinite Loops:** An agent might write a blocking `while(true)` loop that consumes 100% CPU and freezes the host machine.
3. **Host Compromise:** Running untrusted code directly on macOS/Linux gives the agent access to SSH keys, `.aws/credentials`, and personal files.

**agent-immune fixes this with military-grade isolation:**

| Problem | agent-immune answer |
|---------|-------------------|
| "Agents hallucinate malicious packages" | **Dependency Fuzzing** — audits newly imported `npm` or `pip` packages for known CVEs or typo-squatting before `npm install` runs. |
| "Generated code causes infinite loops" | **AST Linting** — performs static analysis to prevent memory leaks and blocking loops prior to compilation. |
| "I don't trust the agent on my host OS" | **Sandboxing** — automatically spins up Firecracker microVMs or rootless Docker containers for execution. |

---

## Architectural Deep Dive

`agent-immune` acts as a zero-trust proxy between the agent's brain and the host operating system.

### 1. Ephemeral Firecracker MicroVMs
Instead of using slow Docker containers for everything, `agent-immune` leverages **Firecracker** (the same tech powering AWS Lambda).
- **Cold Boot Time:** ~150ms. 
- **Isolation:** The agent is given a stripped-down Linux kernel with strictly bounded CPU and RAM. Network egress can be disabled completely.
- **State Wiping:** Once the `agent-spine` workflow node completes, the microVM is instantly destroyed. No state is persisted.

### 2. Pre-Execution Dependency Fuzzing
Whenever the agent attempts to modify `package.json` or `Cargo.toml`, `agent-immune` intercepts the write.
- It queries the OSV (Open Source Vulnerabilities) database in real-time.
- If a package is less than 30 days old, has low download counts, or matches a known CVE, the write is rejected and the agent is warned.

### 3. Static Pathogen Analysis
Before executing any arbitrary script, `agent-immune` parses the AST.
- Blocks execution if it detects unauthorized file reads (e.g., `cat ~/.ssh/id_rsa`).
- Enforces strict timeout boundaries on compiled execution.

---

## Complete Setup (Copy & Paste)

### 1. Install the binary

```bash
curl -fsSL https://raw.githubusercontent.com/autonomic-ai-dev/agent-immune/master/scripts/install.sh | bash -s -- --global
```

### 2. Configuration (`~/.agent_immune/config.yaml`)

```yaml
sandbox:
  engine: firecracker   # or 'docker'
  max_memory_mb: 512
  max_cpu_cores: 1
  network_access: false

fuzzing:
  check_cves: true
  min_package_age_days: 7
```

### 3. Verify

```bash
agent-immune check-engine  # Verifies KVM / Hypervisor access
agent-immune status
```

---

## Commands

| Command | Description |
|---------|-------------|
| `agent-immune scan <file>` | Run AST linting and dependency fuzzing |
| `agent-immune run --sandbox` | Execute code inside an ephemeral microVM |
| `agent-immune jail` | Open an interactive shell inside a secure microVM |

---

## Development

```bash
cargo test --release -p agent-immune
cargo build --release -p agent-immune
```

## License
MIT
