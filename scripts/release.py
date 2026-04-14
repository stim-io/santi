#!/usr/bin/env python3

from __future__ import annotations

import sys

import release_beta


def main(argv: list[str]) -> int:
    if any("beta" in argument for argument in argv):
        return release_beta.main()

    raise SystemExit("santi currently supports beta packaging only; use scripts/release_beta.py")


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
