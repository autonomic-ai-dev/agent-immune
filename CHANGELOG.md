# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
