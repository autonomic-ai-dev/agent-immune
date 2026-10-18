//! In-process WASM sandbox with fuel metering and memory limits.

use std::path::Path;
use std::time::Duration;

use agent_body_core::wasm_engine::{default_fuel_limit, shared as shared_wasm_engine};
use anyhow::{Context, Result};
use wasmtime::{ResourceLimiter, Store};

/// Per-invocation WASM sandbox result.
#[derive(Debug, Clone)]
pub struct WasmExecuteResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub fuel_consumed: u64,
    pub memory_peak_bytes: usize,
}

/// Deterministic WASM sandbox failure modes.
#[derive(Debug, thiserror::Error)]
pub enum WasmSandboxError {
    #[error("out of fuel after {consumed} instructions")]
    OutOfFuel { consumed: u64 },
    #[error("out of memory: requested {requested} bytes (limit {limit})")]
    OutOfMemory { requested: usize, limit: usize },
    #[error("wall-clock timeout after {ms} ms")]
    WallClockTimeout { ms: u64 },
    #[error("validation failed: {0}")]
    ValidationFailed(#[from] anyhow::Error),
}

/// Options for a single WASM invocation.
#[derive(Debug, Clone)]
pub struct WasmRunOptions {
    pub fuel_limit: u64,
    pub memory_limit_bytes: usize,
    pub wall_clock_timeout_ms: u64,
}

impl Default for WasmRunOptions {
    fn default() -> Self {
        Self {
            fuel_limit: default_fuel_limit(),
            memory_limit_bytes: 16 * 1024 * 1024,
            wall_clock_timeout_ms: 10_000,
        }
    }
}

struct LimiterState {
    memory_limit_bytes: usize,
    peak_memory: usize,
}

impl ResourceLimiter for LimiterState {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        self.peak_memory = self.peak_memory.max(desired);
        Ok(desired <= self.memory_limit_bytes)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
}

/// Execute a WASM module file; calls exported `run() -> i32` when present.
pub async fn run_path(
    path: &Path,
    options: &WasmRunOptions,
) -> Result<WasmExecuteResult, WasmSandboxError> {
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("read wasm module {}", path.display()))
        .map_err(WasmSandboxError::ValidationFailed)?;
    run_bytes(&bytes, options).await
}

/// Execute WASM bytes in a fuel-metered sandbox.
pub async fn run_bytes(
    wasm_bytes: &[u8],
    options: &WasmRunOptions,
) -> Result<WasmExecuteResult, WasmSandboxError> {
    let wasm_bytes = wasm_bytes.to_vec();
    let timeout_ms = options.wall_clock_timeout_ms;
    let options = options.clone();
    let timeout = Duration::from_millis(timeout_ms);

    tokio::time::timeout(timeout, async move {
        tokio::task::spawn_blocking(move || execute_sync(&wasm_bytes, &options))
            .await
            .map_err(|e| WasmSandboxError::ValidationFailed(anyhow::anyhow!("join: {e}")))?
    })
    .await
    .map_err(|_| WasmSandboxError::WallClockTimeout { ms: timeout_ms })?
}

fn execute_sync(
    wasm_bytes: &[u8],
    options: &WasmRunOptions,
) -> Result<WasmExecuteResult, WasmSandboxError> {
    let engine = shared_wasm_engine();
    let module = engine
        .get_or_compile(wasm_bytes)
        .map_err(WasmSandboxError::ValidationFailed)?;

    let state = LimiterState {
        memory_limit_bytes: options.memory_limit_bytes,
        peak_memory: 0,
    };

    let mut store = Store::new(engine.engine(), state);
    store.limiter(|s| s as &mut dyn ResourceLimiter);
    store
        .set_fuel(options.fuel_limit)
        .map_err(|e| WasmSandboxError::ValidationFailed(anyhow::anyhow!("set fuel: {e}")))?;

    let instance = wasmtime::Instance::new(&mut store, &module, &[])
        .map_err(|e| WasmSandboxError::ValidationFailed(anyhow::anyhow!("instantiate: {e}")))?;

    let exit_code = if let Ok(run) = instance.get_typed_func::<(), i32>(&mut store, "run") {
        match run.call(&mut store, ()) {
            Ok(code) => code,
            Err(e) => {
                let remaining = store.get_fuel().unwrap_or(0);
                if remaining == 0 {
                    return Err(WasmSandboxError::OutOfFuel {
                        consumed: options.fuel_limit,
                    });
                }
                return Err(map_trap_to_error(e));
            }
        }
    } else if let Ok(start) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
        if let Err(e) = start.call(&mut store, ()) {
            let remaining = store.get_fuel().unwrap_or(0);
            if remaining == 0 {
                return Err(WasmSandboxError::OutOfFuel {
                    consumed: options.fuel_limit,
                });
            }
            return Err(map_trap_to_error(e));
        }
        0
    } else {
        return Err(WasmSandboxError::ValidationFailed(anyhow::anyhow!(
            "wasm module must export run() -> i32 or _start()"
        )));
    };

    let remaining = store
        .get_fuel()
        .map_err(|e| WasmSandboxError::ValidationFailed(anyhow::anyhow!("get fuel: {e}")))?;
    let fuel_consumed = options.fuel_limit.saturating_sub(remaining);

    let peak_memory = store.data().peak_memory;

    Ok(WasmExecuteResult {
        exit_code,
        stdout: String::new(),
        stderr: String::new(),
        fuel_consumed,
        memory_peak_bytes: peak_memory,
    })
}

fn map_trap_to_error(err: wasmtime::Error) -> WasmSandboxError {
    let msg = format!("{err:#}").to_lowercase();
    if msg.contains("fuel") {
        WasmSandboxError::OutOfFuel {
            consumed: default_fuel_limit(),
        }
    } else if msg.contains("memory") {
        WasmSandboxError::OutOfMemory {
            requested: 0,
            limit: 0,
        }
    } else {
        WasmSandboxError::ValidationFailed(anyhow::anyhow!(err))
    }
}

pub fn options_from_job(job: &agent_body_core::SandboxExecute) -> WasmRunOptions {
    WasmRunOptions {
        fuel_limit: job.fuel_limit.unwrap_or_else(default_fuel_limit),
        memory_limit_bytes: job.memory_limit_bytes.unwrap_or(16 * 1024 * 1024),
        wall_clock_timeout_ms: 10_000,
    }
}

pub fn into_execute_result(
    job: &agent_body_core::SandboxExecute,
    result: Result<WasmExecuteResult, WasmSandboxError>,
) -> agent_body_core::ExecuteResult {
    match result {
        Ok(r) => agent_body_core::ExecuteResult {
            msg_id: format!("{}-result", job.msg_id),
            job_id: job.job_id.clone(),
            exit_code: r.exit_code,
            stdout: r.stdout,
            stderr: r.stderr,
            success: r.exit_code == 0,
            fuel_consumed: Some(r.fuel_consumed),
            memory_peak_bytes: Some(r.memory_peak_bytes),
        },
        Err(e) => {
            let (exit_code, stderr) = match &e {
                WasmSandboxError::OutOfFuel { consumed } => {
                    (-2, format!("out of fuel: {consumed}"))
                }
                WasmSandboxError::OutOfMemory { requested, limit } => {
                    (-3, format!("out of memory: {requested}/{limit}"))
                }
                WasmSandboxError::WallClockTimeout { ms } => (-4, format!("timeout: {ms}ms")),
                WasmSandboxError::ValidationFailed(inner) => (-1, inner.to_string()),
            };
            agent_body_core::ExecuteResult {
                msg_id: format!("{}-result", job.msg_id),
                job_id: job.job_id.clone(),
                exit_code,
                stdout: String::new(),
                stderr,
                success: false,
                fuel_consumed: None,
                memory_peak_bytes: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RETURN_42_WAT: &str = r#"
        (module
          (func (export "run") (result i32)
            i32.const 42))
    "#;

    const INFINITE_LOOP_WAT: &str = r#"
        (module
          (func (export "run") (result i32)
            (loop $l br $l)
            i32.const 0))
    "#;

    #[tokio::test]
    async fn run_returns_exit_code() {
        let options = WasmRunOptions {
            fuel_limit: 10_000_000,
            ..Default::default()
        };
        let result = run_bytes(RETURN_42_WAT.as_bytes(), &options)
            .await
            .expect("wasm run");
        assert_eq!(result.exit_code, 42);
        assert!(result.fuel_consumed > 0);
    }

    #[tokio::test]
    async fn infinite_loop_consumes_fuel() {
        let options = WasmRunOptions {
            fuel_limit: 50_000,
            ..Default::default()
        };
        let err = run_bytes(INFINITE_LOOP_WAT.as_bytes(), &options)
            .await
            .expect_err("should run out of fuel");
        assert!(
            matches!(err, WasmSandboxError::OutOfFuel { .. }),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn options_from_job_uses_defaults() {
        let job = agent_body_core::SandboxExecute {
            msg_id: "m".into(),
            job_id: "j".into(),
            command: "noop".into(),
            cwd: None,
            memory_mb: None,
            cpu_cores: None,
            backend: Some("wasm".into()),
            wasm_path: None,
            fuel_limit: None,
            memory_limit_bytes: None,
        };
        let opts = options_from_job(&job);
        assert_eq!(opts.fuel_limit, default_fuel_limit());
    }
}
