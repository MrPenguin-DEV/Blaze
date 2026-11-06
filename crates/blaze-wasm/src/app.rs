//! Sandboxed WebAssembly UI app ABI for Blaze desktop apps.
//!
//! A module exports:
//! - `memory`: linear memory containing UTF-8 JSON.
//! - `blaze_render(width: i32, height: i32) -> i64`: returns
//!   `(pointer << 32) | byte_length` for a serialized `WasmAppFrame`.
//! - `blaze_action(action_id: i32)`: optional event handler for buttons.
//!
//! The guest has no WASI or browser/device imports. Blaze parses a small,
//! non-HTML UI tree and paints it with the native egui renderer.

use super::{HostState, SandboxConfig, SandboxError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use wasmi::{Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimitsBuilder, TypedFunc};

const MAX_FRAME_BYTES: usize = 256 * 1024;
const MAX_FRAME_NODES: usize = 96;
const MAX_TEXT_BYTES: usize = 4096;

/// One safe, native-rendered UI frame returned by a Wasm web-app module.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WasmAppFrame {
    pub title: String,
    pub nodes: Vec<WasmAppNode>,
}

/// Supported widgets. No raw HTML, JavaScript, native handles or arbitrary URLs.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WasmAppNode {
    Heading { text: String },
    Text { text: String },
    Button { label: String, action: i32 },
}

#[derive(Debug, Error)]
pub enum WasmAppError {
    #[error("Wasm app validation failed: {0}")]
    Validation(String),
    #[error("Wasm app instantiation failed: {0}")]
    Instantiation(String),
    #[error("Wasm app must export memory and blaze_render(width, height) -> i64")]
    InvalidRenderAbi,
    #[error("Wasm app button was clicked but blaze_action(action_id) is not exported")]
    MissingActionExport,
    #[error("Wasm app execution trapped: {0}")]
    Execution(String),
    #[error("Wasm app returned an invalid frame: {0}")]
    InvalidFrame(String),
    #[error("Wasm app fuel configuration failed: {0}")]
    FuelConfiguration(String),
}

impl From<SandboxError> for WasmAppError {
    fn from(error: SandboxError) -> Self {
        Self::Validation(error.to_string())
    }
}

/// A loaded app instance. Keeping this value alive preserves the guest's state
/// between button events and render calls.
pub struct WasmAppRuntime {
    store: Store<HostState>,
    _instance: Instance,
    memory: Memory,
    render_fn: TypedFunc<(i32, i32), i64>,
    action_fn: Option<TypedFunc<i32, ()>>,
    sandbox: SandboxConfig,
    frame: WasmAppFrame,
    fuel_consumed: u64,
}

impl WasmAppRuntime {
    /// Loads a no-import Wasm module and renders its first frame.
    pub fn load(
        wasm_bytes: &[u8],
        sandbox: SandboxConfig,
        width: i32,
        height: i32,
    ) -> Result<Self, WasmAppError> {
        if width <= 0 || height <= 0 {
            return Err(WasmAppError::Validation(
                "viewport dimensions must be positive".to_owned(),
            ));
        }

        let mut config = Config::default();
        config.consume_fuel(true);
        config.set_max_recursion_depth(128);
        config.set_max_stack_height(sandbox.max_stack_height);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, wasm_bytes)
            .map_err(|error| WasmAppError::Validation(error.to_string()))?;

        let limits = StoreLimitsBuilder::new()
            .memory_size(sandbox.memory_limit_bytes)
            .instances(1)
            .memories(1)
            .tables(4)
            .build();
        let mut store = Store::new(&engine, HostState { limits });
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(sandbox.fuel_limit)
            .map_err(|error| WasmAppError::FuelConfiguration(error.to_string()))?;

        // Empty linker intentionally denies filesystem, network, browser, WASI,
        // clipboard, device and native-window imports.
        let linker = Linker::<HostState>::new(&engine);
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|error| WasmAppError::Instantiation(error.to_string()))?;

        let memory = instance
            .get_memory(&store, "memory")
            .ok_or(WasmAppError::InvalidRenderAbi)?;
        let render_fn = instance
            .get_typed_func::<(i32, i32), i64>(&store, "blaze_render")
            .map_err(|_| WasmAppError::InvalidRenderAbi)?;
        let action_fn = instance
            .get_typed_func::<i32, ()>(&store, "blaze_action")
            .ok();

        let mut runtime = Self {
            store,
            _instance: instance,
            memory,
            render_fn,
            action_fn,
            sandbox,
            frame: WasmAppFrame {
                title: String::new(),
                nodes: Vec::new(),
            },
            fuel_consumed: 0,
        };
        runtime.render(width, height)?;
        Ok(runtime)
    }

    /// Dispatches a guest-defined button event, then asks the guest to redraw.
    pub fn dispatch_action(
        &mut self,
        action_id: i32,
        width: i32,
        height: i32,
    ) -> Result<(), WasmAppError> {
        let action = self
            .action_fn
            .as_ref()
            .ok_or(WasmAppError::MissingActionExport)?;
        self.store
            .set_fuel(self.sandbox.fuel_limit)
            .map_err(|error| WasmAppError::FuelConfiguration(error.to_string()))?;
        action
            .call(&mut self.store, action_id)
            .map_err(|error| WasmAppError::Execution(error.to_string()))?;
        self.render(width, height)
    }

    fn render(&mut self, width: i32, height: i32) -> Result<(), WasmAppError> {
        self.store
            .set_fuel(self.sandbox.fuel_limit)
            .map_err(|error| WasmAppError::FuelConfiguration(error.to_string()))?;
        let packed = self
            .render_fn
            .call(&mut self.store, (width, height))
            .map_err(|error| WasmAppError::Execution(error.to_string()))? as u64;

        let pointer = (packed >> 32) as u32 as usize;
        let length = (packed & 0xffff_ffff) as u32 as usize;
        if length == 0 || length > MAX_FRAME_BYTES {
            return Err(WasmAppError::InvalidFrame(format!(
                "frame byte length {length} is outside 1..={MAX_FRAME_BYTES}"
            )));
        }

        let mut bytes = vec![0; length];
        self.memory
            .read(&self.store, pointer, &mut bytes)
            .map_err(|error| WasmAppError::InvalidFrame(error.to_string()))?;
        let frame: WasmAppFrame = serde_json::from_slice(&bytes)
            .map_err(|error| WasmAppError::InvalidFrame(error.to_string()))?;
        validate_frame(&frame)?;
        let remaining = self
            .store
            .get_fuel()
            .map_err(|error| WasmAppError::FuelConfiguration(error.to_string()))?;
        self.fuel_consumed = self.sandbox.fuel_limit.saturating_sub(remaining);
        self.frame = frame;
        Ok(())
    }

    pub fn frame(&self) -> &WasmAppFrame {
        &self.frame
    }

    pub fn fuel_consumed(&self) -> u64 {
        self.fuel_consumed
    }
}

fn validate_frame(frame: &WasmAppFrame) -> Result<(), WasmAppError> {
    if frame.title.len() > 512 {
        return Err(WasmAppError::InvalidFrame(
            "app title exceeds 512 bytes".to_owned(),
        ));
    }
    if frame.nodes.len() > MAX_FRAME_NODES {
        return Err(WasmAppError::InvalidFrame(format!(
            "app returned {} UI nodes; maximum is {MAX_FRAME_NODES}",
            frame.nodes.len()
        )));
    }
    for node in &frame.nodes {
        let text = match node {
            WasmAppNode::Heading { text } | WasmAppNode::Text { text } => text,
            WasmAppNode::Button { label, .. } => label,
        };
        if text.len() > MAX_TEXT_BYTES {
            return Err(WasmAppError::InvalidFrame(
                "a UI text field exceeds 4096 bytes".to_owned(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escaped_wat_string(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('"', "\\22")
            .replace('\n', "\\n")
    }

    #[test]
    fn loads_a_wasm_app_renders_and_dispatches_an_action() {
        let json = r#"{"title":"WASM Demo","nodes":[{"kind":"heading","text":"Hello"},{"kind":"button","label":"Run","action":7}]}"#;
        let wat = format!(
            r#"(module
                (memory (export "memory") 1)
                (data (i32.const 0) "{}")
                (func (export "blaze_render") (param i32 i32) (result i64)
                    (i64.const {}))
                (func (export "blaze_action") (param i32)))"#,
            escaped_wat_string(json),
            json.len()
        );
        let wasm = wat::parse_str(wat).unwrap();
        let mut app = WasmAppRuntime::load(&wasm, SandboxConfig::default(), 900, 600).unwrap();
        assert_eq!(app.frame().title, "WASM Demo");
        assert_eq!(app.frame().nodes.len(), 2);
        app.dispatch_action(7, 900, 600).unwrap();
        assert!(app.fuel_consumed() <= SandboxConfig::default().fuel_limit);
    }

    #[test]
    fn rejects_modules_without_the_blaze_render_abi() {
        let wasm = wat::parse_str(r#"(module (memory (export "memory") 1))"#).unwrap();
        let error = WasmAppRuntime::load(&wasm, SandboxConfig::default(), 640, 480).unwrap_err();
        assert!(matches!(error, WasmAppError::InvalidRenderAbi));
    }
}
