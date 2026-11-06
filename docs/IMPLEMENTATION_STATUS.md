# Blaze implementation status

This document distinguishes code that exists from integrations that are still missing.

## Seven implementation areas

| Area | Status | What is in the repository |
|---|---|---|
| 1. Rust workspace/core | Implemented | Multi-crate Rust workspace, shared types, memory policy, tab snapshots. |
| 2. Native shell | Prototype implemented | Native eframe/egui desktop shell rendered through wgpu; no Electron or embedded system WebView. |
| 3. Browser boundary | Standalone host prototype | A separate optional Servo 0.8 host has a WebView, event-loop waker, frame presentation, URL filtering, mouse/keyboard/wheel input, address editing, back/forward history, and Ctrl+T/Ctrl+W/Ctrl+Tab tab shortcuts. It keeps tab URL/title metadata and reuses one active WebView; switching reloads the URL. It is not integrated into the main wgpu tab compositor; downloads and profile-backed permission delegates remain pending. |
| 4. GPU/media | Partial | wgpu accelerates the native UI. The optional Servo build enables its GStreamer media backend; actual playback and hardware decode depend on native GStreamer plugins, host drivers and the Servo media pipeline, and have not been verified here. |
| 5. Tabs/memory | Policy implemented | Background suspension decisions, pinning and dirty-tab protection, configurable live-tab ceiling. Actual renderer reclamation and RAM telemetry require the engine. |
| 6. Privacy/storage | Prototype implemented | Argon2/ChaCha20-Poly1305 encrypted Notes and browser profile, schema-v2 migration, bookmarks, origin-scoped permissions, capped history, and last-session tab/pin restore. A SHA-256 deduplicating quota cache exists. OS keychain, engine permission enforcement, encrypted cookie/cache integration, private-mode teardown, and formal review remain. |
| 7. Workspace/WASM apps | Prototype implemented | Code editor buffer can run compiled compute-only Wasm; Markdown/DOCX, CSV/XLSX and outline/PPTX interchange; bounded Excel-style formulas; notes and encrypted vault; peer WebRTC text data channel with explicit manual SHA-256 DTLS fingerprint verification. Full Office layout/format fidelity, language services, extension host capabilities, contact identities/signaling, and audio/video calling remain. |

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

The optional standalone Servo host now creates a WebView, drives the Servo event loop, presents frames, forwards pointer/keyboard/wheel input, supports address editing, browser history, and lightweight URL tabs. It keeps only one WebView active and reloads the saved URL when switching tabs. Remaining browser work is the in-process main-shell compositor bridge, per-tab renderer isolation/state restoration, downloads, and passing the encrypted profile's permission policy into Servo delegates. Validate builds and real page compatibility on supported operating systems.

### Hardware video and performance

Integrate and benchmark platform decode backends. A target of 50 live tabs all decoding 4K simultaneously below 1 GiB is not a safe engineering guarantee. The first performance target should be many suspended tabs with one or a few active renderers, reporting process RAM and GPU memory independently.

### Office and communication

The office bridge now has DOCX/XLSX/PPTX interchange tests and a CSV-backed Excel-style formula evaluator with a 10,000-cell range cap. Markdown conversion is lossy for advanced layout, charts, theme and media objects; full Office fidelity, formula persistence/recalculation, cell-grid virtualization, and slide rendering remain. Messenger uses WebRTC data channels and blocks sending until the SDP SHA-256 DTLS fingerprint matches a value the user enters after independent verification. It now accepts explicit STUN/TURN URLs and credentials and offers TURN relay-only ICE to hide host/reflexive candidate IPs from the remote peer (the TURN operator can still see the client IP). Long-term identity, a signaling/contacts service, secure credential storage, and message history persistence remain. Calling still needs device capture, media tracks, echo cancellation, NAT traversal, and permission integration.

### Security

The vault and encrypted profile primitives have authentication/tamper-detection tests and the profile supports encrypted session metadata. This is not a complete password manager or encrypted browser cookie store. Add OS keychain support, secure file permissions, key rotation/recovery, process/profile isolation, a content security policy, and independent security review. The Wasm runner has no host imports by default and applies fuel and linear-memory limits; it still needs process isolation and explicit, audited capability APIs before third-party extensions are enabled. The content cache is a separate module and still needs concurrency coordination and browser-request integration.
