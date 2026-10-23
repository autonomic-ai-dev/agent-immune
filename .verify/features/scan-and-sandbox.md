# scan-and-sandbox

Summary: agent-immune checks dependency manifests against the OSV.dev vulnerability database, runs commands inside a configured isolation backend, and measures a script's memory growth to catch leaks.

## Sub-features

- `manifest-scan`: Parses manifests and reports known vulnerabilities.
  - Intent: Surface vulnerable dependencies before they are installed or run.
  - Behaviour: `agent-immune scan <path>` parses the manifest and queries OSV.dev. The MCP tool `immune_scan_manifest` does the same.
  - Outcome: A list of advisories with package, version, and identifier.
- `sandbox-subprocess`: Runs a command in a subprocess with a network blackhole. This is the default backend.
  - Intent: Run untrusted code with no network access, on any host.
  - Behaviour: `agent-immune sandbox run -- <cmd>`. The backend is `subprocess` (`src/sandbox.rs` line 25). The network block is `run_with_network_blackhole` (line 115).
  - Outcome: The command runs and its output returns. Network calls fail inside the sandbox. Not verified by a test in this pass.
- `sandbox-seccomp-flag`: Applies a seccomp-BPF filter to the subprocess on Linux.
  - Intent: Restrict syscalls beyond the network block.
  - Behaviour: This is a boolean `seccomp` (`config.rs` line 36, default `true`), not a backend. When set, `sandbox.rs` line 280 calls `seccomp::apply_to_command` on Linux.
  - Outcome: Disallowed syscalls are rejected on Linux. Not verified on this macOS host.
- `sandbox-firecracker`: Runs the command in a Firecracker micro-VM.
  - Intent: Strongest isolation for the highest-risk commands.
  - Behaviour: Selected by `backend = "firecracker"` (`sandbox.rs` line 85). Requires `AUTONOMIC_FC_KERNEL` and `AUTONOMIC_FC_ROOTFS`, and a `firecracker` binary on the path.
  - Outcome: Runs in the VM when configured. When it is not configured, the backend logs a warning and runs the command as a plain subprocess (`sandbox.rs` lines 101-105). Not verified in this pass.
- `sandbox-wasm`: Runs a WebAssembly module with fuel and memory limits.
  - Intent: Give a sandbox that is deterministic and bounded by instruction count.
  - Behaviour: Selected by `backend = "wasm"` (`sandbox.rs` line 61). `wasm_sandbox.rs` runs the module with `run_path` (line 78) or `run_bytes` (line 90). Errors include `OutOfFuel`, `OutOfMemory`, and `WallClockTimeout` (line 22).
  - Outcome: A module that exhausts fuel or memory returns the matching error. Covered by 3 inline tests.
- `memory-verify`: Measures peak RSS and growth rate of a script.
  - Intent: Catch leaks as a measured number, not an impression.
  - Behaviour: `agent-immune verify-memory <script>` reports peak and growth.
  - Outcome: Numbers printed for the run.
- `jetstream-sandbox-consumer`: Runs sandbox jobs that arrive on the event bus.
  - Intent: Let other organs request isolated execution without a direct HTTP call.
  - Behaviour: `agent-immune serve` starts `run_sandbox_consumer` alongside the HTTP server (`serve.rs` lines 15-18).
  - Outcome: A job on `autonomic.execute.sandbox` runs and its result is published to `autonomic.execute.result`. Covered by `tests/jetstream_consumer_tests.rs`, which runs a real `nats-server`. A consumer error is logged as "JetStream sandbox consumer stopped".
- `immune-status-and-health`: Shows config, backends, and scanner state, and reports daemon health.
  - Behaviour: `agent-immune status`, `GET /health` on port 3106.
  - Outcome: A status table and a health JSON.

## How to get to it (user POV)

- CLI: `agent-immune scan`, `sandbox run`, `verify-memory`, `serve`, `serve-mcp`, `status`
- HTTP: `GET /health` on `http://localhost:3106`. This is the only HTTP route in `agent-immune/src/serve.rs`.
- MCP: `immune_scan_manifest` through agent-body. This is the only tool in `agent-immune/src/mcp_server.rs`.

## Verification Suites (Targeted Test Commands)

- **Unit Tests:** `cargo test` (whole crate, 18 tests in the baseline)
- **Functional API Tests (Hurl):** not yet written
- **Allowlisted Verification:** `.verify/verify.toml` runs `sh .verify/checks/require_tests.sh --test jetstream_consumer_tests`. The scan, HTTP, and sandbox suites are not yet in the allowlist.
- **Preconditions:** Rust toolchain. `scan` needs network access to OSV.dev, so a scan test must use a recorded response or be marked not available.

### Verification Steps

1. **immune-unit-tests** (sub-features: all)
   - Request: `cargo test`
   - Why: Proves the scanner, sandbox, and memory code paths.
   - Expected: 18 passed, 0 failed (baseline).
   - Impact: A failure means scan or isolation behaviour changed. Fix the cause.

## Gotchas

- Silent downgrade: a request for `firecracker` without `AUTONOMIC_FC_KERNEL` and `AUTONOMIC_FC_ROOTFS` runs as a plain subprocess, with only a warning in the log. A caller that asked for VM isolation gets less isolation without an error. This is security-relevant. Change it to fail the request, and add a test for the failure.
- Seccomp and Firecracker are not exercised on this macOS host. Their behaviour is unproven here.
- Defect: `connect_js(_url)` in `jetstream_consumer.rs` ignores its `url` argument, the same defect as in agent-muscle. The test sets `AUTONOMIC_NATS_URL`, which is how the consumer really connects.
- `manifest-scan` queries a live external service. A run without network access cannot prove a result, and a clean result must not be read as "no vulnerabilities".
- `sandbox-process` is a subprocess, not a hardened container. Its network block is the only isolation it provides on macOS.
- Documentation drift: `agent-immune/README.md` lists HTTP routes `POST /scan` and `POST /sandbox/run`, and MCP tools `immune_sandbox_run` and `immune_lint_ast`. None of them exist in the code. The code has only `GET /health` over HTTP and `immune_scan_manifest` over MCP. Fix the README or add the routes, then update this contract.
- This crate has no `.verify/` harness. Add one before the feature map can be run through the allowlist.
