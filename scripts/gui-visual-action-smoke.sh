#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}"

find "$ROOT/qml" -name '*.qml' -print0 | sort -z | xargs -0 qmllint -I "$ROOT/qml"

qmltestrunner \
  -input "$ROOT/tests/qml" \
  -import "$ROOT/qml" \
  -import "$ROOT/qml/components" \
  -import "$ROOT/qml/components/common" \
  -import "$ROOT/qml/components/settings" \
  -import "$ROOT/qml/theme"

if [[ "${RUN_GUI_BINARY_SMOKE:-0}" == "1" ]]; then
  if [[ -z "${LOGOS_BLOCKCHAIN_CIRCUITS:-}" ]]; then
    echo "RUN_GUI_BINARY_SMOKE=1 requires LOGOS_BLOCKCHAIN_CIRCUITS" >&2
    exit 2
  fi
  RISC0_SKIP_BUILD=1 timeout "${GUI_SMOKE_TIMEOUT:-10s}" cargo run -- gui
fi
