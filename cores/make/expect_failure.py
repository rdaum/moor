# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com>
# This program is free software under the GNU General Public License, version 3.
# It is distributed without any warranty. See <https://www.gnu.org/licenses/>.

"""Report an expected command failure without printing a misleading failure log."""

import argparse
import re
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True)
    parser.add_argument("--match", required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command[:1] == ["--"]:
        command = command[1:]
    if not command:
        parser.error("a command is required")
    try:
        result = subprocess.run(
            command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, timeout=600,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        print(f"FAIL {args.name}: {error}", file=sys.stderr)
        return 1
    if result.returncode > 0 and re.search(args.match, result.stdout):
        print(f"PASS {args.name} (expected failure)")
        return 0
    print(result.stdout, end="", file=sys.stderr)
    print(
        f"FAIL {args.name}: expected a failure matching {args.match!r}; "
        f"exit status was {result.returncode}", file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
