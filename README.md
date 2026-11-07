# Blaze

**Blaze** is a Rust-first, privacy-focused desktop browser and productivity workspace.

**Beta release:** `v0.0.1-beta` · Linux x86_64 first · one `blaze` executable

## What is implemented in this repository

- A native Rust desktop shell rendered with egui/eframe and its wgpu graphics backend. It does not use Electron or an embedded system WebView.
- The unified `blaze` executable embeds Servo 0.7 directly in the Browser panel. Each tab owns a separate WebView and independent navigation history; tabs share one Servo engine and browser storage profile. Mouse, keyboard and wheel input are forwarded in the same window.
- Browser address validation, safe-scheme filtering, and a strict browser-engine adapter contract.
- Tab lifecycle policy with foreground/background/suspended states, pinning, unsaved-tab protection, and a configurable live-tab ceiling.
- A local-file vault using Argon2-derived keys and ChaCha20-Poly1305 authenticated encryption.
- A content-addressed SHA-256 disk cache with deduplication, LRU eviction, and a configurable byte quota.
- A resource-limited WebAssembly runtime using Wasmi 2.0, with a compute-only entry point and a Blaze UI-app ABI (compiled WASM app logic, bounded JSON widgets/canvas primitives, action callbacks, no WASI/browser/device imports by default). WASM app primitives are painted by the native eframe/wgpu UI.
- First-party UI surfaces for Code Studio, Markdown/DOCX documents, an editable CSV/XLSX cell grid with a bounded Excel-style formula evaluator and numeric chart preview, PPTX outline interchange, notes, and the encrypted vault.
- WebRTC data-channel Messenger with manual signaling, explicit out-of-band SHA-256 DTLS fingerprint verification, user-supplied STUN/TURN configuration, and optional TURN relay-only candidate policy.
- GitHub Actions CI for formatting, checking, and tests.

## Beta status

This is an early Linux-first beta, not a stable daily-driver browser. The one downloadable executable contains the workspace and embedded Servo browser in a single native window. Background-page freezing, downloads, hardware-video qualification, and cross-device benchmarks are not complete.

## Important implementation limits

Blaze is not yet a complete, daily-usable browser. Servo now renders inside the Browser panel in the same Blaze window. Each tab owns its own WebView and navigation history, while all tabs share one Servo engine and storage profile (cookies and site storage are not partitioned per tab). Rendering currently uses Servo's software offscreen context, reads RGBA frames back to the CPU, and uploads them to egui's GPU texture; this is an integration path, not a zero-copy GPU bridge, and may affect scrolling/video performance. Downloads remain pending.

The native shell's wgpu renderer accelerates native controls and Wasm-app UI primitives. Servo renders offscreen through its software rendering context and its frame is uploaded into the shell. The embedded Servo build enables GStreamer media support; the decoder selected for a YouTube stream depends on codec, installed GStreamer plugins and platform drivers. Hardware decode, GPU-resident frame paths, YouTube compatibility and cross-device performance have not yet been verified. See `docs/VIDEO_ACCELERATION.md` and `scripts/probe-gstreamer-hwdecode.sh`.

The office bridge supports tested DOCX/XLSX/PPTX interchange, but advanced layout, charts, themes and embedded media will not round-trip perfectly through Markdown/CSV. Sheets has a bounded formula evaluator, not a complete recalculating workbook engine. Messenger uses WebRTC data channels and requires manual signaling plus independent DTLS fingerprint confirmation. It accepts user-configured STUN/TURN servers and can use relay-only ICE; relay-only hides host/reflexive IP candidates from the peer but the TURN operator still sees the client IP. There is no contact directory, built-in signaling server or calling media. Audio/video call buttons remain disabled.

The Wasm runtime executes compute-only modules and a small Blaze UI-app ABI inside Code Studio. WASM apps can return bounded text/button/progress/canvas nodes; the native eframe/wgpu host paints those nodes and dispatches button actions. This is not arbitrary HTML/DOM/JavaScript and does not provide WebGPU or direct GPU command submission to the guest. The runtime is not yet an extension marketplace or a browser-page execution engine. The cache is a standalone module and is not yet connected to browser network/resource loading.

## Memory and video target

The aspiration is to keep sleeping tabs and ordinary browsing lean. A universal guarantee of **less than 1 GB while 50 tabs all play 4K video simultaneously** is not credible across arbitrary hardware: active decoders, GPU surfaces, JavaScript heaps, network buffers, and renderer processes consume resources independently. The tab scheduler drops suspended background WebViews after the live-tab ceiling is reached and recreates them from their saved URLs when revisited; hidden views are not guaranteed to have their JS timers, media or network activity suspended. Hardware decode and memory measurements still require real-device benchmarks.

## Build

Requires Rust 1.95 or newer and a native graphics toolchain.

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo run -p blaze-desktop

# Optional Servo page renderer and GStreamer media backend
cargo build --release -p blaze-desktop --features servo-browser --bin blaze

# Launch the workspace shell
./target/release/blaze
```

The embedded Servo browser is built into the same `blaze` binary and appears inside the Browser panel; there is no second Servo host window or `--servo-host` process. This feature is a large native build and requires GStreamer development libraries and runtime plugins for your platform. On Linux, the windowing/graphics development packages required by winit/wgpu may need to be installed for your distribution. Hardware acceleration depends on the graphics driver and backend available on the host.

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
