#!/usr/bin/env bash
# One command for the Linux GPU matrix (REPORT.md §8): on a machine with an Intel, AMD or NVIDIA
# GPU, prints the GPU, the session, the adapter GPUI picks, startup, memory, idle CPU and the
# drag benchmark, then the same drag in WebKitGTK when python3-gi with WebKit2 4.1 is installed.
#
#   scripts/linux-gpu-check.sh path/to/dashboard-gpui-spike
#
# Run it from spikes/gpui in a desktop session (Wayland or X11). Paste the output into the
# report. It uses its own data directory and leaves nothing behind.
set -uo pipefail
export LC_ALL=C
bin=$(realpath "${1:?path to dashboard-gpui-spike}")
here=$(dirname "$(realpath "$0")")
export SPIKE_DATA_DIR=$(mktemp -d)
trap 'rm -rf "$SPIKE_DATA_DIR"' EXIT

echo "== machine"
lspci 2>/dev/null | grep -iE 'vga|3d|display' || echo "lspci: not available"
grep -m1 PRETTY_NAME /etc/os-release
echo "session: ${XDG_SESSION_TYPE:-?} wayland=${WAYLAND_DISPLAY:-} x11=${DISPLAY:-}"
command -v glxinfo >/dev/null && glxinfo -B 2>/dev/null | grep -E 'renderer string|core profile version'
command -v vulkaninfo >/dev/null && vulkaninfo --summary 2>/dev/null | grep -E 'deviceName|driverName|deviceType' || echo "vulkaninfo: not installed"

"$bin" --dev-approve >/dev/null 2>&1

run() { # label, then environment assignments
  local label=$1; shift
  echo "== GPUI, $label"
  env "$@" RUST_LOG=warn,gpui_wgpu=info SPIKE_EXIT_AFTER_MS=4000 "$bin" 2>&1 | grep -E 'Selected GPU adapter|failed:|first frame|cannot start' | sed 's/^\[[^]]*\] //'
  if [ "$label" = wayland ]; then
    env "$@" WAIT_FOR_LOG='first frame' "$here/measure-linux.sh" "gpui-$label" x 5 10 -- "$bin"
  else
    env "$@" "$here/measure-linux.sh" "gpui-$label" 'Dashboard (GPUI spike)' 5 10 -- "$bin"
  fi
  env "$@" SPIKE_DRAG_BENCH=1 timeout 60 "$bin" 2>&1 | grep -o 'drag bench.*'
}
[ -n "${WAYLAND_DISPLAY:-}" ] && run wayland
[ -n "${DISPLAY:-}" ] && run x11 -u WAYLAND_DISPLAY
[ -n "${DISPLAY:-}" ] && run x11-gl-only -u WAYLAND_DISPLAY VK_DRIVER_FILES=/nonexistent.json VK_ICD_FILENAMES=/nonexistent.json

if python3 -c "import gi; gi.require_version('WebKit2', '4.1')" 2>/dev/null; then
  echo "== WebKitGTK 4.1 (Tauri's engine on Linux)"
  timeout 60 python3 -I "$here/webkitgtk-bench.py" --drag 2>&1 | grep -E 'first frame|drag bench'
else
  echo "== WebKitGTK: python3-gi with WebKit2 4.1 not installed (sudo apt install gir1.2-webkit2-4.1)"
fi
