#!/usr/bin/env python3
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

"""Check each binary's resolved graph without workspace feature unification."""
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def graph(package, enabled):
    args = ["cargo", "tree", "--locked", "-p", package, "--edges", "normal,build", "--prefix", "none", "--format", "{p}"]
    if enabled:
        args += ["--features", "postgres"]
    output = subprocess.check_output(args, cwd=ROOT, text=True)
    return {line.split()[0] for line in output.splitlines() if line.strip()}


for package in ["moor-db", "moor-daemon", "moor-server", "moorc", "moor-emh"]:
    for enabled in [False, True]:
        packages = graph(package, enabled)
        assert ("pq-sys" in packages) == enabled, (package, enabled, "libpq feature leakage or missing forwarding")
        if package in ["moor-db", "moor-daemon"]:
            assert not any(p == "tokio" or p.startswith("tokio-") for p in packages), (package, enabled, "Tokio path")
        if package == "moor-db":
            assert ("moor-compiler" in packages) == enabled, (enabled, "compiler feature leakage")
        print(f"{package}: postgres={enabled}: dependency graph passed")
