# Blaze implementation status

This document distinguishes code that exists from integrations that are still missing. The planned public beta build is `v0.0.1-beta`; it packages one `blaze` executable, with the Servo browser mode enabled for Linux x86_64.

## Seven implementation areas

| Area | Status | What is in the repository |
|---|---|---|
| 1. Rust workspace/core | Implemented | Multi-crate Rust workspace, shared types, memory policy, tab snapshots. |
| 2. Native shell | Prototype implemented | Native eframe/egui desktop shell rendered through wgpu; no Electron or embedded system WebView. |
| 3. Browser boundary | Standalone host prototype | The optional Servo 0.7 mode in the unified Blaze executable owns one WebView per tab, normally retains two warm WebViews; pinned background views are exempt from eviction and may raise the count; it evicts older unpinned page heaps, and restores evicted tabs from saved URLs. It supports mouse/keyboard/wheel input, HTTP(S)-only navigation, address editing, back/forward history and Ctrl+T/Ctrl+W/Ctrl+Tab. It opens a second native window using the same executable, not an inline surface in the main shell; downloads and live permission prompts remain pending. |
| 4. GPU/media | Partial | wgpu accelerates native controls and bounded WASM canvas/widget output. Servo's optional GStreamer backend is compiled in, but GPU decode selection, GPU-resident frame presentation and YouTube 4K compatibility are not benchmarked. |
| 5. Tabs/memory | Partial engine integration | Normally two warm Servo WebViews are retained; pinned background views are exempt and may raise the count; older unpinned views are dropped and restored from URL metadata. Hidden views may continue timers/media/network work; true background freezing and RAM telemetry remain. |
| 6. Privacy/storage | Prototype implemented | Argon2/ChaCha20-Poly1305 encrypted Notes and browser profile, schema-v2 migration, bookmarks, origin-scoped permissions, capped history, and last-session tab/pin restore. A SHA-256 deduplicating quota cache exists. OS keychain, engine permission enforcement, encrypted cookie/cache integration, private-mode teardown, and formal review remain. |
| 7. Workspace/WASM apps | Prototype implemented | Code Studio can run compute-only modules and compiled Blaze UI apps; validated text, button, progress and canvas nodes are painted by native eframe/wgpu, with bounded actions. No HTML/DOM/JS/WebGPU imports. Office interchange, bounded formulas, notes, vault and WebRTC text channel are present; full Office fidelity, language services, contacts/signaling and media calling remain. |

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

The optional standalone Servo host creates one WebView per open tab and retains at most two warm views (active plus one LRU warm view). Older unpinned WebViews are dropped to release page resources and rebuilt from URL/title metadata on revisit; their DOM/JS/history state is not retained. Hidden retained views are only hidden from the compositor and may continue JS timers, network/media work. Remaining browser work is the inline main-shell compositor bridge, OS/process isolation, real background freezing, downloads, and live permission prompts. Validate builds and real page compatibility on supported operating systems.

### Hardware video and performance

Benchmark Servo's GStreamer decode path on Intel/AMD/NVIDIA and modest integrated GPUs. The new probe script reports installed decoder elements but does not prove which decoder Servo selected. Verify 4K AV1, VP9 and H.264 separately; keep frames GPU-resident and report CPU RSS, GPU memory, video-engine utilization, dropped frames, tab-switch time and browser process count. A target of 50 live tabs all decoding 4K simultaneously below 1 GiB is not a safe engineering guarantee.

### Office and communication

The office bridge now has DOCX/XLSX/PPTX interchange tests and a CSV-backed Excel-style formula evaluator with a 10,000-cell range cap. Markdown conversion is lossy for advanced layout, charts, theme and media objects; full Office fidelity, formula persistence/recalculation, cell-grid virtualization, and slide rendering remain. Messenger uses WebRTC data channels and blocks sending until the SDP SHA-256 DTLS fingerprint matches a value the user enters after independent verification. It now accepts explicit STUN/TURN URLs and credentials and offers TURN relay-only ICE to hide host/reflexive candidate IPs from the remote peer (the TURN operator can still see the client IP). Long-term identity, a signaling/contacts service, secure credential storage, and message history persistence remain. Calling still needs device capture, media tracks, echo cancellation, NAT traversal, and permission integration.

### Security

The vault and encrypted profile primitives have authentication/tamper-detection tests and the profile supports encrypted session metadata. This is not a complete password manager or encrypted browser cookie store. Add OS keychain support, secure file permissions, key rotation/recovery, process/profile isolation, a content security policy, and independent security review. The Wasm runner has no host imports by default and applies fuel and linear-memory limits. The UI app ABI is intentionally not an HTML runtime and paints only validated bounded nodes through the native UI renderer. It still needs process isolation, richer audited capabilities and a proper app package lifecycle before third-party apps are enabled. The content cache is a separate module and still needs concurrency coordination and browser-request integration.

See [VIDEO_ACCELERATION.md](VIDEO_ACCELERATION.md) for the 4K/GStreamer probe and benchmark requirements.
