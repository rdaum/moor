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

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)
MODE=top
DRY_RUN=0
OUT_DIR=${OUT_DIR:-"${REPO_ROOT}/target/bench/activation-breakdown"}
CARGO_BIN=${CARGO_BIN:-cargo}

usage() {
    cat <<'EOF'
Usage: tools/perf/activation-bench-breakdown.sh [--mode top|nested] [--dry-run]

Runs activation microbenches (bench-utils) and computes a simple ns/op attribution:
  frame input prep -> environment init/copy -> frame scaffolding
  + activation-only input prep -> activation assembly

Options:
  --mode top|nested   Use top-level or nested activation path (default: top)
  --dry-run           Print commands without executing them
  -h, --help          Show this help text

Environment:
  OUT_DIR             Directory for raw logs (default: target/bench/activation-breakdown)
  CARGO_BIN           Cargo binary (default: cargo)
  MOOR_BENCH_PIN_CORE Optional core pin override passed through to bench-utils
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mode)
            shift
            if [[ $# -eq 0 ]]; then
                echo "missing value for --mode" >&2
                exit 1
            fi
            MODE="$1"
            ;;
        --dry-run)
            DRY_RUN=1
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
    shift
done

case "${MODE}" in
    top)
        INPUT_FRAME_BENCH="activation_input_clone_frame_top_simple"
        INPUT_ACT_BENCH="activation_input_clone_activation_top_simple"
        ENV_BENCH="activation_environment_top_level_simple"
        FRAME_BENCH="activation_frame_top_level_simple"
        DIRECT_ASSEMBLY_BENCH="activation_assembly_top_level_simple_direct"
        DIRECT_ASSEMBLY_OVERHEAD_BENCH="activation_assembly_top_level_simple_overhead"
        ACT_BENCH="activation_for_call_top_level_simple"
        ;;
    nested)
        INPUT_FRAME_BENCH="activation_input_clone_frame_nested_simple"
        INPUT_ACT_BENCH="activation_input_clone_activation_nested_simple"
        ENV_BENCH="activation_environment_nested_simple"
        FRAME_BENCH="activation_frame_nested_simple"
        DIRECT_ASSEMBLY_BENCH="activation_assembly_nested_simple_direct"
        DIRECT_ASSEMBLY_OVERHEAD_BENCH="activation_assembly_nested_simple_overhead"
        ACT_BENCH="activation_for_call_nested_simple"
        ;;
    *)
        echo "invalid mode '${MODE}', expected top|nested" >&2
        exit 1
        ;;
esac

mkdir -p "${OUT_DIR}"

run_bench() {
    local bench_name=$1
    local log_file="${OUT_DIR}/${MODE}-${bench_name}.log"

    echo
    echo "==> ${bench_name}"
    echo "log: ${log_file}"

    local cmd=("${CARGO_BIN}" bench -p moor-kernel --bench activation_bench -- "${bench_name}")
    if [[ ${DRY_RUN} -eq 1 ]]; then
        echo "dry-run: ${cmd[*]}"
        return 0
    fi

    (
        cd "${REPO_ROOT}"
        "${cmd[@]}"
    ) 2>&1 | tee "${log_file}"
}

extract_ns() {
    local bench_name=$1
    local log_file=$2
    awk -v bench="${bench_name}" '
        index($0, "📈 Results for " bench ":") { in_section = 1; next }
        in_section && match($0, /([0-9]+([.][0-9]+)?) ns\/op/, m) { print m[1]; exit }
    ' "${log_file}"
}

for bench in "${INPUT_FRAME_BENCH}" "${INPUT_ACT_BENCH}" "${ENV_BENCH}" "${FRAME_BENCH}" "${DIRECT_ASSEMBLY_BENCH}" "${DIRECT_ASSEMBLY_OVERHEAD_BENCH}" "${ACT_BENCH}"; do
    run_bench "${bench}"
done

if [[ ${DRY_RUN} -eq 1 ]]; then
    exit 0
fi

INPUT_FRAME_NS=$(extract_ns "${INPUT_FRAME_BENCH}" "${OUT_DIR}/${MODE}-${INPUT_FRAME_BENCH}.log")
INPUT_ACT_NS=$(extract_ns "${INPUT_ACT_BENCH}" "${OUT_DIR}/${MODE}-${INPUT_ACT_BENCH}.log")
ENV_NS=$(extract_ns "${ENV_BENCH}" "${OUT_DIR}/${MODE}-${ENV_BENCH}.log")
FRAME_NS=$(extract_ns "${FRAME_BENCH}" "${OUT_DIR}/${MODE}-${FRAME_BENCH}.log")
DIRECT_ASSEMBLY_NS=$(extract_ns "${DIRECT_ASSEMBLY_BENCH}" "${OUT_DIR}/${MODE}-${DIRECT_ASSEMBLY_BENCH}.log")
DIRECT_ASSEMBLY_OVERHEAD_NS=$(extract_ns "${DIRECT_ASSEMBLY_OVERHEAD_BENCH}" "${OUT_DIR}/${MODE}-${DIRECT_ASSEMBLY_OVERHEAD_BENCH}.log")
ACT_NS=$(extract_ns "${ACT_BENCH}" "${OUT_DIR}/${MODE}-${ACT_BENCH}.log")

for pair in \
    "input_frame:${INPUT_FRAME_NS}" \
    "input_activation:${INPUT_ACT_NS}" \
    "environment:${ENV_NS}" \
    "frame:${FRAME_NS}" \
    "direct_assembly:${DIRECT_ASSEMBLY_NS}" \
    "direct_assembly_overhead:${DIRECT_ASSEMBLY_OVERHEAD_NS}" \
    "activation:${ACT_NS}"; do
    key=${pair%%:*}
    value=${pair#*:}
    if [[ -z "${value}" ]]; then
        echo "failed to parse ns/op for ${key}" >&2
        exit 1
    fi
done

echo
echo "=== Raw ns/op (${MODE}) ==="
printf "input_frame_%s_simple:       %s ns/op\n" "${MODE}" "${INPUT_FRAME_NS}"
printf "input_activation_%s_simple:  %s ns/op\n" "${MODE}" "${INPUT_ACT_NS}"
printf "environment_%s_simple:      %s ns/op\n" "${MODE}" "${ENV_NS}"
printf "frame_%s_simple:            %s ns/op\n" "${MODE}" "${FRAME_NS}"
printf "assembly_%s_simple_direct:  %s ns/op\n" "${MODE}" "${DIRECT_ASSEMBLY_NS}"
printf "assembly_%s_overhead:       %s ns/op\n" "${MODE}" "${DIRECT_ASSEMBLY_OVERHEAD_NS}"
printf "activation_%s_simple:       %s ns/op\n" "${MODE}" "${ACT_NS}"

echo
echo "=== Attributed Breakdown (${MODE}) ==="
awk \
    -v input_frame_ns="${INPUT_FRAME_NS}" \
    -v input_act_ns="${INPUT_ACT_NS}" \
    -v env_ns="${ENV_NS}" \
    -v frame_ns="${FRAME_NS}" \
    -v direct_assembly_ns="${DIRECT_ASSEMBLY_NS}" \
    -v direct_assembly_overhead_ns="${DIRECT_ASSEMBLY_OVERHEAD_NS}" \
    -v activation_ns="${ACT_NS}" '
BEGIN {
    total = activation_ns + 0.0
    input_frame = input_frame_ns + 0.0
    input_act = input_act_ns + 0.0
    env_core = (env_ns + 0.0) - input_frame
    frame_scaffold = (frame_ns + 0.0) - (env_ns + 0.0)
    activation_input_extra = input_act - input_frame
    activation_assembly_residual = total - (frame_ns + 0.0) - activation_input_extra
    activation_assembly_direct = direct_assembly_ns + 0.0
    activation_assembly_overhead = direct_assembly_overhead_ns + 0.0
    activation_assembly_direct_corrected = activation_assembly_direct - activation_assembly_overhead

    printf "%-32s %12s %10s\n", "component", "ns/op", "share"
    printf "%-32s %12.2f %9.2f%%\n", "frame input prep", input_frame, (total > 0 ? input_frame * 100.0 / total : 0.0)
    printf "%-32s %12.2f %9.2f%%\n", "environment core", env_core, (total > 0 ? env_core * 100.0 / total : 0.0)
    printf "%-32s %12.2f %9.2f%%\n", "frame scaffolding", frame_scaffold, (total > 0 ? frame_scaffold * 100.0 / total : 0.0)
    printf "%-32s %12.2f %9.2f%%\n", "activation-only input prep", activation_input_extra, (total > 0 ? activation_input_extra * 100.0 / total : 0.0)
    printf "%-32s %12.2f %9.2f%%\n", "activation assembly (direct)", activation_assembly_direct, (total > 0 ? activation_assembly_direct * 100.0 / total : 0.0)
    printf "%-32s %12.2f %9.2f%%\n", "activation assembly overhead", activation_assembly_overhead, (total > 0 ? activation_assembly_overhead * 100.0 / total : 0.0)
    printf "%-32s %12.2f %9.2f%%\n", "activation assembly (corrected)", activation_assembly_direct_corrected, (total > 0 ? activation_assembly_direct_corrected * 100.0 / total : 0.0)
    printf "%-32s %12.2f %9.2f%%\n", "activation assembly (residual)", activation_assembly_residual, (total > 0 ? activation_assembly_residual * 100.0 / total : 0.0)
    printf "%-32s %12.2f %9.2f%%\n", "total", total, 100.0
}'

echo
echo "logs written under: ${OUT_DIR}"
