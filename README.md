# Blaze

**Blaze** is a Rust-first, privacy-focused desktop browser and productivity workspace.

> Status: early foundation. This repository is currently being bootstrapped; it is not yet a usable web browser.

## Product goals

- Native desktop application; no Electron and no embedded system WebView as the long-term shell.
- Rust application and systems core, with WebAssembly for sandboxed extensions and selected app workloads.
- Standards-based web rendering through an embeddable browser engine (Servo is the leading candidate).
- GPU-accelerated compositing and hardware video decode where supported by the platform.
- Memory-aware tabs: discard/suspend background pages, restore from compact session snapshots, and enforce resource budgets.
- Private local storage with encryption at rest, strict site permissions, isolated processes, and clear telemetry controls.
- Integrated Code Studio, documents, spreadsheets, presentations, notes, messaging, and calling.

## Reality about the memory target

The aspiration is to keep ordinary browsing and sleeping tabs very lightweight. A hard guarantee of **under 1 GB with 50 tabs all actively playing 4K video** is not technically credible across arbitrary websites and hardware. Every active video can require decoder surfaces, GPU memory, network buffers, JavaScript heaps, and renderer resources. Blaze will measure and optimize this workload, use hardware decoding, and apply backpressure, but will not claim an impossible universal limit.

## Initial architecture

- `crates/blaze-core`: shared browser/session/resource-management primitives.
- `docs/ARCHITECTURE.md`: engine, security, storage, and application architecture.
- Planned next crates: native shell, engine adapter, renderer, storage, security, and workspace apps.

## Build

Requires a recent stable Rust toolchain:

```sh
cargo check --workspace
cargo test --workspace
```

## Current status

This commit establishes the workspace and foundational contracts only. It does not yet render websites, play video, or ship the integrated apps. Those require staged engine integration, platform backends, and end-to-end testing.
