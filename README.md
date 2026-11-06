# Blaze

**Blaze** is a Rust-first, privacy-focused desktop browser and productivity workspace.

Current version: 0.2.0 · Native shell prototype · Linux-first development, cross-platform architecture

## What is implemented in this repository

- A native Rust desktop shell rendered with egui/eframe and its wgpu graphics backend. It does not use Electron or an embedded system WebView.
- An optional standalone Servo 0.8 page-renderer binary with mouse, keyboard and wheel input, HTTP(S)-only navigation, a GStreamer media backend, and lightweight URL tabs switched by keyboard shortcuts. The Servo host opens in a separate native window and reuses one active WebView.
- Browser address validation, safe-scheme filtering, and a strict browser-engine adapter contract.
- Tab lifecycle policy with foreground/background/suspended states, pinning, unsaved-tab protection, and a configurable live-tab ceiling.
- A local-file vault using Argon2-derived keys and ChaCha20-Poly1305 authenticated encryption.
- A content-addressed SHA-256 disk cache with deduplication, LRU eviction, and a configurable byte quota.
- A compute-only WebAssembly runner using Wasmi with fuel and linear-memory limits and no host imports by default.
- First-party UI surfaces for Code Studio, Markdown/DOCX documents, CSV/XLSX sheets with a bounded Excel-style formula evaluator, PPTX outline interchange, notes, and the encrypted vault.
- WebRTC data-channel Messenger with manual signaling, explicit out-of-band SHA-256 DTLS fingerprint verification, user-supplied STUN/TURN configuration, and optional TURN relay-only candidate policy.
- GitHub Actions CI for formatting, checking, and tests.

## Important implementation limits

Blaze is not yet a complete, daily-usable browser. An optional Servo host can render pages in a separate native window and forwards common pointer and keyboard input, but it is not embedded in the main shell's tab compositor. The main-shell tab strip does not yet own Servo WebViews, process-level tab isolation, downloads or profile-backed permission prompts.

The native shell's wgpu renderer accelerates the app UI. Servo uses its own rendering context in a separate window. The optional Servo build enables GStreamer media support, but successful media playback and hardware decode depend on installed GStreamer plugins and platform drivers; no cross-device performance results are claimed.

The office bridge supports tested DOCX/XLSX/PPTX interchange, but advanced layout, charts, themes and embedded media will not round-trip perfectly through Markdown/CSV. Sheets has a bounded formula evaluator, not a complete recalculating workbook engine. Messenger uses WebRTC data channels and requires manual signaling plus independent DTLS fingerprint confirmation. It accepts user-configured STUN/TURN servers and can use relay-only ICE; relay-only hides host/reflexive IP candidates from the peer but the TURN operator still sees the client IP. There is no contact directory, built-in signaling server or calling media. Audio/video call buttons remain disabled.

The Wasm runner currently executes compute-only modules. Its linker exposes no host functions, and it is not yet integrated into a third-party extension manager. The cache is a standalone module and is not yet connected to browser network/resource loading.

## Memory and video target

The aspiration is to keep sleeping tabs and ordinary browsing lean. A universal guarantee of **less than 1 GB while 50 tabs all play 4K video simultaneously** is not credible across arbitrary hardware: active decoders, GPU surfaces, JavaScript heaps, network buffers, and renderer processes consume resources independently. Blaze has tab-suspension policy, but actual renderer reclamation, hardware decode, and memory measurements require the real engine integration and benchmarks.

## Build

Requires Rust 1.95 or newer and a native graphics toolchain.

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo run -p blaze-desktop

# Optional Servo page renderer and GStreamer media backend
cargo build -p blaze-desktop --features servo-host --bins
cargo run -p blaze-desktop --features servo-host --bin blaze
```

The optional Servo feature is a large native build and requires GStreamer development libraries and runtime plugins for your platform. On Linux, the windowing/graphics development packages required by winit/wgpu may need to be installed for your distribution. Hardware acceleration depends on the graphics driver and backend available on the host.

## Workspace layout

- `crates/blaze-core` — tab/session/resource policy.
- `crates/blaze-browser` — URL policy and browser-engine adapter contract.
- `crates/blaze-cache` — bounded content-addressed disk cache.
- `crates/blaze-vault` — authenticated encryption and encrypted file helpers.
- `crates/blaze-workspace` — app data models and export helpers.
- `crates/blaze-wasm` — compute-only, resource-limited Wasm runtime.
- `crates/blaze-desktop` — native desktop shell, app panels, and optional Servo browser host.
- `docs/ARCHITECTURE.md` — system architecture.
- `docs/BUILDING.md` — native and Servo build instructions.
- `docs/IMPLEMENTATION_STATUS.md` — implementation status and remaining work.
