//! Tiny example for the Blaze WASM App ABI.
//!
//! Build with:
//!   rustup target add wasm32-unknown-unknown
//!   cargo build --manifest-path examples/wasm-webapp/Cargo.toml --target wasm32-unknown-unknown --release
//!
//! Open the resulting .wasm file in Blaze Code Studio using "Run WASM web app".

use std::fmt::{self, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

static CLICKS: AtomicU32 = AtomicU32::new(0);
static FRAME: Mutex<[u8; 512]> = Mutex::new([0; 512]);

struct FrameWriter<'a> {
    output: &'a mut [u8],
    length: usize,
}

impl Write for FrameWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let bytes = value.as_bytes();
        let end = self.length.checked_add(bytes.len()).ok_or(fmt::Error)?;
        if end > self.output.len() {
            return Err(fmt::Error);
        }
        self.output[self.length..end].copy_from_slice(bytes);
        self.length = end;
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn blaze_render(width: i32, height: i32) -> i64 {
    let count = CLICKS.load(Ordering::Relaxed);
    let mut frame = FRAME.lock().expect("frame lock poisoned");
    let mut writer = FrameWriter {
        output: &mut *frame,
        length: 0,
    };
    let _ = write!(
        writer,
        "{{\"title\":\"Blaze WASM Counter\",\"nodes\":[{{\"kind\":\"heading\",\"text\":\"Compiled WebAssembly app\"}},{{\"kind\":\"text\",\"text\":\"Native host viewport: {} × {}. Button presses: {}.\"}},{{\"kind\":\"button\",\"label\":\"Increment counter\",\"action\":1}},{{\"kind\":\"text\",\"text\":\"This app runs in a fuel- and memory-limited guest with no WASI or browser imports.\"}}]}}",
        width, height, count
    );
    let pointer = writer.output.as_ptr() as usize as u32;
    let length = writer.length as u32;
    drop(writer);
    drop(frame);
    (((pointer as u64) << 32) | length as u64) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn blaze_action(action_id: i32) {
    if action_id == 1 {
        CLICKS.fetch_add(1, Ordering::Relaxed);
    }
}
