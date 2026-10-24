# Scan and sandbox

## What it does

agent-immune checks dependency manifests against the OSV.dev vulnerability database, runs commands inside a configured isolation backend, measures a script's memory growth to catch leaks, and runs sandbox jobs that arrive on the event bus.

## How it works

1. **Manifest scan.** `agent-immune scan <path>` parses the manifest and queries OSV.dev. The MCP tool `immune_scan_manifest` does the same.
2. **Sandbox backends.** `agent-immune sandbox run -- <cmd>` picks a backend:
   - `subprocess` (default): a child process with a network blackhole (`run_with_network_blackhole`)
   - `wasm`: a WebAssembly module with fuel and memory limits, and errors for out-of-fuel, out-of-memory, and wall-clock timeout
   - `firecracker`: a micro-VM, which needs `AUTONOMIC_FC_KERNEL`, `AUTONOMIC_FC_ROOTFS`, and a `firecracker` binary
3. **Seccomp.** On Linux, the `seccomp` option (default `true`) applies a seccomp-BPF filter to the subprocess.
4. **Memory check.** `agent-immune verify-memory <script>` prints peak RSS and growth rate.
5. **Bus consumer.** `agent-immune serve` runs `run_sandbox_consumer` next to the HTTP server. It takes jobs from `autonomic.execute.sandbox` and publishes results to `autonomic.execute.result`.
6. **Status.** `GET /health` on port 3106, and `agent-immune status`.

## Where the code is

- Sandbox: `agent-immune/src/sandbox.rs` (backend selection around line 85, network block at line 115, seccomp at line 280)
- WASM: `agent-immune/src/wasm_sandbox.rs`
- Bus consumer: `agent-immune/src/jetstream_consumer.rs`
- Test: `tests/jetstream_consumer_tests.rs`, which starts a real `nats-server`

## How to use it

- `agent-immune scan Cargo.toml` to check dependencies
- `agent-immune sandbox run -- <cmd>` to run a command isolated
- `agent-immune serve` to accept sandbox jobs from the bus

## How it is verified

- `cargo test` for the whole crate (18 tests in the recorded baseline)
- `sh .verify/checks/require_tests.sh --test jetstream_consumer_tests`: a job runs and its result is published
- `bash .verify/checks/route_drift.sh`: the README route table matches source

## Known limits

- **Silent downgrade.** A request for `firecracker` without the kernel and rootfs variables runs as a plain subprocess, with only a warning in the log. A caller that asked for VM isolation gets less isolation with no error. This is security-relevant. The fix is to fail the request, with a test.
- Seccomp and Firecracker have not been exercised on this macOS host.
- `scan` queries a live service. A result without network access proves nothing, and a clean result must not be read as "no vulnerabilities".
- The subprocess backend is not a hardened container. On macOS its network block is its only isolation.
- The README lists routes `POST /scan` and `POST /sandbox/run` and tools `immune_sandbox_run` and `immune_lint_ast`. The code does not have them. Fix the README, or add the routes.
- The consumer's URL handling was fixed on 2026-10-24. The helper is duplicated in agent-muscle until body-core ships it under a new tag.

## Related

- Trace and adaptation in muscle, which runs commands with a similar guard: [trace-and-adaptation](../../../agent-muscle/.verify/knowledge-base/trace-and-adaptation.md)
