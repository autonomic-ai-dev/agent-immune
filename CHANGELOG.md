# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
