# Building Blaze

Blaze is a Rust workspace. The desktop shell uses eframe/egui with wgpu.
The optional Servo 0.7 browser engine is embedded in the Browser panel of the
same native window. Its native dependency tree is substantially larger.

## Toolchain

Use stable Rust 1.95 or newer. Install the native build tools and graphics,
window-system, Fontconfig/FreeType, and (for Servo) GStreamer development
packages for your operating system.

## Default desktop shell

From the repository root:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo run -p blaze-desktop
```

The default build contains the workspace shell. To include live web pages,
build with the `servo-browser` feature.

## Single-window Blaze with Servo

Build the workspace and embedded browser into one executable:

```sh
cargo build --release -p blaze-desktop --features servo-browser --bin blaze
./target/release/blaze
```

The Browser panel opens Google by default. Tabs are separate Servo WebViews
with independent navigation histories; they share one Servo engine and storage
profile, so cookies and site storage are not partitioned per tab. Suspended
background WebViews are dropped and recreated from saved URLs when revisited.

### Browser controls

- Back, forward, reload, address/search, bookmark, new tab, and close tab are
  available in the Browser panel.
- Mouse movement, clicks, wheel scrolling, named keys, and text input are
  forwarded from egui to the active WebView.
- Navigation is restricted to HTTP(S) and `about:blank`.

## Rendering and performance

Servo renders into a software offscreen context. Blaze reads each new RGBA
frame back to CPU memory and uploads it into an egui/wgpu texture. This gives
the browser a single-window surface without opening another native window,
but it is not a zero-copy GPU compositor bridge and can affect scrolling and
video performance. A hardware-backed shared-texture path remains future work.

## Linux media dependencies

The `servo-browser` feature enables Servo's GStreamer media backend. On a
Debian or Ubuntu-derived distribution, a typical starting point is:

```sh
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config cmake clang \
  libasound2-dev libudev-dev \
  libx11-dev libx11-xcb-dev libxcb1-dev libxcb-render0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libwayland-dev libxrandr-dev \
  libxcursor-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev \
  libfontconfig1-dev libfreetype6-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  libgstreamer-plugins-bad1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
  gstreamer1.0-libav gstreamer1.0-gl
```

The exact package names and required plugins vary by distribution and driver.
Enabling GStreamer does not guarantee hardware decode; actual playback depends
on the codec, installed plugins, GPU/driver stack, and Servo media pipeline.

## Current browser limits

- Tabs have separate WebViews and histories, but do not yet have per-tab cookie
  or site-storage partitioning.
- Suspended views are recreated from URL metadata; their in-memory DOM, JS heap,
  scroll position, and page history are not retained.
- Hidden views are not guaranteed to stop JavaScript timers, network requests,
  or media playback.
- Downloads, live permission prompts, and a zero-copy GPU compositor bridge
  remain future work.
- The 50-tab / 50-simultaneous-4K / under-1-GiB request is a stress target, not
  a universal resource guarantee.

## Build the example WASM app

Install the WebAssembly target and build the standalone sample:

```sh
rustup target add wasm32-unknown-unknown
cargo build --manifest-path examples/wasm-webapp/Cargo.toml --target wasm32-unknown-unknown --release
```

Open `target/wasm32-unknown-unknown/release/blaze_wasm_counter.wasm` in Code
Studio and choose **Run WASM web app**. The sample uses Blaze's bounded UI ABI;
it is not an arbitrary HTML/JavaScript website. The native host draws its
UI/canvas primitives through eframe/wgpu.

For the media backend, see [VIDEO_ACCELERATION.md](VIDEO_ACCELERATION.md) and
run `scripts/probe-gstreamer-hwdecode.sh` after installing the relevant
GStreamer plugins and GPU driver.
