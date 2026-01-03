#!/bin/bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# General Public License as published by the Free Software Foundation, version
# 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.
#

# Check that the builtin table in builtins.rs hasn't been modified in ways
# that would invalidate compiled programs (removals, reorders, renames).
# Adding new builtins in reserved slots is allowed.
#
# Usage: ./check-builtin-stability.sh [base_ref]
#   base_ref: git ref to compare against (default: origin/main)

set -e

BASE_REF="${1:-}"
if [ -z "$BASE_REF" ]; then
  if [ -n "${GITHUB_BASE_REF:-}" ]; then
    BASE_REF="origin/${GITHUB_BASE_REF}"
  elif [ -n "${FORGEJO_BASE_REF:-}" ]; then
    BASE_REF="origin/${FORGEJO_BASE_REF}"
  elif [ -n "${GITEA_BASE_REF:-}" ]; then
    BASE_REF="origin/${GITEA_BASE_REF}"
  else
    BASE_REF="origin/main"
  fi
fi
BUILTINS_FILE="crates/common/src/builtins.rs"
GROUP_SIZE=256

extract_builtins() {
  python3 - "$1" <<'PY'
import re
import sys

path = sys.argv[1]
group_size = int(sys.argv[2])

names = []
count = 0
pending = False

with open(path, "r", encoding="utf-8") as handle:
    for line in handle:
        if pending:
            match = re.search(r'"([^"]+)"', line)
            if match:
                names.append(match.group(1))
                count += 1
                pending = False
            continue

        match = re.search(r'mk_builtin\(\s*"([^"]+)"', line)
        if match:
            names.append(match.group(1))
            count += 1
            continue

        if "mk_builtin(" in line:
            pending = True
            continue

        if "pad_group(" in line:
            missing = group_size - count
            if missing < 0:
                missing = 0
            names.extend(["<reserved>"] * missing)
            count = 0

if count:
    missing = group_size - count
    if missing > 0:
        names.extend(["<reserved>"] * missing)

print("\n".join(names))
PY
}

extract_builtins "$BUILTINS_FILE" "$GROUP_SIZE" > /tmp/pr_builtins.txt

git show "${BASE_REF}:${BUILTINS_FILE}" > /tmp/base_builtins.rs 2>/dev/null || {
  echo "Could not fetch base branch file, skipping check"
  exit 0
}
extract_builtins /tmp/base_builtins.rs "$GROUP_SIZE" > /tmp/base_builtins.txt

BASE_COUNT=$(wc -l < /tmp/base_builtins.txt)
PR_COUNT=$(wc -l < /tmp/pr_builtins.txt)

echo "Base branch has $BASE_COUNT builtins"
echo "PR branch has $PR_COUNT builtins"

ERRORS=""
line_num=0
ADDED=""
while IFS= read -r base_builtin; do
  line_num=$((line_num + 1))
  pr_builtin=$(sed -n "${line_num}p" /tmp/pr_builtins.txt)
  if [ "$base_builtin" = "<reserved>" ]; then
    if [ -n "$pr_builtin" ] && [ "$pr_builtin" != "<reserved>" ]; then
      ADDED="${ADDED}Builtin #${line_num} added as '${pr_builtin}'\n"
    fi
    continue
  fi

  if [ "$base_builtin" != "$pr_builtin" ]; then
    if [ -z "$pr_builtin" ] || [ "$pr_builtin" = "<reserved>" ]; then
      ERRORS="${ERRORS}ERROR: Builtin #${line_num} '${base_builtin}' was REMOVED\n"
    else
      ERRORS="${ERRORS}ERROR: Builtin #${line_num} changed from '${base_builtin}' to '${pr_builtin}'\n"
    fi
  fi
done < /tmp/base_builtins.txt

if [ -n "$ADDED" ]; then
  echo ""
  echo "New builtins added in reserved slots:"
  printf "$ADDED"
fi

if [ -n "$ERRORS" ]; then
  echo ""
  echo "=========================================="
  echo "BUILTIN TABLE STABILITY CHECK FAILED"
  echo "=========================================="
  echo ""
  echo "Modifying existing builtins will invalidate compiled programs!"
  echo ""
  echo "If intentional, you must:"
  echo "  1. Increment the DB version"
  echo "  2. Add migration in crates/db/src/provider/fjall_migration.rs"
  echo ""
  printf "$ERRORS"
  exit 1
fi

echo ""
echo "Builtin table stability check PASSED"
