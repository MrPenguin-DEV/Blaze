# Changelog

## v0.0.1-beta — 2026-11-06

Initial Linux-first beta of the Rust-native Blaze workspace.

### Included

- One `blaze` executable for the desktop shell and optional Servo browser mode.
- Servo 0.7 browser engine with a GStreamer media backend, HTTP(S)-only navigation, URL/title/pinned-tab restore metadata, and a two-warm-view budget for unpinned tabs.
- Native eframe/egui/wgpu workspace panels.
- Resource-limited WebAssembly compute modules and the Blaze UI-app ABI with bounded native-rendered widgets/canvas primitives.
- Markdown/DOCX, CSV/XLSX and outline/PPTX interchange prototypes; bounded spreadsheet formulas.
- Encrypted local profile/vault primitives, WebRTC text data channel, manual signaling and explicit fingerprint verification.

### Known beta limitations

- The Servo page surface opens in a second native window and OpenGL context; it is not embedded inline in the eframe shell.
- Pinned background pages are exempt from tab eviction; hidden pages can continue background JavaScript, media and network work.
- The GStreamer hardware-decoding path and 4K YouTube playback have not been certified or benchmarked across GPUs.
- Downloads, real-time audio/video calling, Office fidelity, contacts/signaling service and an app marketplace are not complete.
- Linux x86_64 is the only artifact targeted by the beta workflow. Other platforms require their own native builds and validation.
