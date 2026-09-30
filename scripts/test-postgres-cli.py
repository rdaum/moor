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

"""Check storage CLI routing and setup without creating local world or ancillary files."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile
import uuid

ROOT = Path(__file__).resolve().parent.parent
BINARIES = ["moor-daemon", "moor", "moorc", "moor-emh"]


def run(binary, args, env, success):
    result = subprocess.run([str(ROOT / "target/debug" / binary), *args], env=env,
                            text=True, capture_output=True, timeout=45)
    assert (result.returncode == 0) == success, (binary, result.returncode, result.stdout, result.stderr)
    return result.stdout + result.stderr


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--disabled", action="store_true")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="moor-storage-cli-") as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        env["XDG_CONFIG_HOME"] = str(root / "config")
        env["XDG_DATA_HOME"] = str(root / "xdg-data")
        for binary in BINARIES:
            positional = [] if binary == "moorc" else [str(root / "data")]
            selected = [*positional, "--storage-backend", "postgres"]
            if args.disabled:
                output = run(binary, selected, env, False)
                assert "PostgreSQL support is disabled" in output, output
                assert list(root.iterdir()) == [], list(root.iterdir())
                print(f"{binary}: disabled feature rejected before filesystem changes")
                continue
            postgres = ["--pg-service", "moor_adapter", "--pg-hostaddr", "127.0.0.1",
                        "--pg-schema", "cli_" + uuid.uuid4().hex]
            for extra in [["--db", "world.db"], ["--pg-query-timeout-seconds", "0"]]:
                run(binary, [*selected, *postgres, *extra, "--init-storage"], env, False)
                assert list(root.iterdir()) == [], list(root.iterdir())
            output = run(binary, [*selected, *postgres, "--init-storage"], env, True)
            assert "Initialized PostgreSQL database" in output, output
            assert list(root.iterdir()) == [], list(root.iterdir())
            output = run(binary, [*selected, *postgres, "--init-storage"], env, False)
            assert "42P06" in output, output
            assert list(root.iterdir()) == [], list(root.iterdir())
            print(f"{binary}: explicit setup and path rejection passed")

        if args.disabled:
            return
        for binary in ["moor-daemon", "moor"]:
            schema = "yaml_" + uuid.uuid4().hex
            config = root / "storage.yaml"
            config.write_text(f"storage:\n  backend: postgres\n  postgres:\n    service: moor_adapter\n    schema: {schema}\n    hostaddr: 127.0.0.1\n", encoding="utf8")
            invocation = [str(root / "data"), "--config-file", str(config), "--init-storage"]
            run(binary, [*invocation, "--pg-schema", schema + "_override"], env, True)
            run(binary, invocation, env, True)
            config.write_text(config.read_text(encoding="utf8") + "database: {}\n", encoding="utf8")
            output = run(binary, invocation, env, False)
            assert "Fjall database table settings" in output, output
            assert list(root.iterdir()) == [config], list(root.iterdir())
            config.unlink()
            print(f"{binary}: YAML selection and explicit CLI overrides passed")


if __name__ == "__main__":
    main()
