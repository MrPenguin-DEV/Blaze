# Blaze implementation status

This document distinguishes code that exists from integrations that are still missing.

## Seven implementation areas

| Area | Status | What is in the repository |
|---|---|---|
| 1. Rust workspace/core | Implemented | Multi-crate Rust workspace, shared types, memory policy, tab snapshots. |
| 2. Native shell | Prototype implemented | Native eframe/egui desktop shell rendered through wgpu; no Electron or embedded system WebView. |
| 3. Browser boundary | Partial | Safe URL normalization and BrowserEngine trait. Actual Servo page rendering, input, downloads, and permissions are not connected. |
| 4. GPU/media | Partial | wgpu accelerates the native UI. Web-content compositing and hardware video decode are not connected. |
| 5. Tabs/memory | Policy implemented | Background suspension decisions, pinning and dirty-tab protection, configurable live-tab ceiling. Actual renderer reclamation and RAM telemetry require the engine. |
| 6. Privacy/storage | Primitive implemented | Argon2-derived key and ChaCha20-Poly1305 encrypted file envelope, Notes vault UI, and standalone SHA-256 disk cache with deduplication/LRU quota. OS keychain, encrypted profile DB, cache-to-engine integration, private-mode teardown, and formal review remain. |
| 7. Workspace/WASM apps | Foundations implemented | Code editor buffer, Markdown document editor/export, CSV editor/export, presentation outline/export, notes, message draft, call UI, Wasmi compute-only runtime with fuel/memory limits. Full app services and office file interoperability remain. |

## Run the native app

Requires Rust 1.95+ and platform windowing/graphics development libraries:

```sh
cargo run -p blaze-desktop
```

## Run checks

```sh
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

CI is configured to run formatting, compilation, and tests on each push and pull request.

## Critical remaining work

### Real browser engine

Connect Servo through a dedicated host integration. The Servo embedding contract requires an EventLoopWaker, Servo instance, RenderingContext, WebViewBuilder, WebViewDelegate frame presentation, and input forwarding. The embedder must reconcile Servo's rendering context with Blaze's native wgpu UI and prove that the chosen path is stable on Linux, Windows, and macOS.

### Hardware video and performance

Integrate and benchmark platform decode backends. A target of 50 live tabs all decoding 4K simultaneously below 1 GiB is not a safe engineering guarantee. The first performance target should be many suspended tabs with one or a few active renderers, reporting process RAM and GPU memory independently.

### Office and communication

The initial editors save common portable text formats. Add tested DOCX, XLSX, and PPTX import/export; spreadsheet formulas and charting; presentation layout/rendering; document pagination; and robust autosave/recovery. Messaging requires user identity, trust/key verification, relay or peer connectivity, spam controls, and a reviewed protocol. Calling additionally requires device permission UX, WebRTC media, echo cancellation, NAT traversal, and encrypted transport.

### Security

The vault primitive has authentication and tamper-detection tests, but it is not a complete password manager or encrypted browser profile. Add OS keychain support, secure file permissions, key rotation and recovery strategy, profile isolation, a content security policy, and independent security review. The Wasm runner has no host imports by default and applies fuel and linear-memory limits; it still needs process isolation and explicit, audited capability APIs before third-party extensions are enabled. The content cache is a separate module and still needs concurrency coordination and browser-request integration.
