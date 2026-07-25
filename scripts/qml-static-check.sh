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
  -import "$ROOT/qml/features/settings/controls" \
  -import "$ROOT/qml/theme"
