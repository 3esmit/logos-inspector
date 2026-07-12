#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}"

run_qml_static_checks() {
  find "$ROOT/qml" -name '*.qml' -print0 | sort -z | xargs -0 qmllint -I "$ROOT/qml"
}

run_qml_tests() {
  qmltestrunner \
    -input "$ROOT/tests/qml" \
    -import "$ROOT/qml" \
    -import "$ROOT/qml/components" \
    -import "$ROOT/qml/components/common" \
    -import "$ROOT/qml/features/settings/controls" \
    -import "$ROOT/qml/theme"
}

run_zone_visual_smoke() {
  "$ROOT/scripts/zones-visual-smoke.sh"
}

run_gui_launcher_smoke() {
  if [[ -z "${LOGOS_BLOCKCHAIN_CIRCUITS:-}" ]]; then
    echo "RUN_GUI_BINARY_SMOKE=1 requires LOGOS_BLOCKCHAIN_CIRCUITS" >&2
    exit 2
  fi
  RISC0_SKIP_BUILD=1 timeout "${GUI_SMOKE_TIMEOUT:-10s}" cargo run -- gui
}

run_standalone_gui_smoke() {
  LOGOS_INSPECTOR_QML_DIR="$ROOT/qml" \
    RISC0_SKIP_BUILD=1 \
    timeout "${GUI_SMOKE_TIMEOUT:-10s}" cargo run -p logos-inspector-standalone-gui
}

run_qml_static_checks
run_qml_tests
run_zone_visual_smoke

if [[ "${RUN_GUI_BINARY_SMOKE:-0}" == "1" ]]; then
  run_gui_launcher_smoke
fi

if [[ "${RUN_STANDALONE_GUI_SMOKE:-0}" == "1" ]]; then
  run_standalone_gui_smoke
fi
