# Blaze architecture

## 1. Native shell and engine boundary

The desktop shell is planned as a native Rust application. The browser engine is a separate subsystem behind a narrow adapter interface. Servo is the first engine candidate because it is Rust-based and designed for embeddability. Before committing to it, validate current platform support, web compatibility, media playback, accessibility, and licensing.

Blaze will not use Electron or a system WebView as its permanent browser implementation. A real browser engine is required: a custom tab strip or URL bar alone is not a browser.

## 2. Rendering and video

- GPU compositing through a cross-platform graphics abstraction such as wgpu, subject to the selected engine's integration points.
- Prefer platform hardware video decode (VA-API on supported Linux systems, VideoToolbox on macOS, and DXVA/D3D paths on Windows).
- Keep decoded frames in GPU-friendly surfaces where possible to avoid copies.
- Bound decoder queues and pause/throttle off-screen media according to user settings.
- Report actual CPU RAM and GPU memory separately.

The 50-tab/50-simultaneous-4K scenario is a stress benchmark, not a guaranteed sub-1-GiB operating point. A realistic low-memory mode prioritizes one active tab, suspends inactive tabs, and restores them on demand.

## 3. Tab and process model

- Isolate sites/origins where the engine supports it.
- Prioritize the foreground tab.
- Throttle timers and background work.
- Suspend eligible tabs after a configurable idle period.
- Persist compact restore metadata rather than serializing live process memory.
- Never suspend a tab with unsaved user data without an explicit recovery mechanism.

## 4. Security and privacy

- Use OS-backed secret storage for encryption keys.
- Encrypt user content and sensitive browser data at rest; use authenticated encryption and versioned key derivation.
- Keep private browsing state ephemeral and delete it when its private session ends.
- Apply least-privilege permissions for camera, microphone, location, notifications, clipboard, and downloads.
- Use process isolation and sandboxing; WebAssembly is an extension/app sandbox, not a replacement for browser origin security.
- Avoid custom cryptography. Use reviewed libraries and platform security facilities.
- Do not promise end-to-end encryption for messaging until identity, key verification, recovery, multi-device behavior, and protocol review are implemented.

## 5. Integrated workspace

The browser workspace will expose first-party modules with shared identity and permission surfaces:

- Code Studio: editor, language services, terminal integration, and WASM extension runtime.
- Documents, spreadsheets, and presentations: interoperable file formats and import/export.
- Notes: local-first encrypted storage and search.
- Messenger and calling: explicit permissions, encrypted transport, and clear contact identity.

These are separate product modules, not features that should be tightly coupled to the renderer process.

## 6. Storage optimization

- Content-addressed cache with quotas and eviction.
- Deduplicate immutable assets where safe.
- Separate cache from user-authored data.
- Compress snapshots and structured records; avoid recompressing already-compressed media.
- Use SQLite or another transactional store for metadata, with a migration strategy.
- Encrypt sensitive records and keep key material outside the database.
- Provide per-site storage inspection and deletion.

## 7. Milestones

1. Workspace and core policy types.
2. Native shell, navigation UI, and engine adapter spike.
3. Real page rendering, navigation, downloads, and permissions.
4. GPU compositing and hardware media decode on target platforms.
5. Tab scheduler, suspension, crash recovery, and memory benchmarks.
6. Encrypted storage, private sessions, and security review.
7. Workspace apps and extension runtime.
8. Compatibility, accessibility, packaging, and release automation.
