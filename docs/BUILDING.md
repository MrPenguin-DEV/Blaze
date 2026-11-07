# Building Blaze

Blaze is a Rust workspace. The default desktop application builds the native
wgpu-rendered shell. The optional Servo browser host is a separate native
window and has a significantly larger native dependency tree.

## Toolchain

Use stable Rust 1.95 or newer. The eframe 0.37 desktop dependency requires
Rust 1.95. Install the native build tools and graphics/window-system development
packages for your operating system.

## Default desktop shell

From the repository root:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo run -p blaze-desktop
```

The shell includes workspace panels, profile and Notes vault UI, CSV spreadsheet editor, formula bar, and Messenger UI. Its browser address bar manages tab metadata and can launch the bundled Servo mode.

## Unified Blaze binary with Servo mode

The Servo 0.7 browser engine is an optional feature linked into the same
`blaze` executable as the eframe/egui workspace shell. Build it with:

```sh
cargo build --release -p blaze-desktop --features servo-browser --bin blaze
```

Launch the shell:

```sh
./target/release/blaze
```

Launch the browser mode directly:

```sh
./target/release/blaze --servo-browser https://example.com
```

The shell's **Open in Servo** action launches the browser mode from that same
executable. It opens a second native window with Servo's own OpenGL context;
the page is not yet embedded inside the shell's egui/wgpu tab area.

### Browser shortcuts

| Shortcut | Action |
|---|---|
| Ctrl+L | Edit address in the window title bar; Enter navigates, Esc cancels |
| Ctrl+R | Reload current page |
| Ctrl+T | Open a new blank tab |
| Ctrl+W | Close current tab (pinned tabs are protected) |
| Ctrl+P | Toggle the current tab's pinned state |
| Ctrl+Tab | Next tab |
| Ctrl+Shift+Tab | Previous tab |
| Alt+Left | Back |
| Alt+Right | Forward |

The Servo browser mode maintains one WebView per live tab, with a warm-view
budget of two for unpinned tabs. Older unpinned views are dropped and restored
from URL/title metadata when revisited; their JavaScript heap, scroll position,
media state and page history are not retained after eviction. Pinned views are
exempt and can raise the active renderer count. Hidden retained tabs are not
guaranteed to stop JavaScript timers, network requests or media playback.

## Linux media dependencies

The optional Servo build enables the GStreamer media backend. On a Debian or
Ubuntu-derived distribution, a typical starting point is:

```sh
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config cmake clang \
  libasound2-dev libudev-dev \
  libx11-dev libx11-xcb-dev libxcb1-dev libxcb-render0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libwayland-dev libxrandr-dev \
  libxcursor-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  libgstreamer-plugins-bad1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
  gstreamer1.0-libav gstreamer1.0-gl
```

The exact package names and required plugins vary by distribution and driver.
GStreamer being enabled does not itself guarantee hardware decode: actual
playback depends on installed plugins, the GPU/driver stack, and the media
pipeline used by the selected Servo revision.

## Current rendering boundary

The main application UI uses wgpu. Servo currently owns a separate rendering
context and browser window. An in-process compositor bridge from Servo frames
to the main egui/wgpu tab surface is not included. Downloads, process-per-site
isolation, profile-backed permission prompts, and integrated browser tabs are
still engineering tasks.

Messenger's Network / NAT settings accept optional STUN/TURN URLs, TURN username and credential. Relay-only requires at least one TURN/TURNS URL. It hides host and server-reflexive candidates from the remote peer; it does not hide the connecting IP from the TURN provider. Credentials remain in process memory and are not written to the encrypted profile.

The 50-tab / 50-simultaneous-4K / under-1-GiB request remains a stress target, not a guaranteed resource budget. The Servo mode retains two warm views for unpinned tabs and restores evicted pages from URL metadata, not live renderer state.


## Build the example WASM app

Install the WebAssembly target and build the standalone sample:

```sh
rustup target add wasm32-unknown-unknown
cargo build --manifest-path examples/wasm-webapp/Cargo.toml --target wasm32-unknown-unknown --release
```

Open `target/wasm32-unknown-unknown/release/blaze_wasm_counter.wasm` in Code Studio by setting the module path and choosing **Run WASM web app**. The sample uses Blaze's bounded UI ABI; it is not an arbitrary HTML/JavaScript website. The native host draws its UI/canvas primitives through eframe/wgpu.

For the optional Linux media backend, see [VIDEO_ACCELERATION.md](VIDEO_ACCELERATION.md) and run `scripts/probe-gstreamer-hwdecode.sh` after installing the relevant GStreamer plugins and GPU driver.


The browser is bundled as a mode of the same executable. To launch it directly:

```sh
cargo run -p blaze-desktop --features servo-browser --bin blaze -- --servo-browser https://example.com
```
