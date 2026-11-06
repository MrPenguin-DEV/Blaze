# Blaze

**Blaze** is a Rust-first, privacy-focused desktop browser and productivity workspace.

**Beta release:** `v0.0.1-beta` · Linux x86_64 first · one `blaze` executable

## What is implemented in this repository

- A native Rust desktop shell rendered with egui/eframe and its wgpu graphics backend. It does not use Electron or an embedded system WebView.
- The unified `blaze` executable includes the optional Servo 0.7 page-renderer mode with mouse, keyboard and wheel input, HTTP(S)-only navigation, a GStreamer media backend, and per-tab WebViews. It normally retains two warm views (active plus one unpinned tab); pinned background tabs are intentionally exempt from eviction and can raise that count. Older unpinned views are evicted and restored from URL metadata.
- Browser address validation, safe-scheme filtering, and a strict browser-engine adapter contract.
- Tab lifecycle policy with foreground/background/suspended states, pinning, unsaved-tab protection, and a configurable live-tab ceiling.
- A local-file vault using Argon2-derived keys and ChaCha20-Poly1305 authenticated encryption.
- A content-addressed SHA-256 disk cache with deduplication, LRU eviction, and a configurable byte quota.
- A resource-limited WebAssembly runtime using Wasmi 2.0, with a compute-only entry point and a Blaze UI-app ABI (compiled WASM app logic, bounded JSON widgets/canvas primitives, action callbacks, no WASI/browser/device imports by default). WASM app primitives are painted by the native eframe/wgpu UI.
- First-party UI surfaces for Code Studio, Markdown/DOCX documents, an editable CSV/XLSX cell grid with a bounded Excel-style formula evaluator and numeric chart preview, PPTX outline interchange, notes, and the encrypted vault.
- WebRTC data-channel Messenger with manual signaling, explicit out-of-band SHA-256 DTLS fingerprint verification, user-supplied STUN/TURN configuration, and optional TURN relay-only candidate policy.
- GitHub Actions CI for formatting, checking, and tests.

## Important implementation limits

Blaze is not yet a complete, daily-usable browser. An optional Servo host can render pages in a separate native window and forwards common pointer and keyboard input, but it is not embedded in the main shell's tab compositor. The main-shell tab strip does not yet own Servo WebViews or process-level tab isolation. When it launches the browser mode, it now passes its saved tab URLs/titles, pin flags, active index, and the origin-permission snapshot. The Servo host restores those URLs in its own window; this is still not an in-process compositor bridge. Downloads remain pending.

The native shell's wgpu renderer accelerates native controls and Wasm-app UI primitives. Servo uses its own rendering context in a separate window. The optional Servo build of the same `blaze` executable enables GStreamer media support; the decoder selected for a YouTube stream depends on codec, installed GStreamer plugins and platform drivers. Hardware decode, GPU-resident frame paths, YouTube compatibility and cross-device performance have not yet been verified. See `docs/VIDEO_ACCELERATION.md` and `scripts/probe-gstreamer-hwdecode.sh`.

The office bridge supports tested DOCX/XLSX/PPTX interchange, but advanced layout, charts, themes and embedded media will not round-trip perfectly through Markdown/CSV. Sheets has a bounded formula evaluator, not a complete recalculating workbook engine. Messenger uses WebRTC data channels and requires manual signaling plus independent DTLS fingerprint confirmation. It accepts user-configured STUN/TURN servers and can use relay-only ICE; relay-only hides host/reflexive IP candidates from the peer but the TURN operator still sees the client IP. There is no contact directory, built-in signaling server or calling media. Audio/video call buttons remain disabled.

The Wasm runtime executes compute-only modules and a small Blaze UI-app ABI inside Code Studio. WASM apps can return bounded text/button/progress/canvas nodes; the native eframe/wgpu host paints those nodes and dispatches button actions. This is not arbitrary HTML/DOM/JavaScript and does not provide WebGPU or direct GPU command submission to the guest. The runtime is not yet an extension marketplace or a browser-page execution engine. The cache is a standalone module and is not yet connected to browser network/resource loading.

## Memory and video target

The aspiration is to keep sleeping tabs and ordinary browsing lean. A universal guarantee of **less than 1 GB while 50 tabs all play 4K video simultaneously** is not credible across arbitrary hardware: active decoders, GPU surfaces, JavaScript heaps, network buffers, and renderer processes consume resources independently. The Servo host retains at most two warm views and evicts older unpinned WebViews; a hidden view is not guaranteed to have its JS timers, media or network activity suspended. Hardware decode and memory measurements still require real-device benchmarks.

## Build

Requires Rust 1.95 or newer and a native graphics toolchain.

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo run -p blaze-desktop

# Optional Servo page renderer and GStreamer media backend
cargo build -p blaze-desktop --features servo-host --bin blaze
cargo run -p blaze-desktop --features servo-host --bin blaze
```

The optional Servo mode is built into the same `blaze` binary; it opens its own browser window and OpenGL context. It is not an inline page surface in the shell. This feature is a large native build and requires GStreamer development libraries and runtime plugins for your platform. On Linux, the windowing/graphics development packages required by winit/wgpu may need to be installed for your distribution. Hardware acceleration depends on the graphics driver and backend available on the host.

See [BETA_RELEASE.md](docs/BETA_RELEASE.md) for the v0.0.1 beta scope, build and packaging notes.

## Workspace layout

- `crates/blaze-core` — tab/session/resource policy.
- `crates/blaze-browser` — URL policy and browser-engine adapter contract.
- `crates/blaze-cache` — bounded content-addressed disk cache.
- `crates/blaze-vault` — authenticated encryption and encrypted file helpers.
- `crates/blaze-workspace` — app data models and export helpers.
- `crates/blaze-wasm` — compute-only and native-rendered UI-app Wasm runtime with resource limits.
- `crates/blaze-desktop` — unified `blaze` executable, native shell, app panels, and optional Servo browser mode.
- `docs/ARCHITECTURE.md` — system architecture.
- `docs/BUILDING.md` — native and Servo build instructions.
- `docs/IMPLEMENTATION_STATUS.md` — implementation status and remaining work.
