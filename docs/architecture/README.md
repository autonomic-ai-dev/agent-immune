# agent-immune architecture documentation

## Design goals

agent-immune provides **defense in depth** for AI-generated code. Three independent safety layers that compose for zero-trust agent workflows.

### Layer 1: Static scanning

```
agent-immune scan ./Cargo.toml
  1. Parse manifest (Cargo.toml, package.json, requirements.txt, or pipfile)
  2. Extract dependencies with version constraints
  3. Query OSV.dev vulnerability database (parallel, buffer-20 concurrency)
  4. Return structured JSON: { package, version, osv_id, severity, summary }
```

The parallel OSV query uses a bounded semaphore (20 concurrent requests) to avoid overwhelming the API while keeping scan times reasonable for large manifests.

### Layer 2: Sandboxed execution

Three backends with increasing isolation:

| Backend | Isolation | Startup | Requirements |
|---------|-----------|---------|-------------|
| `process` | Network blackhole (DNS + connect fail) | ~5ms | None (works on macOS and Linux) |
| `seccomp` | Seccomp-BPF syscall filter | ~10ms | Linux with `CONFIG_SECCOMP` |
| `firecracker` | Full micro-VM | ~200ms | Linux, `AUTONOMIC_FC_KERNEL`, `AUTONOMIC_FC_ROOTFS` |

Network isolation is implemented by setting `LD_PRELOAD` to a shared library that intercepts and blocks `socket()`, `connect()`, and `sendto()` syscalls (macOS fallback) or through seccomp-BPF rules (Linux).

### Layer 3: Memory verification

```
agent-immune verify-memory ./script.sh
  1. Execute script with `ulimit -v` memory limit
  2. Sample RSS at 100ms intervals
  3. Detect: peak RSS > threshold, growth rate > threshold/sec
  4. Return: { passed, peak_rss_kb, growth_kb, samples }
```

### Key design decisions

| Decision | Rationale |
|----------|-----------|
| **Three independent layers** | Each layer is independently useful. A solo developer might only use `scan`; a team might add `sandbox run`; an enterprise adds `verify-memory`. |
| **OSV.dev, not a local DB** | OSV is free, open, and always up-to-date. Local advisory DBs require constant updates. |
| **Process sandbox as default** | Firecracker is powerful but heavy. The process backend with network blackhole covers 90% of use cases with minimal overhead. |
| **Bounded OSV concurrency** | 20 concurrent queries balance speed against OSV rate limits and local network capacity. |

### Alternatives considered

| Option | Why rejected |
|--------|-------------|
| **Snyk / GitHub Advisory DB** | Proprietary or requires API keys. OSV.dev is open and free. |
| **gVisor / runc** | Heavier than seccomp-BPF, requires Linux kernel config. Firecracker is a better VM boundary. |
| **Static analysis only** | Misses runtime vulnerabilities (OOM, infinite loops, malicious runtime behavior). |
