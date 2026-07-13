#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="${GUI_NAV_ARTIFACT_DIR:-$(mktemp -d)}"
KEEP_ARTIFACTS="${GUI_NAV_ARTIFACT_DIR:+1}"
XVFB_PID=""
APP_PID=""

cleanup() {
  local status=$?
  set +e
  [[ -z "$APP_PID" ]] || kill "$APP_PID" 2>/dev/null
  [[ -z "$XVFB_PID" ]] || kill "$XVFB_PID" 2>/dev/null
  if (( status == 0 )) && [[ -z "$KEEP_ARTIFACTS" ]]; then
    rm -rf "$ARTIFACT_DIR"
  else
    printf 'GUI navigation artifacts: %s\n' "$ARTIFACT_DIR" >&2
  fi
}
trap cleanup EXIT

for command in cargo python3 Xvfb xwininfo import convert compare identify rg; do
  command -v "$command" >/dev/null 2>&1 || {
    printf 'GUI navigation E2E requires %s\n' "$command" >&2
    exit 2
  }
done

mkdir -p "$ARTIFACT_DIR"

if [[ "${GUI_NAV_SKIP_BUILD:-0}" != "1" ]]; then
  (
    cd "$ROOT"
    RISC0_SKIP_BUILD=1 cargo build -p logos-inspector-standalone-gui
  )
fi

TARGET_DIR="$(
  cd "$ROOT"
  cargo metadata --no-deps --format-version 1 \
    | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])'
)"
GUI_BINARY="${GUI_NAV_BINARY:-$TARGET_DIR/debug/logos-inspector-standalone-gui}"
[[ -x "$GUI_BINARY" ]] || {
  printf 'Standalone GUI binary is unavailable: %s\n' "$GUI_BINARY" >&2
  exit 2
}

DISPLAY_NUMBER=""
for candidate in $(seq 90 119); do
  if [[ ! -e "/tmp/.X11-unix/X$candidate" ]]; then
    DISPLAY_NUMBER="$candidate"
    break
  fi
done
[[ -n "$DISPLAY_NUMBER" ]] || {
  printf 'No free X display is available\n' >&2
  exit 2
}

DISPLAY_VALUE=":$DISPLAY_NUMBER"
Xvfb "$DISPLAY_VALUE" -screen 0 1440x900x24 -ac -nolisten tcp \
  >"$ARTIFACT_DIR/xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 1

DISPLAY="$DISPLAY_VALUE" \
LOGOS_INSPECTOR_QML_DIR="$ROOT/qml" \
QML_DISABLE_DISK_CACHE=1 \
QT_LOGGING_RULES='qt.qml.connections.warning=true;qt.qml.binding.removal.info=true' \
"$GUI_BINARY" >"$ARTIFACT_DIR/app.log" 2>&1 &
APP_PID=$!

WINDOW_ID=""
for _ in $(seq 1 100); do
  kill -0 "$APP_PID" 2>/dev/null || {
    tail -100 "$ARTIFACT_DIR/app.log" >&2
    exit 1
  }
  WINDOW_ID="$(DISPLAY="$DISPLAY_VALUE" xwininfo -root -tree 2>/dev/null \
    | awk '/"Logos Inspector"/ { print $1; exit }')"
  [[ -z "$WINDOW_ID" ]] || break
  sleep 0.1
done
[[ -n "$WINDOW_ID" ]] || {
  printf 'Standalone GUI window did not open\n' >&2
  exit 1
}
sleep 3

DISPLAY="$DISPLAY_VALUE" import -window "$WINDOW_ID" "$ARTIFACT_DIR/before.png"

DISPLAY="$DISPLAY_VALUE" \
GUI_NAV_CLICK_X="${GUI_NAV_CLICK_X:-122}" \
GUI_NAV_CLICK_Y="${GUI_NAV_CLICK_Y:-165}" \
python3 - <<'PY'
import ctypes
import os
import time

x11 = ctypes.CDLL("libX11.so.6")
xtst = ctypes.CDLL("libXtst.so.6")
x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XFlush.argtypes = [ctypes.c_void_p]
x11.XFlush.restype = ctypes.c_int
x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
x11.XCloseDisplay.restype = ctypes.c_int
xtst.XTestFakeMotionEvent.argtypes = [
    ctypes.c_void_p,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_ulong,
]
xtst.XTestFakeMotionEvent.restype = ctypes.c_int
xtst.XTestFakeButtonEvent.argtypes = [
    ctypes.c_void_p,
    ctypes.c_uint,
    ctypes.c_int,
    ctypes.c_ulong,
]
xtst.XTestFakeButtonEvent.restype = ctypes.c_int

display = x11.XOpenDisplay(os.environ["DISPLAY"].encode())
if not display:
    raise SystemExit("cannot open X display")

x = int(os.environ["GUI_NAV_CLICK_X"])
y = int(os.environ["GUI_NAV_CLICK_Y"])
xtst.XTestFakeMotionEvent(display, -1, x, y, 0)
xtst.XTestFakeButtonEvent(display, 1, 1, 0)
xtst.XTestFakeButtonEvent(display, 1, 0, 0)
x11.XFlush(display)
time.sleep(0.1)
x11.XCloseDisplay(display)
PY

convert "$ARTIFACT_DIR/before.png" -crop 880x150+240+75 +repage \
  "$ARTIFACT_DIR/before-main.png"

CHANGED_PIXELS=0
CONTENT_COLORS=0
RENDER_ATTEMPT=0
for RENDER_ATTEMPT in $(seq 1 "${GUI_NAV_MAX_ATTEMPTS:-50}"); do
  kill -0 "$APP_PID" 2>/dev/null || {
    tail -100 "$ARTIFACT_DIR/app.log" >&2
    exit 1
  }
  DISPLAY="$DISPLAY_VALUE" import -window "$WINDOW_ID" "$ARTIFACT_DIR/after.png"
  convert "$ARTIFACT_DIR/after.png" -crop 880x150+240+75 +repage \
    "$ARTIFACT_DIR/after-main.png"
  METRIC_OUTPUT="$(compare -metric AE \
    "$ARTIFACT_DIR/before-main.png" "$ARTIFACT_DIR/after-main.png" null: 2>&1 || true)"
  CHANGED_PIXELS="${METRIC_OUTPUT%% *}"
  CONTENT_COLORS="$(identify -format '%k' "$ARTIFACT_DIR/after-main.png")"
  if [[ "$CHANGED_PIXELS" =~ ^[0-9]+$ ]] \
      && [[ "$CONTENT_COLORS" =~ ^[0-9]+$ ]] \
      && (( CHANGED_PIXELS >= 500 && CONTENT_COLORS >= 16 )); then
    break
  fi
  sleep "${GUI_NAV_POLL_INTERVAL:-0.2}"
done

printf 'Main content changed pixels: %s\n' "$CHANGED_PIXELS"
printf 'Main content colors: %s\n' "$CONTENT_COLORS"
printf 'Render attempts: %s\n' "$RENDER_ATTEMPT"

if [[ ! "$CHANGED_PIXELS" =~ ^[0-9]+$ ]] || (( CHANGED_PIXELS < 500 )); then
  printf 'Blocks navigation did not replace main content\n' >&2
  exit 1
fi

if [[ ! "$CONTENT_COLORS" =~ ^[0-9]+$ ]] || (( CONTENT_COLORS < 16 )); then
  printf 'Blocks navigation rendered blank main content\n' >&2
  exit 1
fi

if rg -q 'TypeError|ReferenceError|Cannot read property|Unable to assign|Detected function .* no signal' \
    "$ARTIFACT_DIR/app.log"; then
  printf 'Standalone GUI emitted a fatal QML diagnostic\n' >&2
  rg -n 'TypeError|ReferenceError|Cannot read property|Unable to assign|Detected function .* no signal' \
    "$ARTIFACT_DIR/app.log" >&2
  exit 1
fi

printf 'Standalone GUI navigation E2E passed\n'
