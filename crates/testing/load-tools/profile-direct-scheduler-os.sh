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

usage() {
    cat <<'EOF'
Usage:
  profile-direct-scheduler-os.sh [options] [-- <extra direct-scheduler-load-test args>]

Runs OS-side profiling for direct-scheduler-load-test with three passes:
  1) perf stat
  2) perf sched record + timehist + latency
  3) offcputime-bpfcc (optional; auto-skipped if unavailable)

Options:
  --concurrency N           Fixed min/max concurrency (default: 8)
  --duration-seconds N      Swamp-mode duration per pass (default: 30)
  --warmup-seconds N        Warmup to ignore before profiling (default: 10)
  --num-verb-iterations N   Inner verb iterations (default: 7000)
  --num-objects N           Number of test objects (default: 1)
  --max-ticks N             fg_ticks/bg_ticks for test DB (default: 1000000000)
  --with-thread-cpu         Extra pass: per-thread CPU usage + HW counters via perf stat
  --with-hotspots           Extra pass: sampled CPU hotspots via perf record/report
  --hw-events LIST          Comma-separated perf events for --with-thread-cpu
                            (default: task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses,stalled-cycles-frontend,stalled-cycles-backend,context-switches,cpu-migrations,page-faults)
  --hotspot-freq N          Sampling frequency for --with-hotspots (default: 999)
  --out-dir PATH            Output directory (default: /tmp/moor-direct-sched-profile-<ts>)
  --build                   Force build step before profiling
  --skip-build              Skip build step
  --skip-offcpu             Skip off-CPU pass
  --no-sudo                 Do not prefix perf/offcpu commands with sudo
  -h, --help                Show this help

Examples:
  ./crates/testing/load-tools/profile-direct-scheduler-os.sh
  ./crates/testing/load-tools/profile-direct-scheduler-os.sh --duration-seconds 120 --concurrency 8
  ./crates/testing/load-tools/profile-direct-scheduler-os.sh -- --debug
EOF
}

CONCURRENCY=8
DURATION_SECONDS=30
WARMUP_SECONDS=10
NUM_VERB_ITERATIONS=7000
NUM_OBJECTS=1
MAX_TICKS=1000000000
WITH_THREAD_CPU=0
WITH_HOTSPOTS=0
THREAD_HW_EVENTS="task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses,stalled-cycles-frontend,stalled-cycles-backend,context-switches,cpu-migrations,page-faults"
HOTSPOT_FREQ=999
SKIP_BUILD=1
SKIP_OFFCPU=0
USE_SUDO=1
OUT_DIR=""
EXTRA_ARGS=()
WORKLOAD_PID=""
ACTIVE_PIDS=()
SCRIPT_FAILED=0

while [[ $# -gt 0 ]]; do
    case "$1" in
    --concurrency)
        CONCURRENCY="${2:?missing value for --concurrency}"
        shift 2
        ;;
    --duration-seconds)
        DURATION_SECONDS="${2:?missing value for --duration-seconds}"
        shift 2
        ;;
    --warmup-seconds)
        WARMUP_SECONDS="${2:?missing value for --warmup-seconds}"
        shift 2
        ;;
    --num-verb-iterations)
        NUM_VERB_ITERATIONS="${2:?missing value for --num-verb-iterations}"
        shift 2
        ;;
    --num-objects)
        NUM_OBJECTS="${2:?missing value for --num-objects}"
        shift 2
        ;;
    --max-ticks)
        MAX_TICKS="${2:?missing value for --max-ticks}"
        shift 2
        ;;
    --with-thread-cpu)
        WITH_THREAD_CPU=1
        shift
        ;;
    --with-hotspots)
        WITH_HOTSPOTS=1
        shift
        ;;
    --hw-events)
        THREAD_HW_EVENTS="${2:?missing value for --hw-events}"
        shift 2
        ;;
    --hotspot-freq)
        HOTSPOT_FREQ="${2:?missing value for --hotspot-freq}"
        shift 2
        ;;
    --out-dir)
        OUT_DIR="${2:?missing value for --out-dir}"
        shift 2
        ;;
    --skip-build)
        SKIP_BUILD=1
        shift
        ;;
    --build)
        SKIP_BUILD=0
        shift
        ;;
    --skip-offcpu)
        SKIP_OFFCPU=1
        shift
        ;;
    --no-sudo)
        USE_SUDO=0
        shift
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    --)
        shift
        EXTRA_ARGS=("$@")
        break
        ;;
    *)
        echo "Unknown argument: $1" >&2
        usage
        exit 2
        ;;
    esac
done

if ! command -v perf >/dev/null 2>&1; then
    echo "perf not found in PATH" >&2
    exit 1
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
cd "${REPO_ROOT}"

if [[ -z "${OUT_DIR}" ]]; then
    OUT_DIR="/tmp/moor-direct-sched-profile-$(date +%Y%m%d-%H%M%S)"
fi
mkdir -p "${OUT_DIR}"

cleanup() {
    for pid in "${ACTIVE_PIDS[@]:-}"; do
        if kill -0 "${pid}" >/dev/null 2>&1; then
            kill "${pid}" >/dev/null 2>&1 || true
        fi
    done

    # Perf commands under sudo can leave root-owned artifacts; hand them back.
    if [[ "${USE_SUDO}" -eq 1 ]] && command -v sudo >/dev/null 2>&1; then
        sudo chown -R "$(id -u)":"$(id -g)" "${OUT_DIR}" >/dev/null 2>&1 || true
    fi

    if [[ "${SCRIPT_FAILED}" -ne 0 ]]; then
        echo "Script failed. Check logs under: ${OUT_DIR}" >&2
    fi
}
trap cleanup EXIT
trap 'SCRIPT_FAILED=1' ERR

if [[ "${USE_SUDO}" -eq 1 ]]; then
    SUDO=(sudo)
else
    SUDO=()
fi

BIN="${REPO_ROOT}/target/release/direct-scheduler-load-test"
TOTAL_SWAMP_SECONDS=$((DURATION_SECONDS + WARMUP_SECONDS))
COMMON_LOAD_ARGS=(
    --swamp-mode
    --min-concurrency "${CONCURRENCY}"
    --max-concurrency "${CONCURRENCY}"
    --num-verb-iterations "${NUM_VERB_ITERATIONS}"
    --num-objects "${NUM_OBJECTS}"
    --max-ticks "${MAX_TICKS}"
)

if [[ "${#EXTRA_ARGS[@]}" -gt 0 ]]; then
    COMMON_LOAD_ARGS+=("${EXTRA_ARGS[@]}")
fi

start_workload_bg() {
    local pass_name="$1"
    local out_csv="${OUT_DIR}/${pass_name}.load-results.csv"
    local stdout_log="${OUT_DIR}/${pass_name}.workload.stdout.log"
    local stderr_log="${OUT_DIR}/${pass_name}.workload.stderr.log"

    local args=(
        "${COMMON_LOAD_ARGS[@]}"
        --swamp-duration-seconds "${TOTAL_SWAMP_SECONDS}"
        --output-file "${out_csv}"
    )
    "${BIN}" "${args[@]}" >"${stdout_log}" 2>"${stderr_log}" &
    WORKLOAD_PID=$!
    ACTIVE_PIDS+=("${WORKLOAD_PID}")
}

remove_active_pid() {
    local pid="$1"
    local kept=()
    for p in "${ACTIVE_PIDS[@]:-}"; do
        if [[ "${p}" != "${pid}" ]]; then
            kept+=("${p}")
        fi
    done
    ACTIVE_PIDS=("${kept[@]}")
}

wait_for_pid_nonfatal() {
    local pid="$1"
    local label="$2"
    set +e
    wait "${pid}"
    local st=$?
    set -e
    remove_active_pid "${pid}"
    if [[ "${st}" -ne 0 ]]; then
        echo "Warning: ${label} exited with status ${st}" >&2
    fi
    return 0
}

run_profiler_nonfatal() {
    local label="$1"
    shift
    set +e
    "$@"
    local st=$?
    set -e
    if [[ "${st}" -ne 0 ]]; then
        echo "Warning: ${label} exited with status ${st}" >&2
    fi
    return 0
}

capture_thread_snapshot() {
    local pid="$1"
    local out_file="$2"

    if [[ ! -d "/proc/${pid}/task" ]]; then
        echo "No /proc task info for pid ${pid}" >"${out_file}"
        return 0
    fi

    {
        echo "pid=${pid}"
        echo "captured_at=$(date --iso-8601=seconds)"
        echo "tid,comm,psr,state"
        for task_path in "/proc/${pid}/task/"*; do
            local tid comm stat_line psr state
            tid="${task_path##*/}"
            if [[ ! -r "${task_path}/comm" || ! -r "${task_path}/stat" ]]; then
                continue
            fi
            comm="$(<"${task_path}/comm")"
            stat_line="$(<"${task_path}/stat")"
            # procfs stat fields:
            # 3=state, 39=processor. We strip "(comm)" first to make fields stable.
            state="$(echo "${stat_line}" | sed -E 's/^[0-9]+ \([^)]*\) //; s/ .*//')"
            psr="$(echo "${stat_line}" | awk '{print $39}')"
            echo "${tid},${comm},${psr},${state}"
        done
    } >"${out_file}"
}

sleep_warmup_or_fail() {
    local pid="$1"
    if [[ "${WARMUP_SECONDS}" -gt 0 ]]; then
        sleep "${WARMUP_SECONDS}"
    fi
    if ! kill -0 "${pid}" >/dev/null 2>&1; then
        echo "Workload process ${pid} exited before profiling window started" >&2
        return 1
    fi
    return 0
}

echo "Output directory: ${OUT_DIR}"
echo "Repository root: ${REPO_ROOT}"
echo "Concurrency: ${CONCURRENCY}"
echo "Warmup per pass: ${WARMUP_SECONDS}s"
echo "Measured duration per pass: ${DURATION_SECONDS}s"
echo "Extra per-thread CPU pass: ${WITH_THREAD_CPU}"
echo "Extra hotspots pass: ${WITH_HOTSPOTS}"
echo "Extra args: ${EXTRA_ARGS[*]:-<none>}"

if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    echo
    echo "[1/4] Building direct-scheduler-load-test (release + debug symbols + frame pointers)..."
    if ! RUSTFLAGS="-C force-frame-pointers=yes" CARGO_PROFILE_RELEASE_DEBUG=1 \
        cargo build \
            --manifest-path crates/testing/load-tools/Cargo.toml \
            --bin direct-scheduler-load-test \
            --release \
            >"${OUT_DIR}/build.stdout.log" 2>"${OUT_DIR}/build.stderr.log"; then
        echo "Build failed. Last lines from build.stderr.log:" >&2
        tail -n 60 "${OUT_DIR}/build.stderr.log" >&2 || true
        exit 1
    fi
else
    echo
    echo "[1/4] Skipping build (--skip-build)"
fi

if [[ ! -x "${BIN}" ]]; then
    echo "Binary not found or not executable: ${BIN}" >&2
    exit 1
fi

echo
echo "[2/4] perf stat run..."
start_workload_bg perf-stat
perf_stat_pid="${WORKLOAD_PID}"
sleep_warmup_or_fail "${perf_stat_pid}"
run_profiler_nonfatal "perf stat" \
    "${SUDO[@]}" perf stat -d \
    -e task-clock,cycles,instructions,branches,branch-misses,cache-misses,context-switches,cpu-migrations \
    -p "${perf_stat_pid}" -- sleep "${DURATION_SECONDS}" \
    >"${OUT_DIR}/perf-stat.profiler.stdout.log" 2>"${OUT_DIR}/perf-stat.stderr.log"
wait_for_pid_nonfatal "${perf_stat_pid}" "perf-stat workload"

echo
echo "[3/4] perf sched record/timehist/latency run..."
start_workload_bg perf-sched
perf_sched_pid="${WORKLOAD_PID}"
sleep_warmup_or_fail "${perf_sched_pid}"
run_profiler_nonfatal "perf sched record" \
    "${SUDO[@]}" perf sched record -o "${OUT_DIR}/perf-sched.data" -p "${perf_sched_pid}" -- sleep "${DURATION_SECONDS}" \
    >"${OUT_DIR}/perf-sched.profiler.stdout.log" 2>"${OUT_DIR}/perf-sched.record.log"
wait_for_pid_nonfatal "${perf_sched_pid}" "perf-sched workload"
if [[ -f "${OUT_DIR}/perf-sched.data" ]]; then
    run_profiler_nonfatal "perf sched timehist" \
        "${SUDO[@]}" perf sched timehist -i "${OUT_DIR}/perf-sched.data" \
        >"${OUT_DIR}/perf-sched.timehist.txt" 2>"${OUT_DIR}/perf-sched.timehist.err.log"
    run_profiler_nonfatal "perf sched latency" \
        "${SUDO[@]}" perf sched latency -i "${OUT_DIR}/perf-sched.data" \
        >"${OUT_DIR}/perf-sched.latency.txt" 2>"${OUT_DIR}/perf-sched.latency.err.log"
else
    echo "Warning: perf-sched.data not found; skipping timehist/latency decode" >&2
fi

echo
if [[ "${SKIP_OFFCPU}" -eq 1 ]]; then
    echo "[4/4] Skipping off-CPU run (--skip-offcpu)"
elif command -v offcputime-bpfcc >/dev/null 2>&1; then
    echo "[4/4] offcputime-bpfcc run..."
    start_workload_bg offcpu
    workload_pid="${WORKLOAD_PID}"
    sleep_warmup_or_fail "${workload_pid}"

    set +e
    "${SUDO[@]}" offcputime-bpfcc -df -p "${workload_pid}" "${DURATION_SECONDS}" \
        >"${OUT_DIR}/offcpu.folded.txt" 2>"${OUT_DIR}/offcpu.log"
    offcpu_status=$?
    wait_for_pid_nonfatal "${workload_pid}" "offcpu workload"
    workload_status=0
    set -e

    if [[ "${offcpu_status}" -ne 0 ]]; then
        echo "Warning: offcputime-bpfcc failed (exit ${offcpu_status}); see ${OUT_DIR}/offcpu.log" >&2
    fi
    if [[ "${workload_status}" -ne 0 ]]; then
        echo "Warning: off-CPU workload run exited with status ${workload_status}; see ${OUT_DIR}/offcpu.workload.stderr.log" >&2
    fi
else
    echo "[4/4] offcputime-bpfcc not found; skipping off-CPU run"
fi

if [[ "${WITH_THREAD_CPU}" -eq 1 ]]; then
    echo
    echo "[extra] perf stat per-thread CPU/HW counters run..."
    start_workload_bg perf-thread-cpu
    perf_thread_cpu_pid="${WORKLOAD_PID}"
    sleep_warmup_or_fail "${perf_thread_cpu_pid}"
    capture_thread_snapshot "${perf_thread_cpu_pid}" "${OUT_DIR}/perf-thread-cpu.threads.csv"
    run_profiler_nonfatal "perf stat per-thread" \
        "${SUDO[@]}" perf stat --per-thread -d -d -d \
        -e "${THREAD_HW_EVENTS}" \
        -p "${perf_thread_cpu_pid}" -- sleep "${DURATION_SECONDS}" \
        >"${OUT_DIR}/perf-thread-cpu.profiler.stdout.log" 2>"${OUT_DIR}/perf-thread-cpu.stderr.log"
    wait_for_pid_nonfatal "${perf_thread_cpu_pid}" "perf-thread-cpu workload"
fi

if [[ "${WITH_HOTSPOTS}" -eq 1 ]]; then
    echo
    echo "[extra] perf record hotspots run..."
    start_workload_bg perf-hotspots
    perf_hotspots_pid="${WORKLOAD_PID}"
    sleep_warmup_or_fail "${perf_hotspots_pid}"
    capture_thread_snapshot "${perf_hotspots_pid}" "${OUT_DIR}/perf-hotspots.threads.csv"
    run_profiler_nonfatal "perf record hotspots" \
        "${SUDO[@]}" perf record -F "${HOTSPOT_FREQ}" --call-graph fp \
        -o "${OUT_DIR}/perf-hotspots.data" \
        -p "${perf_hotspots_pid}" -- sleep "${DURATION_SECONDS}" \
        >"${OUT_DIR}/perf-hotspots.profiler.stdout.log" 2>"${OUT_DIR}/perf-hotspots.record.log"
    wait_for_pid_nonfatal "${perf_hotspots_pid}" "perf-hotspots workload"

    if [[ -f "${OUT_DIR}/perf-hotspots.data" ]]; then
        run_profiler_nonfatal "perf report hotspots" \
            "${SUDO[@]}" perf report --stdio --percent-limit 0.2 \
            --sort comm,symbol -i "${OUT_DIR}/perf-hotspots.data" \
            >"${OUT_DIR}/perf-hotspots.report.txt" 2>"${OUT_DIR}/perf-hotspots.report.err.log"

        awk '/moor-task-pool-|moor-scheduler|direct-schedule|moor-seq-writer|moor-batch-writ/' \
            "${OUT_DIR}/perf-hotspots.report.txt" \
            >"${OUT_DIR}/perf-hotspots.report.selected-threads.txt" || true

        run_profiler_nonfatal "perf report taskpool self hotspots" \
            "${SUDO[@]}" perf report --stdio --no-children --percent-limit 0.2 \
            --sort symbol --comms moor-task-pool- \
            -i "${OUT_DIR}/perf-hotspots.data" \
            >"${OUT_DIR}/perf-hotspots.report.taskpool-self.txt" \
            2>"${OUT_DIR}/perf-hotspots.report.taskpool-self.err.log"

        run_profiler_nonfatal "perf report atomic callchains" \
            "${SUDO[@]}" perf report --stdio --percent-limit 0.01 \
            --sort parent,symbol --comms moor-task-pool- \
            --symbols __aarch64_ldadd8_relax,__aarch64_ldadd8_rel \
            -i "${OUT_DIR}/perf-hotspots.data" \
            >"${OUT_DIR}/perf-hotspots.report.atomics.callchains.txt" \
            2>"${OUT_DIR}/perf-hotspots.report.atomics.callchains.err.log"

        # Attribute atomic instructions to their immediate caller using perf script stacks.
        # The weights here are event-period weighted, not raw sample counts.
        run_profiler_nonfatal "perf script atomic attribution" \
            "${SUDO[@]}" bash -lc '
                perf script -i "$1" --comms moor-task-pool- 2>"$2" |
                awk '"'"'
function flush_sample(   i,j,a,caller,key){
  if (sample_count<=0 || n==0) return;
  for (i=1;i<=n;i++) {
    if (frames[i] ~ /^__aarch64_ldadd8_rel/) {
      j=i+1;
      while (j<=n && (frames[j]=="[unknown]" || frames[j] ~ /^0x[0-9a-f]+$/)) j++;
      if (j<=n) {
        key=frames[i] " => " frames[j];
        by_caller[key]+=sample_count;
        by_atomic[frames[i]]+=sample_count;
        total_atomic+=sample_count;
      }
    }
  }
}
/^[^[:space:]].*:[[:space:]]*[0-9]+[[:space:]]+armv8_pmuv3_1\/cycles\/P:/ {
  flush_sample();
  n=0;
  if (match($0, /:[[:space:]]*([0-9]+)[[:space:]]+armv8_pmuv3_1\/cycles\/P:/, m)) sample_count=m[1]+0; else sample_count=1;
  next;
}
/^[\t ]+[0-9a-f]+[[:space:]]+/ {
  line=$0;
  sub(/^[\t ]+[0-9a-f]+[[:space:]]+/, "", line);
  split(line, a, /\+/);
  sym=a[1];
  gsub(/[[:space:]]+$/, "", sym);
  if (sym!="") frames[++n]=sym;
  next;
}
END {
  flush_sample();
  printf "total_atomic_weight,%d\n", total_atomic;
  for (k in by_atomic) printf "atomic,%s,%d,%.2f%%\n", k, by_atomic[k], 100*by_atomic[k]/total_atomic;
  for (k in by_caller) printf "caller,%s,%d,%.2f%%\n", k, by_caller[k], 100*by_caller[k]/total_atomic;
}
'"'"' | sort -t, -k3,3nr >"$3"
            ' _ \
            "${OUT_DIR}/perf-hotspots.data" \
            "${OUT_DIR}/perf-hotspots.atomics.perf-script.err.log" \
            "${OUT_DIR}/perf-hotspots.atomics.by-caller.csv"
    else
        echo "Warning: perf-hotspots.data not found; skipping report decode" >&2
    fi
fi

cat >"${OUT_DIR}/run-config.txt" <<EOF
bin=${BIN}
concurrency=${CONCURRENCY}
warmup_seconds=${WARMUP_SECONDS}
duration_seconds=${DURATION_SECONDS}
total_swamp_seconds=${TOTAL_SWAMP_SECONDS}
num_verb_iterations=${NUM_VERB_ITERATIONS}
num_objects=${NUM_OBJECTS}
max_ticks=${MAX_TICKS}
with_thread_cpu=${WITH_THREAD_CPU}
with_hotspots=${WITH_HOTSPOTS}
thread_hw_events=${THREAD_HW_EVENTS}
hotspot_freq=${HOTSPOT_FREQ}
extra_args=${EXTRA_ARGS[*]:-}
EOF

echo
echo "Done. Artifacts:"
for f in \
    "${OUT_DIR}/perf-stat.stderr.log" \
    "${OUT_DIR}/perf-stat.load-results.csv" \
    "${OUT_DIR}/perf-sched.data" \
    "${OUT_DIR}/perf-sched.timehist.txt" \
    "${OUT_DIR}/perf-sched.latency.txt" \
    "${OUT_DIR}/perf-sched.load-results.csv"
do
    if [[ -f "${f}" ]]; then
        echo "  ${f}"
    fi
done
if [[ -f "${OUT_DIR}/offcpu.folded.txt" ]]; then
    echo "  ${OUT_DIR}/offcpu.folded.txt"
fi
if [[ -f "${OUT_DIR}/offcpu.load-results.csv" ]]; then
    echo "  ${OUT_DIR}/offcpu.load-results.csv"
fi
if [[ -f "${OUT_DIR}/perf-thread-cpu.stderr.log" ]]; then
    echo "  ${OUT_DIR}/perf-thread-cpu.stderr.log"
fi
if [[ -f "${OUT_DIR}/perf-thread-cpu.threads.csv" ]]; then
    echo "  ${OUT_DIR}/perf-thread-cpu.threads.csv"
fi
if [[ -f "${OUT_DIR}/perf-hotspots.data" ]]; then
    echo "  ${OUT_DIR}/perf-hotspots.data"
fi
if [[ -f "${OUT_DIR}/perf-hotspots.report.txt" ]]; then
    echo "  ${OUT_DIR}/perf-hotspots.report.txt"
fi
if [[ -f "${OUT_DIR}/perf-hotspots.report.selected-threads.txt" ]]; then
    echo "  ${OUT_DIR}/perf-hotspots.report.selected-threads.txt"
fi
if [[ -f "${OUT_DIR}/perf-hotspots.report.taskpool-self.txt" ]]; then
    echo "  ${OUT_DIR}/perf-hotspots.report.taskpool-self.txt"
fi
if [[ -f "${OUT_DIR}/perf-hotspots.report.atomics.callchains.txt" ]]; then
    echo "  ${OUT_DIR}/perf-hotspots.report.atomics.callchains.txt"
fi
if [[ -f "${OUT_DIR}/perf-hotspots.atomics.by-caller.csv" ]]; then
    echo "  ${OUT_DIR}/perf-hotspots.atomics.by-caller.csv"
fi
