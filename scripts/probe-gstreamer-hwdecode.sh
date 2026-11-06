#!/usr/bin/env bash
set -euo pipefail

if ! command -v gst-inspect-1.0 >/dev/null 2>&1; then
  echo "GStreamer tools are missing. Install gstreamer1.0-tools and platform decoder plugins."
  exit 2
fi

echo "Blaze video acceleration probe"
echo "Installed elements only; this does not prove Servo selected a decoder."
echo

if command -v vainfo >/dev/null 2>&1; then
  echo "VA-API:"
  vainfo 2>&1 | sed -n '1,35p' || true
else
  echo "VA-API: vainfo not installed (optional)"
fi

if command -v nvidia-smi >/dev/null 2>&1; then
  echo
  echo "NVIDIA:"
  nvidia-smi --query-gpu=name,driver_version --format=csv,noheader 2>/dev/null || true
fi

echo
echo "GStreamer GL elements (texture-backed presentation path):"
for element in glsinkbin glupload glcolorconvert glimagesink; do
  if gst-inspect-1.0 "$element" >/dev/null 2>&1; then
    printf '  [available] %s\n' "$element"
  fi
done

echo
echo "Common GStreamer hardware decoder elements:"
found=0
for decoder in \
  vah264dec vah265dec vaav1dec vavp9dec vavp8dec \
  nvh264dec nvh265dec nvav1dec nvvp9dec \
  amfh264dec amfh265dec v4l2h264dec v4l2h265dec vtdec; do
  if gst-inspect-1.0 "$decoder" >/dev/null 2>&1; then
    printf '  [available] %s\n' "$decoder"
    found=1
  fi
done

if [[ "$found" -eq 0 ]]; then
  echo "  No common hardware decoder element found."
  echo "  Install the driver and the matching GStreamer plugin for your GPU."
fi

echo
echo "YouTube may deliver AV1, VP9 or H.264 depending on negotiation."
echo "For smooth 4K, the exact selected codec/profile needs a working hardware decoder."
echo "Keep decoded frames in GPU memory where possible to avoid expensive CPU copies."
echo "This probe does not benchmark playback or modify plugin ranks."
