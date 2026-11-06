# Blaze v0.0.1 Beta

**Release channel:** prerelease  
**Version:** `0.0.1-beta`  
**Target artifact:** Linux x86_64  
**Executable:** one `blaze` binary

## What is bundled

The beta package contains one executable for the native workspace shell and its optional Servo 0.7 browser mode. The browser mode is started from the same executable with `--servo-host`; the shell's **Open in Servo** action invokes that mode automatically.

The browser view still uses a second native window and its own OpenGL rendering context. The shell uses eframe/egui/wgpu. This release does not claim an inline browser surface or a direct Servo-to-wgpu zero-copy texture bridge.

## Build from source

Install the Linux graphics, Fontconfig/FreeType and GStreamer development packages listed in `.github/workflows/release.yml`, then run:

```sh
cargo build --release -p blaze-desktop --features servo-host --bin blaze
```

Launch the shell:

```sh
./target/release/blaze
```

Launch the browser mode directly:

```sh
./target/release/blaze --servo-host https://example.com
```

## Runtime dependencies

The packaged executable still relies on host graphics libraries, Fontconfig/FreeType and GStreamer runtime libraries/plugins. On Ubuntu/Debian, install the matching runtime packages, including the GStreamer base/good/bad plugin sets. GPU decode depends on the installed driver and the stream codec. See [VIDEO_ACCELERATION.md](VIDEO_ACCELERATION.md).

## Beta limitations

- Browser content is not rendered inside the shell's tab area.
- The page renderer currently supports a limited set of browser shortcuts and no download manager.
- The warm retained Servo view budget is two for unpinned tabs. Pinned pages can increase that number; hidden pages are not guaranteed to stop background JavaScript or media.
- Hardware decoding and 4K YouTube playback are unverified on real hardware.
- Calling and full Office editing/fidelity are not complete.
- The beta is a development preview. Back up important data and do not treat it as a production browser or secure communications product.

## Release workflow

The GitHub Actions workflow `.github/workflows/release.yml` builds the single binary and publishes a GitHub prerelease when the `beta/v0.0.1-beta` candidate branch is pushed (the workflow creates the `v0.0.1-beta` tag), or when that tag is pushed directly. It also supports manual dispatch from the Actions tab.
