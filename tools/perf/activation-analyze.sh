#!/usr/bin/env bash
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

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)
OUT_DIR=${1:-"${REPO_ROOT}/target/perf/activation"}
PERF_BIN=${PERF_BIN:-perf}
SYMBOL=${SYMBOL:-'moor_kernel::vm::activation::Activation::for_call'}

DATA_FILE="${OUT_DIR}/perf.data"
if [[ ! -f "${DATA_FILE}" ]]; then
    echo "perf data not found: ${DATA_FILE}" >&2
    exit 1
fi

"${PERF_BIN}" report \
    --stdio \
    --sort overhead,symbol,dso \
    --percent-limit 0.1 \
    -i "${DATA_FILE}" > "${OUT_DIR}/report-top.txt"

"${PERF_BIN}" report \
    --stdio \
    --children \
    --sort overhead,symbol,dso \
    --percent-limit 0.1 \
    -i "${DATA_FILE}" > "${OUT_DIR}/report-children.txt"

if "${PERF_BIN}" annotate --stdio --symbol "${SYMBOL}" -i "${DATA_FILE}" > "${OUT_DIR}/annotate-for_call.txt" 2>/dev/null; then
    echo "Wrote annotate-for_call.txt for symbol ${SYMBOL}"
else
    echo "Could not annotate symbol '${SYMBOL}' (symbol may be optimized out or renamed)." >&2
fi

echo "Wrote analysis outputs to ${OUT_DIR}:"
echo "  report-top.txt"
echo "  report-children.txt"
echo "  annotate-for_call.txt (if symbol resolved)"
