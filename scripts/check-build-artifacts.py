#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

from build_artifacts import BuildArtifacts


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    errors = BuildArtifacts(ROOT).validate()
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
