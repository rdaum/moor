# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# Affero General Public License as published by the Free Software Foundation,
# version 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
# details.
#
# You should have received a copy of the GNU Affero General Public License along
# with this program. If not, see <https://www.gnu.org/licenses/>.

"""Compare isolated Fjall builds using identical core and foreground workloads."""

import argparse
import hashlib
import json
import platform
import re
import subprocess
import tempfile
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--variant", nargs=3, action="append", metavar=("NAME", "MOORC", "MICRO"), required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--commits", type=int, default=250000)
    parser.add_argument("--stress-seconds", type=int, default=10)
    parser.add_argument("--only-micro", action="store_true", help="Run foreground workloads only")
    parser.add_argument("--skip-write-stress", action="store_true", help="Omit the workloads that require the game_update core object")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    args.output.mkdir(parents=True, exist_ok=True)
    variants = [(name, str(Path(moorc).resolve()), str(Path(micro).resolve())) for name, moorc, micro in args.variant]
    report = {
        "platform": platform.platform(), "variants": {}, "runs": [],
        "fixture_sha256": {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
                           for path in sorted((root / "cores/benches/src").glob("*.moo"))},
        "cpu_topology": subprocess.check_output(["lscpu"], text=True),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "cpu": next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")), "unknown"),
    }
    for name, moorc, micro in variants:
        report["variants"][name] = {path: hashlib.sha256(Path(path).read_bytes()).hexdigest() for path in (moorc, micro)}
    flags = ["--use-boolean-returns", "true", "--use-symbols-in-builtins", "true", "--custom-errors", "true", "--use-uuobjids", "true", "--anonymous-objects", "false"]
    workloads = [
        ("history-rollup", "test_string_history_append", "{1,1024,9472,70,0,2,1,0}", 3),
        ("history-pressure", "test_string_history_append", "{8,1024,2048,150,0,0,1,0}", 3),
        ("history-replace", "test_string_history_append", "{1,1024,2048,150,0,0,1,1}", 3),
        ("history-rebuild", "test_string_history_append", "{1,1024,2048,150,0,0,1,2}", 3),
        ("write-overwrite", "test_write_stress", f"{{{args.stress_seconds},256,20,0}}", 1),
        ("write-append", "test_write_stress", f"{{{args.stress_seconds},256,20,1}}", 1),
    ]
    if args.only_micro:
        workloads = []
    if args.skip_write_stress:
        workloads = [item for item in workloads if not item[0].startswith("write-")]
    ansi = re.compile(r"\x1b\[[0-9;]*m")
    for repeat in range(args.repeats):
        # Alternate build order to reduce systematic thermal/order bias.
        order = variants if repeat % 2 == 0 else list(reversed(variants))
        for name, moorc, micro in order:
            jobs = [(f"micro-{threads}", [micro, str(threads), str(args.commits)]) for threads in (1, 4)]
            with tempfile.TemporaryDirectory(prefix="moor-persistence-qualification-") as temporary:
                for label, verb, values, phases in workloads:
                    jobs.append((label, [moorc, *flags, "--src-objdef-dir", str(root / "cores/benches/src"), "--out-objdef-dir", str(Path(temporary) / label), "--test-wizard", "2", "--run-tests", "true", "--test-filter", f"#668:{verb}", "--test-timeout", "360", "--test-phases", str(phases), "--test-args", values]))
                for label, command in jobs:
                    print(f"{name} {label} repeat={repeat + 1}", flush=True)
                    log = args.output / f"{name}-{label}-{repeat + 1}.log"
                    started = time.monotonic()
                    with log.open("w") as stream:
                        result = subprocess.run(command, cwd=root, stdout=stream, stderr=subprocess.STDOUT, timeout=600)
                    text = ansi.sub("", log.read_text())
                    selected = [line for line in text.splitlines() if any(marker in line for marker in ("HISTORY_APPEND_RESULT", "PERSISTENCE_BOUNDARY", "PERSISTENCE_OCCUPANCY", "TICK_STATS", "WRITE_STRESS_RESULT", "CACHE_SUM", "PERF_KEY", "PERF_SUM", "commits_per_second=", "worker_performance_cores="))]
                    report["runs"].append({"variant": name, "workload": label, "repeat": repeat + 1, "command": command, "seconds": time.monotonic() - started, "exit_code": result.returncode, "measurements": selected})
                    (args.output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
                    if result.returncode:
                        raise SystemExit(f"failed: {log}")


if __name__ == "__main__":
    main()
