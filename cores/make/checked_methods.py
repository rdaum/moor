# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com>
# This program is free software under the GNU General Public License, version 3.
# It is distributed without any warranty. See <https://www.gnu.org/licenses/>.

import argparse
from pathlib import Path
import re
import subprocess
import sys

parser = argparse.ArgumentParser(description="Run method tests and reject empty discovery.")
parser.add_argument("--log", type=Path)
parser.add_argument("command", nargs=argparse.REMAINDER)
args = parser.parse_args()
if not args.command:
    parser.error("a compiler command is required")

# moorc currently accepts an empty discovered or filtered method selection.
try:
    result = subprocess.run(
        args.command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=600
    )
except subprocess.TimeoutExpired:
    sys.exit("FAIL: method process exceeded 600 seconds")
if args.log:
    args.log.write_text(result.stdout)
print(result.stdout, end="")
if not re.search(r"Found [1-9][0-9]* tests", result.stdout):
    sys.exit("FAIL: method suite discovered no tests")
if re.search(r"Filtered tests from [0-9]+ to 0", result.stdout):
    sys.exit("FAIL: filter selected no tests")
sys.exit(result.returncode)
