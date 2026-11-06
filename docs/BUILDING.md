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

The shell includes the workspace panels, profile and Notes vault UI, CSV
spreadsheet editor, formula bar, and Messenger UI. Its browser address bar
validates addresses and manages local tab metadata; the shell does not itself
embed the web renderer yet.

## Servo browser host

The Servo dependency is opt-in so that users who only need the workspace shell
do not compile the full browser engine. Build both binaries:

```sh
cargo build --release -p blaze-desktop --features servo-host --bins
```

The resulting executables are:

- `target/release/blaze` — main native workspace.
- `target/release/blaze-servo` — standalone Servo browser window.

Launch the browser host directly:

```sh
./target/release/blaze-servo https://example.com
```

Or launch the main shell and use the **Open in Servo** button after building
with the `servo-host` feature. Keep both executables in the same directory so
the shell can locate `blaze-servo`. The shell passes its tab URL/title list,
pinned flags, active index, and site-permission snapshot to the Servo process.
This restores the selected page and other tab URLs but not live JavaScript or
renderer state.

### Servo keyboard shortcuts

| Shortcut | Action |
|---|---|
| Ctrl+L | Edit the current address in the window title bar; Enter navigates, Esc cancels |
| Ctrl+R | Reload current page |
| Ctrl+T | Open a new blank tab |
| Ctrl+W | Close the current tab (pinned tabs are protected; last tab becomes blank) |
| Ctrl+P | Toggle the current tab's pinned state |
| Ctrl+Tab | Next tab |
| Ctrl+Shift+Tab | Previous tab |
| Alt+Left | Back |
| Alt+Right | Forward |

The current Servo host keeps tab URL/title metadata and reuses one active
WebView. Switching tabs reloads the selected URL; it does not preserve each
tab's scroll position, JavaScript heap, renderer process, or live media state.
This is intentionally lighter than keeping every hidden renderer active.

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

The 50-tab / 50-simultaneous-4K / under-1-GiB request remains a stress target,
not a guaranteed resource budget. The present Servo window uses one active
WebView and lightweight URL metadata for tabs; profile tab restoration restores
URLs and pin flags, not live renderer state.


## Build the example WASM app

Install the WebAssembly target and build the standalone sample:

```sh
rustup target add wasm32-unknown-unknown
cargo build --manifest-path examples/wasm-webapp/Cargo.toml --target wasm32-unknown-unknown --release
```

Open `target/wasm32-unknown-unknown/release/blaze_wasm_counter.wasm` in Code Studio by setting the module path and choosing **Run WASM web app**. The sample uses Blaze's bounded UI ABI; it is not an arbitrary HTML/JavaScript website. The native host draws its UI/canvas primitives through eframe/wgpu.

For the optional Linux media backend, see [VIDEO_ACCELERATION.md](VIDEO_ACCELERATION.md) and run `scripts/probe-gstreamer-hwdecode.sh` after installing the relevant GStreamer plugins and GPU driver.
