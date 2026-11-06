# Video acceleration and 4K playback

Blaze does not use WebAssembly to decode or render YouTube video. A Wasm guest cannot bypass the browser engine's media pipeline or manufacture hardware codec support. The correct path is:

1. Servo parses the page and runs its web APIs.
2. Servo's optional GStreamer media backend negotiates a decoder for the stream codec.
3. A matching driver/plugin performs hardware decoding (where supported).
4. Decoded frames stay in GPU-compatible memory as long as possible.
5. Servo/WebRender composites the page; the Blaze shell separately uses eframe/wgpu for its native controls.

GStreamer documents hardware decoding through VA-API, NVIDIA NVDEC, AMD AMF, Vulkan Video, VideoToolbox and V4L2. It also explains why copying decoded frames from GPU memory to system memory can remove much of the performance benefit. Hardware decoder availability and automatic selection depend on the device, driver, plugin and pipeline ranks: https://gstreamer.freedesktop.org/documentation/tutorials/playback/hardware-accelerated-video-decoding.html

Servo 0.7's GStreamer media backend also includes the Unix GL renderer component. Its `RenderUnix` implementation uses GStreamer's `glsinkbin` around an appsink and maps shared frames as OpenGL texture IDs. That is a native GPU presentation route—not WASM rendering—and it still depends on a working GL/EGL stack and the actual Servo pipeline selecting it. It does not mean the separate Blaze shell and Servo window already share one wgpu texture or that every codec/driver combination is zero-copy.

## Check a Linux machine

Install the GStreamer inspection tool and the plugin packages appropriate to the GPU and distribution, then run:

```sh
chmod +x scripts/probe-gstreamer-hwdecode.sh
./scripts/probe-gstreamer-hwdecode.sh
```

The probe reports common installed decoder elements, GStreamer GL elements, and optional VA-API/NVIDIA diagnostics. It does not prove that Servo selected a hardware decoder, that the plugin can decode the exact stream profile, or that YouTube playback works.

A YouTube stream may use AV1, VP9 or H.264 depending on negotiation. For smooth 4K, the exact codec/profile selected by the site needs hardware decode support. Verify actual behavior with a known 4K stream while observing CPU use, GPU/video-engine use, dropped frames and GPU memory. Do not blindly force decoder ranks: GStreamer notes that some hardware decoders are deliberately ranked lower because of driver defects.

## Current Blaze implementation status

- The optional `servo-host` build enables Servo 0.7's `media-gstreamer` backend.
- The native shell's app UI uses eframe/wgpu. Servo renders browser content in its own rendering context in a separate window; there is not yet a zero-copy Servo-to-wgpu texture bridge.
- The browser host normally retains two warm WebViews (active plus one unpinned tab); pinned background views are exempt and may raise the count. Older unpinned views are dropped and recreated from their saved URL when revisited. This preserves fast switching for a warm tab without keeping an unlimited number of renderer/page heaps.
- Hiding a WebView changes its compositor visibility; it is not a guarantee that JavaScript timers, network requests, media or service workers stop. Renderer dropping is the current hard resource reclamation path.
- YouTube compatibility, GPU decode selection, frame-copy behavior and low-end hardware performance have not been benchmarked in this repository.

## Performance target

Optimize the real foreground workload first: hardware decode for the active 4K video, GPU-resident frames, no unnecessary frame copies, and few live background renderers. Report CPU RSS, GPU memory, video-engine utilization, dropped frames, startup time and tab-switch latency independently. A claim such as 50 concurrent 4K streams below 1 GiB is not a realistic universal target.
