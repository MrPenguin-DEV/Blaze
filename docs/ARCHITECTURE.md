# Blaze architecture

## Native application shell

The desktop application is written in Rust and uses eframe/egui with its wgpu renderer for the native UI. This is not an embedded WebView: the browser content renderer must be a real browser engine behind the blaze-browser adapter. UI GPU acceleration and webpage GPU compositing are distinct systems.

## Browser engine boundary

Servo is the leading engine candidate. The current adapter contract normalizes user navigation and defines the boundary where engine navigation, lifecycle, and capability reporting belong. A standalone Servo host is connected as an optional binary. Each tab owns a WebView; the host normally keeps two warm views (active and one recently used unpinned view); pinned background views are exempt and can raise the count, while other unpinned views are dropped and restoring them from URL metadata. This gives quick return to a warm tab and bounds renderer count, but dropping a view loses its in-memory page state. This host remains separate from the main wgpu UI.

Servo's embedding sequence includes:
1. An EventLoopWaker that schedules Servo event-loop work.
2. A Servo instance built with ServoBuilder.
3. A RenderingContext implementation.
4. A WebViewBuilder and WebViewDelegate that paints and presents frames.
5. Input forwarding and event-loop servicing.

Servo's embedding interface is still a host-integration project, not a drop-in egui widget. Its rendering context and event loop must be integrated with the desktop host. The integration must be validated against the selected Servo revision and the supported desktop platforms.

## Rendering and video

- The shell uses wgpu for its own UI rendering.
- Web content rendering must be negotiated with the selected engine's supported rendering context. Do not assume egui's wgpu textures can directly consume Servo output; the current Servo host is still a separate window/context.
- Prefer platform hardware video decode (VA-API, NVDEC, AMF, Vulkan Video, V4L2, VideoToolbox or the matching platform API) subject to the Servo media backend and installed plugins.
- Keep decoded frames in GPU-friendly surfaces where possible to avoid copies.
- Keep decoder queues bounded and avoid redundant GPU-to-CPU frame copies. The current host does not promise that off-screen media/timers are paused.
- Report CPU RAM and GPU memory separately.

The 50-tab/50-simultaneous-4K scenario is a stress benchmark, not a guaranteed sub-1-GiB operating point. Low-memory mode should prioritize the foreground tab, suspend eligible background tabs, and restore them on demand.

## Tabs and resource policy

blaze-core provides a scheduler with foreground, background, and suspended states. It protects the active tab, pinned tabs, and tabs marked dirty from automatic suspension. The configured live-tab ceiling is a policy limit, not OS-level memory enforcement; the real engine must expose process and memory telemetry before enforcement can be made precise.

## Privacy and security

- blaze-vault derives a key from a user password with Argon2 and encrypts files using ChaCha20-Poly1305 authenticated encryption.
- The encrypted envelope stores a salt and nonce, but never the password or raw key.
- Wrong passwords and modified ciphertext fail authentication.
- The desktop prototype can encrypt and restore its Notes panel content. OS keychain integration, encrypted browser profiles, key rotation/recovery, hardened file permissions, and independent security review remain pending.
- Private browsing must be ephemeral and cleared when its private session ends.
- Apply least-privilege site permissions for camera, microphone, location, notifications, clipboard, and downloads.
- Browser origin isolation and OS process sandboxing are still required.
- Messenger verifies the remote SDP's SHA-256 DTLS fingerprint against a value independently confirmed by the user. It supports explicit ICE servers and an optional relay-only candidate policy. Relay-only conceals host/server-reflexive candidates from the remote peer but not from the TURN provider. Fingerprint comparison is a manual peer check, not a durable identity/key-management system. Do not promise complete end-to-end identity, recovery, multi-device security, or protocol review.
- Avoid custom cryptography.

## Integrated workspace and Wasm

blaze-workspace contains Rust data models and format helpers for:
- Code Studio source buffers.
- Markdown-first documents and notes.
- CSV spreadsheet data.
- Outline-based presentations.
- Message drafts and call lifecycle state.
- Capability declarations for future extensions.

blaze-wasm executes both compute-only exports and a narrow Blaze UI-app ABI. UI apps export memory, `blaze_render(width, height) -> i64` and optionally `blaze_action(action_id)`. The guest returns bounded JSON nodes; the desktop host validates them and paints text/buttons/progress/canvas primitives through egui/wgpu. The Wasmi runtime is an interpreter, not a native JIT or direct WebGPU bridge. Its empty linker exposes no WASI, browser, file, network, clipboard or device imports, with fuel, stack/recursion and linear-memory/store limits. A production app system still needs process isolation and audited capabilities.

The Sheets desktop panel now renders an editable CSV-backed cell grid with row/column insertion, a formula bar and a compact numeric preview chart. The grid remains CSV-backed; this is not yet a full workbook engine with formatting, cell styles, persistent formula graphs, or chart objects. The desktop panels are starter surfaces. Full language servers, terminal isolation, DOCX/XLSX/PPTX interoperability, sync, contacts, calling, and richer extension APIs remain to be implemented.

## Storage optimization

- blaze-cache stores re-creatable resources under SHA-256 keys and deduplicates identical bytes.
- The cache rebuilds a lightweight index from shard directories and applies a byte quota with least-recently-used eviction.
- The cache must contain only re-creatable browser assets, never user notes, credentials, cookies, or documents.
- The cache crate is not yet connected to browser request/response loading.
- Keep encrypted user-authored data separate from disposable cache content.
- Add cache integrity checks, concurrent-writer coordination, and performance benchmarks before production use.
- Compress snapshots and structured records; do not recompress already-compressed media.
- Use a transactional metadata store and versioned migrations before persisting browser profiles.
- Provide per-site storage inspection and deletion.

## Milestones

1. Workspace and core policy types — implemented.
2. Native desktop shell — prototype implemented.
3. Browser navigation policy and engine adapter contract — implemented; actual page rendering, downloads, and permissions pending.
4. Native UI GPU rendering — uses wgpu; webpage GPU compositing and hardware media decode pending.
5. Tab scheduler and memory policy — implemented at policy level; renderer controls and RAM benchmarks pending.
6. Encrypted local file primitive and Notes vault UI — implemented; profile/keychain integration and security review pending.
7. Workspace panel foundations, compute-only Wasm runner, and quota cache — implemented; full Office formats, extension management/capabilities, and network services pending.
8. Compatibility, accessibility, packaging, and release automation — pending.
