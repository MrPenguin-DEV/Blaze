# Blaze

**Blaze** is a Rust-first, privacy-focused desktop browser and productivity workspace.

Current version: 0.2.0 · Native shell prototype · Linux-first development, cross-platform architecture

## What is implemented in this repository

- A native Rust desktop shell rendered with egui/eframe and its wgpu graphics backend. It does not use Electron or an embedded system WebView.
- Browser address validation, safe-scheme filtering, and a strict browser-engine adapter contract.
- Tab lifecycle policy with foreground/background/suspended states, pinning, unsaved-tab protection, and a configurable live-tab ceiling.
- A local-file vault using Argon2-derived keys and ChaCha20-Poly1305 authenticated encryption.
- A content-addressed SHA-256 disk cache with deduplication, LRU eviction, and a configurable byte quota.
- A compute-only WebAssembly runner using Wasmi with fuel and linear-memory limits and no host imports by default.
- First-party UI surfaces for Code Studio, Markdown documents, CSV sheets, slide outlines, notes, message drafts, calling UI, and the encrypted vault.
- GitHub Actions CI for formatting, checking, and tests.

## Important implementation limits

Blaze is not yet a complete, daily-usable web browser. The native shell and browser controls are present, but a production page renderer is not connected. The adapter is the integration point for Servo. Servo embedding requires an event-loop waker, a Servo instance, a rendering context, WebView delegation, frame presentation, and forwarding input events. It is not enough to add an engine dependency.

The native shell's wgpu renderer accelerates the app UI. It does **not** mean web content is already GPU-composited. Web-page rendering, GPU surfaces, hardware video decode, WebRTC, and renderer process isolation are not active in this prototype.

The productivity panels are useful starter surfaces, not full Office replacements. Documents are Markdown-first, spreadsheets use CSV, and presentations are outline-based. DOCX/XLSX/PPTX compatibility, formulas, charts, slide layout, language servers, and terminal integration are pending. Messenger is a local draft panel; calling buttons are disabled. No messaging service, verified identity, reviewed end-to-end encryption protocol, or media transport is included.

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
```

On Linux, the windowing/graphics development packages required by winit/wgpu may need to be installed for your distribution. Hardware acceleration depends on the graphics driver and backend available on the host.

## Workspace layout

- `crates/blaze-core` — tab/session/resource policy.
- `crates/blaze-browser` — URL policy and browser-engine adapter contract.
- `crates/blaze-cache` — bounded content-addressed disk cache.
- `crates/blaze-vault` — authenticated encryption and encrypted file helpers.
- `crates/blaze-workspace` — app data models and export helpers.
- `crates/blaze-wasm` — compute-only, resource-limited Wasm runtime.
- `crates/blaze-desktop` — native desktop shell and app panels.
- `docs/ARCHITECTURE.md` — system architecture.
- `docs/IMPLEMENTATION_STATUS.md` — implementation status and remaining work.
