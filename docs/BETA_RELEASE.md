# Blaze v0.0.1 Beta

**Release channel:** prerelease  
**Version:** `0.0.1-beta`  
**Target artifact:** Linux x86_64  
**Executable:** one `blaze` binary

## What is bundled

The beta archive includes the resolved Cargo lockfile and one executable for the native workspace shell with Servo 0.7 embedded in the Browser panel. The browser runs inside the same native window; there is no separate Servo host process.

The shell uses eframe/egui/wgpu. Servo renders through a software offscreen context; Blaze reads RGBA frames back and uploads them to an egui texture. This is not a zero-copy GPU bridge and may limit rendering performance.

## Build from source

Install the Linux graphics, Fontconfig/FreeType and GStreamer development packages listed in `.github/workflows/release.yml`, then run:

```sh
cargo build --release -p blaze-desktop --features servo-browser --bin blaze
```

Check the version and launch the shell:

```sh
./target/release/blaze --version
./target/release/blaze
```

## Runtime dependencies

The packaged executable still relies on host graphics libraries, Fontconfig/FreeType and GStreamer runtime libraries/plugins. On Ubuntu/Debian, install the matching runtime packages, including the GStreamer base/good/bad plugin sets. GPU decode depends on the installed driver and the stream codec. See [VIDEO_ACCELERATION.md](VIDEO_ACCELERATION.md).

## Beta limitations

- Each tab owns a separate WebView and navigation history, but tabs currently share one Servo engine and storage profile; cookies/site storage are not isolated per tab.
- The page renderer currently supports a limited set of browser shortcuts and no download manager.
- Suspended background WebViews are dropped and recreated from URL metadata when revisited. Hidden pages are not guaranteed to stop background JavaScript or media.
- Hardware decoding and 4K YouTube playback are unverified on real hardware.
- Calling and full Office editing/fidelity are not complete.
- The beta is a development preview. Back up important data and do not treat it as a production browser or secure communications product.

## Release workflow

The GitHub Actions workflow `.github/workflows/release.yml` builds the single binary and publishes a GitHub prerelease when the `beta/v0.0.1-beta` candidate branch is pushed (the workflow creates the `v0.0.1-beta` tag), or when that tag is pushed directly. It also supports manual dispatch from the Actions tab.
