#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

section() {
  printf '\n== %s ==\n' "$1"
}

kv() {
  printf '%-24s %s\n' "$1" "$2"
}

bool_check() {
  local label="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    kv "$label" "yes"
  else
    kv "$label" "no"
  fi
}

run_timed() {
  local label="$1"
  local log_path="$2"
  shift 2
  local started finished elapsed
  started="$(date +%s)"
  printf '[run] %s\n' "$label"
  if "$@" >"$log_path" 2>&1; then
    finished="$(date +%s)"
    elapsed="$((finished - started))"
    printf '[ok]  %s (%ss)\n' "$label" "$elapsed"
  else
    finished="$(date +%s)"
    elapsed="$((finished - started))"
    printf '[fail] %s (%ss)\n' "$label" "$elapsed"
    printf '       log: %s\n' "$log_path"
    return 1
  fi
}

gst_plugin_support() {
  if ! command -v gst-inspect-1.0 >/dev/null 2>&1; then
    echo "unknown"
    return
  fi

  if gst-inspect-1.0 pipewiresrc >/dev/null 2>&1 \
    && gst-inspect-1.0 ximagesrc >/dev/null 2>&1 \
    && gst-inspect-1.0 x264enc >/dev/null 2>&1 \
    && gst-inspect-1.0 mp4mux >/dev/null 2>&1 \
    && gst-inspect-1.0 pulsesrc >/dev/null 2>&1 \
    && gst-inspect-1.0 level >/dev/null 2>&1; then
    echo "available"
  else
    echo "missing"
  fi
}

section "Environment"
kv "date" "$(date --iso-8601=seconds)"
kv "uname" "$(uname -srmo)"
kv "cwd" "$ROOT_DIR"
kv "DISPLAY" "${DISPLAY:-<unset>}"
kv "WAYLAND_DISPLAY" "${WAYLAND_DISPLAY:-<unset>}"
kv "XDG_SESSION_TYPE" "${XDG_SESSION_TYPE:-<unset>}"

section "Runtime Tools"
bool_check "gst-launch-1.0" command -v gst-launch-1.0
bool_check "gst-inspect-1.0" command -v gst-inspect-1.0
bool_check "pactl" command -v pactl
bool_check "pw-top" command -v pw-top
bool_check "xrandr" command -v xrandr
bool_check "xwininfo" command -v xwininfo
kv "gst native path" "$(gst_plugin_support)"

section "Audio Inputs"
if command -v pactl >/dev/null 2>&1; then
  pactl list short sources | sed 's/^/  /'
else
  echo "  pactl unavailable"
fi

section "Build And Tests"
run_timed "cargo check" /tmp/record-screen-linux-check.log cargo check
run_timed "capture-linux unit tests" /tmp/record-screen-linux-unit.log cargo test -p capture-linux -- --nocapture

if [[ -n "${DISPLAY:-}" ]]; then
  run_timed "linux smoke test (no mic)" /tmp/record-screen-linux-smoke.log cargo test -p capture-linux linux_smoke_recording_creates_output_file -- --ignored --nocapture
  run_timed "linux smoke test (with mic)" /tmp/record-screen-linux-smoke-mic.log env RECORD_SCREEN_SMOKE_WITH_MIC=1 cargo test -p capture-linux linux_smoke_recording_creates_output_file -- --ignored --nocapture
else
  echo "[skip] linux smoke tests require DISPLAY"
fi

section "Pulse Probe"
if command -v gst-launch-1.0 >/dev/null 2>&1; then
  run_timed "gstreamer pulse probe" /tmp/record-screen-linux-pulse.log \
    gst-launch-1.0 -q pulsesrc device=default num-buffers=16 ! level interval=100000000 post-messages=true ! fakesink || true
else
  echo "[skip] gst-launch-1.0 unavailable"
fi

section "Summary"
if [[ "${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
  echo "- session: Wayland"
else
  echo "- session: X11 or non-Wayland"
fi
echo "- x11 smoke coverage: available when DISPLAY is set"
echo "- pure wayland runtime coverage: requires logging into a Wayland-only session"
echo "- detailed logs:"
echo "  /tmp/record-screen-linux-check.log"
echo "  /tmp/record-screen-linux-unit.log"
echo "  /tmp/record-screen-linux-smoke.log"
echo "  /tmp/record-screen-linux-smoke-mic.log"
echo "  /tmp/record-screen-linux-pulse.log"
