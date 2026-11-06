# Blaze architecture

## Native application shell

The desktop application is written in Rust and uses eframe/egui with the wgpu renderer for its native UI. This is not an embedded WebView: the browser content renderer must be a real browser engine behind the blaze-browser adapter. UI GPU acceleration and webpage GPU compositing are distinct systems.

## Browser engine boundary

Servo is the leading engine candidate. The current adapter contract normalizes user navigation and defines the boundary where engine navigation, lifecycle, and capability reporting belong. The production adapter is not connected yet.

Servo's documented embedding sequence includes:
1. An EventLoopWaker that schedules Servo::spin_event_loop.
2. A Servo instance built with ServoBuilder.
3. A RenderingContext implementation.
4. A WebViewBuilder and WebViewDelegate that paints and presents frames.
5. Input forwarding and event-loop servicing.

Servo's current embedding guide describes this interface but calls the embedding chapter a work in progress. Its rendering context and event loop must be integrated with the desktop host; do not assume that a widget containing an address bar is a browser engine.

## Rendering and video

- The shell uses wgpu for its own UI rendering.
- Web content rendering must be negotiated with the selected engine's supported rendering context. A separate GL/surfman context may be needed; do not assume egui's wgpu textures can directly consume Servo output.
- Prefer platform hardware video decode (VA-API on supported Linux systems, VideoToolbox on macOS, and DXVA/D3D paths on Windows), subject to the engine's media backend.
- Keep decoded frames in GPU-friendly surfaces where possible to avoid copies.
- Bound decoder queues and pause or throttle off-screen media.
- Report CPU RAM and GPU memory separately.

The 50-tab/50-simultaneous-4K scenario is a stress benchmark, not a guaranteed sub-1-GiB operating point. Low-memory mode should prioritize the foreground tab, suspend eligible background tabs, and restore them on demand.

## Tabs and resource policy

blaze-core provides a scheduler with foreground, background, and suspended states. It protects the active tab, pinned tabs, and tabs marked dirty from automatic suspension. The configured live-tab ceiling is a policy limit, not OS-level memory enforcement; the real engine must expose process and memory telemetry before enforcement can be made precise.

## Privacy, security, and storage

- blaze-vault derives a key from a user password with Argon2 and encrypts files using ChaCha20-Poly1305 authenticated encryption.
- The encrypted envelope stores a salt and nonce, but never the password or raw key.
- Wrong passwords and modified ciphertext fail authentication.
- Keep OS keychain integration as a future option; passwords should not be stored in plaintext.
- Private browsing must be ephemeral and cleared when its private session ends.
- Apply least-privilege site permissions for camera, microphone, location, notifications, clipboard, and downloads.
- Browser origin isolation and process sandboxing are still required. WebAssembly extensions are not a replacement for browser security boundaries.
- Do not promise end-to-end encryption for messaging until identity, key verification, recovery, multi-device behavior, and protocol review are complete.
- Avoid custom cryptography.

## Integrated workspace

blaze-workspace contains Rust data models and format helpers for:
- Code Studio source buffers.
- Markdown-first documents and notes.
- CSV spreadsheet data.
- Outline-based presentations.
- Message drafts and call lifecycle state.
- Capability declarations for future WASM extensions.

The desktop panels are prototype surfaces. Full language servers, terminal isolation, DOCX/XLSX/PPTX interoperability, sync, contacts, calling, and a WASM runtime still need dedicated implementation and security review.

## Storage optimization

- Separate user-authored data from cache.
- Encrypt user-authored notes and sensitive records before writing them.
- Add content-addressed cache quotas and eviction when the engine is integrated.
- Compress snapshots and structured records; do not recompress already-compressed media.
- Use a transactional metadata store and versioned migrations before persisting browser profiles.
- Provide per-site storage inspection and deletion.

## Milestones

1. Workspace and core policy types — implemented.
2. Native desktop shell and browser adapter contract — implemented as a prototype.
3. Production web rendering, navigation, downloads, and permissions — pending Servo adapter.
4. Web-content GPU compositing and hardware media decode — pending engine/rendering backend integration.
5. Tab scheduler and memory policy — implemented at policy level; engine process controls and benchmarks pending.
6. Encrypted local file primitive — implemented; profile/keychain integration and security review pending.
7. Workspace app foundations — implemented as starter panels and data models; full office formats, WASM execution, and messaging/calling services pending.
8. Compatibility, accessibility, packaging, and release automation — pending.
