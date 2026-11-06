mod app;
pub use app::{WasmAppError, WasmAppFrame, WasmAppNode, WasmAppRuntime};

//! Minimal WebAssembly extension executor for untrusted, compute-only modules.
//!
//! No filesystem, network, clipboard, browser, or OS imports are linked by
//! default. Extensions receive no host capabilities unless a reviewed host API
//! is deliberately added later. Fuel and linear-memory limits bound guest work.

use thiserror::Error;
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

#[derive(Clone, Copy, Debug)]
pub struct SandboxConfig {
    pub fuel_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_stack_height: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            fuel_limit: 5_000_000,
            memory_limit_bytes: 32 * 1024 * 1024,
            max_stack_height: 512 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionReport {
    pub fuel_limit: u64,
    pub fuel_remaining: u64,
    pub fuel_consumed: u64,
}

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("Wasm module validation failed: {0}")]
    Validation(String),
    #[error("Wasm instantiation failed: {0}")]
    Instantiation(String),
    #[error("exported entrypoint '{0}' is missing or has the wrong signature")]
    InvalidEntrypoint(String),
    #[error("Wasm execution trapped: {0}")]
    Execution(String),
    #[error("could not configure the Wasm fuel budget: {0}")]
    FuelConfiguration(String),
}

#[derive(Debug)]
struct HostState {
    limits: StoreLimits,
}

/// Validates and executes a no-argument/no-result Wasm entrypoint.
///
/// This is deliberately compute-only: the linker has no host imports, so an
/// extension cannot access the filesystem, network, browser state, or devices.
/// A production extension API must add individually reviewed capability
/// functions and must still run inside an OS process sandbox.
pub fn run_compute_module(
    wasm_bytes: &[u8],
    entrypoint: &str,
    sandbox: SandboxConfig,
) -> Result<ExecutionReport, SandboxError> {
    let mut engine_config = Config::default();
    engine_config.consume_fuel(true);
    engine_config.set_max_recursion_depth(128);
    engine_config.set_max_stack_height(sandbox.max_stack_height);
    let engine = Engine::new(&engine_config);

    let module = Module::new(&engine, wasm_bytes)
        .map_err(|error| SandboxError::Validation(error.to_string()))?;

    let limits = StoreLimitsBuilder::new()
        .memory_size(sandbox.memory_limit_bytes)
        .instances(1)
        .memories(1)
        .tables(4)
        .build();
    let state = HostState { limits };
    let mut store = Store::new(&engine, state);
    store.limiter(|state| &mut state.limits);
    store
        .set_fuel(sandbox.fuel_limit)
        .map_err(|error| SandboxError::FuelConfiguration(error.to_string()))?;

    // Empty linker means imported host functions are denied by default.
    let linker = Linker::<HostState>::new(&engine);
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|error| SandboxError::Instantiation(error.to_string()))?;

    let function = instance
        .get_typed_func::<(), ()>(&store, entrypoint)
        .map_err(|_| SandboxError::InvalidEntrypoint(entrypoint.to_owned()))?;
    function
        .call(&mut store, ())
        .map_err(|error| SandboxError::Execution(error.to_string()))?;

    let remaining = store
        .get_fuel()
        .map_err(|error| SandboxError::FuelConfiguration(error.to_string()))?;
    Ok(ExecutionReport {
        fuel_limit: sandbox.fuel_limit,
        fuel_remaining: remaining,
        fuel_consumed: sandbox.fuel_limit.saturating_sub(remaining),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_pure_compute_extension() {
        let wasm = wat::parse_str(r#"(module (func (export "run")))"#).unwrap();
        let report = run_compute_module(&wasm, "run", SandboxConfig::default()).unwrap();
        assert!(report.fuel_consumed <= report.fuel_limit);
    }

    #[test]
    fn refuses_missing_entrypoint() {
        let wasm = wat::parse_str(r#"(module (func (export "other")))"#).unwrap();
        let error = run_compute_module(&wasm, "run", SandboxConfig::default()).unwrap_err();
        assert!(matches!(error, SandboxError::InvalidEntrypoint(_)));
    }
}
