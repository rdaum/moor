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

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)
OUT_DIR=${OUT_DIR:-"${REPO_ROOT}/target/perf/activation"}
SCENARIO=${SCENARIO:-nested_simple}
ITERS=${ITERS:-1500000}
WARMUP=${WARMUP:-100000}
STAT_REPEATS=${STAT_REPEATS:-3}
PERF_BIN=${PERF_BIN:-perf}

EVENTS=${PERF_EVENTS:-cycles,instructions,branches,branch-misses,cache-references,cache-misses,L1-dcache-loads,L1-dcache-load-misses,LLC-loads,LLC-load-misses,dTLB-loads,dTLB-load-misses,stalled-cycles-frontend,stalled-cycles-backend}

mkdir -p "${OUT_DIR}"

cd "${REPO_ROOT}"
cargo build --release -p moor-kernel --bin activation_profile

BIN="${REPO_ROOT}/target/release/activation_profile"
CMD=("${BIN}" --scenario "${SCENARIO}" --iters "${ITERS}" --warmup "${WARMUP}")

printf 'Running: %q ' "${CMD[@]}"
printf '\n'

"${PERF_BIN}" stat \
    --all-user \
    -r "${STAT_REPEATS}" \
    -d \
    -d \
    -d \
    -x, \
    -o "${OUT_DIR}/stat-default.csv" \
    -- "${CMD[@]}"

"${PERF_BIN}" stat \
    --all-user \
    -r "${STAT_REPEATS}" \
    -x, \
    -e "${EVENTS}" \
    -o "${OUT_DIR}/stat-events.csv" \
    -- "${CMD[@]}"

"${PERF_BIN}" record \
    --all-user \
    -g \
    --call-graph dwarf,16384 \
    -o "${OUT_DIR}/perf.data" \
    -- "${CMD[@]}"

"${PERF_BIN}" report \
    --stdio \
    --no-children \
    --sort overhead,symbol,dso \
    --percent-limit 0.2 \
    -i "${OUT_DIR}/perf.data" > "${OUT_DIR}/report-self.txt"

"${PERF_BIN}" report \
    --stdio \
    --sort overhead,symbol,dso \
    --percent-limit 0.2 \
    -i "${OUT_DIR}/perf.data" > "${OUT_DIR}/report-inclusive.txt"

echo "Wrote perf outputs to ${OUT_DIR}:"
echo "  stat-default.csv"
echo "  stat-events.csv"
echo "  perf.data"
echo "  report-self.txt"
echo "  report-inclusive.txt"
