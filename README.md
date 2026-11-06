# Blaze

**Blaze** is a Rust-first, privacy-focused desktop browser and productivity workspace.

Current version: 0.2.0 · Native shell prototype · Linux-first development, cross-platform architecture

## What is implemented in this repository

- A native Rust desktop shell rendered with egui/eframe and the wgpu graphics backend. It does not use Electron or an embedded system WebView.
- Browser navigation input validation and a strict engine-adapter boundary.
- Tab lifecycle management with foreground/background/suspended states, pinning, dirty-tab protection, and a configurable live-tab ceiling.
- A password-derived authenticated-encryption vault for local files, with authenticated decryption and tamper detection.
- First-party workspace models and UI surfaces for Code Studio, documents, CSV spreadsheets, slide outlines, notes, messaging drafts, and calling.
- A CI workflow for formatting, checking, and tests.

## Important implementation limits

Blaze is not yet a complete, daily-usable web browser. The native shell and browser controls are present, but a production page renderer is not yet connected. The engine adapter is the integration point for Servo. Servo embedding requires an event-loop waker, a Servo instance, a rendering context, WebView delegation, frame presentation, and forwarding input events. That is more than adding a dependency to Cargo.

The built-in productivity surfaces are first working shells and local data models, not yet full Microsoft Office replacements. Documents are Markdown-first, spreadsheets currently use CSV, and presentations are outline-based. Messenger and calling are UI/data-model foundations; no network service, identity system, realtime media stack, or end-to-end encrypted protocol is claimed.

## Memory and video target

The ambition is to keep sleeping tabs and ordinary browsing lean. A universal guarantee of **less than 1 GB while 50 tabs all play 4K video simultaneously** is not credible: active decoders, GPU surfaces, JavaScript heaps, network buffers, and page processes consume resources independently. Blaze implements a scheduler that can suspend eligible background tabs, but the actual browser engine and hardware video pipeline still need measurement and optimization.

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

- crates/blaze-core — tab/session/resource policy.
- crates/blaze-browser — URL policy and browser-engine adapter contract.
- crates/blaze-vault — authenticated encryption and encrypted file helpers.
- crates/blaze-workspace — app data models and export helpers.
- crates/blaze-desktop — native desktop shell and app panels.
- docs/ARCHITECTURE.md — system architecture.
- docs/IMPLEMENTATION_STATUS.md — completed work, known gaps, and next integration steps.
