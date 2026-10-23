# agent-immune features

## Baseline preconditions

- Rust toolchain matching `Cargo.lock`.
- `scan` needs network access to OSV.dev. Firecracker needs `AUTONOMIC_FC_KERNEL` and `AUTONOMIC_FC_ROOTFS`.

## Driving conventions

- Unit tests run through `cargo test`. Logs go to `.verify/artifacts/<RUN_ID>/`.

## Proof standards

- A feature is proven by its unit test. The scan result needs a recorded response, not a live query, to be reproducible.

## Feature index

| Feature file | Sub-features | Primary surface |
|---|---|---|
| `scan-and-sandbox.md` | 8 | `scan`, `sandbox run` (subprocess, seccomp flag, firecracker, wasm), `verify-memory`, `serve` (HTTP `/health` only), `serve-mcp` (`immune_scan_manifest`) |
