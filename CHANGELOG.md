# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [1.0.0-beta.3.9] - 2026-10-21

### Added
- Blocking scans request human approval from agent-mouth; without approval they fail closed.
- macOS CLI notarization step when the App Store Connect API key secrets are set.

### Changed
- Sandbox runs in an ephemeral copy of the workspace and enforces a 60s timeout.
- `.verify/run.sh` entrypoint added.

### Security
- OSV scans fail closed and block critical, high, and unknown advisories.


## [1.0.0-beta.3.1] - 2026-10-18

### Fixed
- **Tracing Stdio Leakage**: Routed tracing subscriber logs explicitly to `stderr` in `main.rs` to maintain clean MCP/JSON-RPC stdio pipes.
- **Lint Cleanup**: Resolved clippy warnings across scanner modules.

### Changed
- Bumped version to `v1.0.0-beta.3.1`.

## [1.0.0-beta.3] - 2026-10-17

### Changed
- Bumped version to `v1.0.0-beta.3` and updated `agent-body-core` dependency to `v1.0.0-beta.3`.
- Relocated comprehensive `CONTEXT.md` to repository root and aligned documentation hub links.

## [1.0.0-beta.2] - 2026-10-16

### Added
- Official Homebrew tap formula support (`brew install agent-immune`).

### Changed
- Consolidated architecture documentation into centralized ecosystem hub.

## [1.0.0-beta.1] - 2026-10-16

### Changed
- Unified ecosystem versioning to 1.0.0-beta.1
- Pinned `agent-body-core` dependency to `v1.0.0-beta.1`
- Preserved CI Developer ID signatures in `scripts/install.sh` (clears quarantine without re-signing)
- Dynamic GitHub pre-release flag in release workflows for beta/rc/alpha tags
- CI strictly arm64 macOS native runners with Developer ID code signing
- Aligned documentation and CI path filters with agent-brain

## [0.5.10] - 2026-07-18

### Added

- Developer ID sign macOS release binaries

## [0.5.9] - 2026-07-02

### Added

- **WASM sandbox (Phase 1 kernel overhaul)** — `wasm_sandbox.rs` with fuel metering, memory limits, wall-clock timeout, and blake3 module cache via `agent-body-core::wasm_engine`
- **`backend: "wasm"`** routing in `run_isolated` — set `wasm_path` or a `.wasm` command path; configure fuel via `AUTONOMIC_WASM_FUEL` (default 500M instructions)
- **Health endpoint** — reports `wasm_sandbox` and default fuel budget

### Changed

- **agent-body-core** — git branch `kernel-phase-1` with `wasm` feature until `v0.5.17` is tagged
- **`ExecuteResult`** — propagates `fuel_consumed` and `memory_peak_bytes` from WASM runs

## [0.5.8] - 2026-06-27

### Changed

- **agent-body-core dependency** — updated tag to `v0.5.14` for ecosystem alignment

## [0.5.7] - 2026-06-27

### Added

- **`serve-mcp` subcommand** — starts the MCP stdio server only (no HTTP daemon). Used by `agent-body serve-mcp` gateway for persistent tool aggregation.
- Dual-mode MCP server with organ tool naming convention (`immune_scan_manifest`, `immune_sandbox_run`, `immune_lint_ast`)
- **Integration tests** — `ServeMcp` CLI parsing, `ImmuneMcp` tool handler response validation (manifest scan, sandbox run, AST lint)

## [0.5.6] - 2026-06-23

### Added

- Global `--progress` CLI flag for structured ProgressTree output (also `AGENT_PROGRESS=1`)

## [0.5.5] - 2026-06-21

### Fixed

- Optimize `scan_npm` and `scan_cargo` by using OSV batch query API for ~10x speedup

## [0.5.4] - 2026-06-21

### Fixed

- CI builds use git `agent-body-core` dependency instead of a local path

## [0.5.3] - 2026-06-21

### Added

- Sandbox jobs honor `memory_mb`, `cpu_cores`, and `backend` from `SandboxExecute` JetStream payloads

### Changed

- JetStream consumer uses `agent_body_core::connect_nats()` for authenticated NATS
- Linux subprocess sandbox applies `ulimit` resource caps (256MB default)

## [0.5.2] - 2026-06-20

### Changed

- OSV vulnerability queries run concurrently (buffer_unordered 20) for faster manifest scans

## [0.5.1] - 2026-06-20

### Added

- `--version` CLI flag (`6183db9`)
- Mermaid architecture charts in README (`eeea3a8`)

### Changed

- Professional README with standalone and integrated usage (`4cb18f8`)

### Fixed

- aarch64 seccomp allowlist uses `faccessat` (`393cb62`)
- Arch-specific seccomp syscall lists for aarch64 CI (`86db422`)
- seccompiler `BackendError` on Linux (`410697a`)
- Linux seccomp compile for CI (`54c05ff`)

## [0.5.0] - 2026-06-20

### Added

- **Firecracker backend** — `sandbox.backend = "firecracker"` with `AUTONOMIC_FC_KERNEL` + `AUTONOMIC_FC_ROOTFS`
- **Seccomp-BPF profiles** — Linux subprocess sandbox applies syscall filter (`sandbox.seccomp`, default true)

## [0.4.0] - 2026-06-20

### Added

- **Memory leak gate** — `verify-memory script.sh` samples RSS and rejects runaway growth before dataset commits

## [0.3.0] - 2026-06-20

### Added

- **JetStream sandbox consumer** — `serve` consumes `autonomic.execute.sandbox`, publishes `autonomic.execute.result`
- **Network egress blackhole** — Linux `unshare -n` isolation (`sandbox.network_blackhole`, default on)
- **`agent-immune sandbox`** CLI — run scripts in isolated subprocess with optional `--allow-network`

### Changed

- Version bumped from `0.2.0` to `0.3.0`

## [0.2.0] - 2026-06-20

### Added

- **Unified config** — loads from `~/.autonomic/config.toml` via `agent-body-core::organ_config::load("immune")`

### Changed

- Version bumped from `0.1.0` to `0.2.0`

## [0.1.0] - 2026-06-20

### Added

- **Initial project scaffold** — workspace, crate, config with auto-created `~/.config/agent-immune/config.yaml`
- **Dependency scanner** — parses Cargo.toml and package.json manifests; extracts dependency name, version, ecosystem
- **OSV.dev integration** — async API client queries the Open Source Vulnerabilities database per dependency
- **CLI** — `agent-immune scan <path>` (parse + query OSV), `serve` (daemon placeholder), `status` (config info)
- **CI pipeline** — test + build + release workflows matching ecosystem standard
