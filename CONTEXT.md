# agent-immune — Context for agents

Zero-trust admission for Autonomic (OSV scan, sandbox, memory verify). Part of [agent-body](https://github.com/autonomic-ai-dev/agent-body).

| Field | Value |
|-------|-------|
| Role | Admission / policy (OPA analog) |
| Binary | `agent-immune` |
| HTTP | `:3106` |
| Version | v1.0.0-beta.2 |
| Docs Hub | [https://autonomic-ai.dev/docs/security](https://autonomic-ai.dev/docs/security) · [Architecture](https://autonomic-ai.dev/docs/architecture) |

## When to use

Scan manifests for CVEs before run; wrap untrusted execute in seccomp/Firecracker; gate on RSS growth.

## Key verbs

- `scan` / OSV parallel queries
- `sandbox run`
- `verify-memory`

## Verification

Scan pass/fail can be recorded when allowlisted:

```bash
agent-brain verify --source orchestrator -- agent-immune scan …
```

Prefer project `.verify/verify.toml` entries over ad-hoc asserts.

## CI

Path filter on `agent-immune/**` + Cargo/Makefile/scripts/`.github`. Docs-only PRs skip compose.

## Related

Operator usage: [https://autonomic-ai.dev/docs/security](https://autonomic-ai.dev/docs/security) · Architecture: [https://autonomic-ai.dev/docs/architecture](https://autonomic-ai.dev/docs/architecture) · Muscle (execution peer) · spine (policy nodes) · brain (verify)
