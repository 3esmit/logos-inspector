#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT/tests/qml/fixtures/ZonesVisualFixture.qml"
OUTPUT_DIR="${ZONE_VISUAL_OUTPUT_DIR:-$(mktemp -d)}"
KEEP_OUTPUT="${ZONE_VISUAL_OUTPUT_DIR:+1}"

cleanup() {
  if [[ -z "$KEEP_OUTPUT" ]]; then
    rm -rf "$OUTPUT_DIR"
  fi
}
trap cleanup EXIT

mkdir -p "$OUTPUT_DIR"

capture() {
  local width="$1"
  local height="$2"
  local tab="$3"
  local output="$OUTPUT_DIR/zones-${width}x${height}-${tab}.png"

  QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}" \
    QT_FATAL_WARNINGS=1 \
    timeout 15s qml \
      -I "$ROOT/qml" \
      -I "$ROOT/tests/qml/fixtures" \
      "$FIXTURE" -- \
      --width "$width" \
      --height "$height" \
      --tab "$tab" \
      --out "$output"

  test -s "$output"
  file "$output" | grep -q "${width} x ${height}"

  if command -v identify >/dev/null 2>&1; then
    local colors
    colors="$(identify -format '%k' "$output")"
    if (( colors < 16 )); then
      echo "Zones visual fixture is unexpectedly blank: $output" >&2
      return 1
    fi
  fi
}

capture 1024 720 overview
capture 1024 720 l2
capture 1440 900 evidence
capture 1440 900 l2
capture 1440 900 l2-block
capture 1920 1080 l2-trace
capture 1920 1080 sources

if [[ -n "$KEEP_OUTPUT" ]]; then
  printf 'Zones visual screenshots: %s\n' "$OUTPUT_DIR"
fi
