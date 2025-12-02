#!/bin/bash
# Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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
# Adding new builtins at the end is allowed.
#
# Usage: ./check-builtin-stability.sh [base_ref]
#   base_ref: git ref to compare against (default: origin/main)

set -e

BASE_REF="${1:-origin/main}"
BUILTINS_FILE="crates/common/src/builtins.rs"

extract_builtins() {
  # Handle both single-line: mk_builtin("name", ...)
  # and multi-line: mk_builtin(\n    "name", ...)
  grep -ozP 'mk_builtin\(\s*"\K[^"]+' "$1" | tr '\0' '\n' || true
}

extract_builtins "$BUILTINS_FILE" > /tmp/pr_builtins.txt

git show "${BASE_REF}:${BUILTINS_FILE}" > /tmp/base_builtins.rs 2>/dev/null || {
  echo "Could not fetch base branch file, skipping check"
  exit 0
}
extract_builtins /tmp/base_builtins.rs > /tmp/base_builtins.txt

BASE_COUNT=$(wc -l < /tmp/base_builtins.txt)
PR_COUNT=$(wc -l < /tmp/pr_builtins.txt)

echo "Base branch has $BASE_COUNT builtins"
echo "PR branch has $PR_COUNT builtins"

ERRORS=""
line_num=0
while IFS= read -r base_builtin; do
  line_num=$((line_num + 1))
  pr_builtin=$(sed -n "${line_num}p" /tmp/pr_builtins.txt)
  if [ "$base_builtin" != "$pr_builtin" ]; then
    if [ -z "$pr_builtin" ]; then
      ERRORS="${ERRORS}ERROR: Builtin #${line_num} '${base_builtin}' was REMOVED\n"
    else
      ERRORS="${ERRORS}ERROR: Builtin #${line_num} changed from '${base_builtin}' to '${pr_builtin}'\n"
    fi
  fi
done < /tmp/base_builtins.txt

if [ "$PR_COUNT" -gt "$BASE_COUNT" ]; then
  echo ""
  echo "New builtins added at the end:"
  tail -n $((PR_COUNT - BASE_COUNT)) /tmp/pr_builtins.txt
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
