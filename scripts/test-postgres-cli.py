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

"""Check storage CLI routing, setup isolation, and a cross-backend objdef round trip."""
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


def snapshot_roundtrip(root, env):
    features = ["--use-boolean-returns=true", "--use-symbols-in-builtins=true",
                "--custom-errors=true", "--use-uuobjids=true", "--anonymous-objects=false"]
    first, postgres_dump, final = (root / name for name in ["first", "postgres", "final"])
    pg = ["--storage-backend=postgres", "--pg-service=moor_adapter", "--pg-hostaddr=127.0.0.1",
          "--pg-schema=roundtrip_" + uuid.uuid4().hex, "--pg-max-exports=1"]
    run("moorc", [*features, "--db", str(root / "fjall-first"),
                  "--src-objdef-dir", str(ROOT / "cores/benches/src"),
                  "--out-objdef-dir", str(first)], env, True)
    run("moorc", [*pg, "--init-storage"], env, True)
    run("moorc", [*features, *pg, "--src-objdef-dir", str(first),
                  "--out-objdef-dir", str(postgres_dump)], env, True)
    restored = ["--db", str(root / "fjall-restored")]
    run("moorc", [*features, *restored, "--src-objdef-dir", str(postgres_dump),
                  "--out-objdef-dir", str(final)], env, True)

    def contents(path):
        return {file.relative_to(path): file.read_bytes() for file in path.rglob("*") if file.is_file()}

    original = contents(first)
    assert original and contents(postgres_dump) == original
    assert contents(final) == original
    empty = root / "empty"
    empty.mkdir()
    for storage in [pg, restored]:
        output = run("moorc", [*features, *storage, "--src-objdef-dir", str(empty),
                              "--run-tests=true", "--test-wizard=2", "--test-phases=3",
                              "--test-filter=#668:test_string_history_append", "--test-timeout=30",
                              "--test-args={2, 16, 128, 5, 0, 1, 1, 0}"], env, True)
        assert "Test #668:test_string_history_append passed" in output, output
    print(f"moorc: Fjall/objdef/PostgreSQL/objdef/Fjall round trip preserved {len(original)} files; both functional probes passed")


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
            for extra in [["--db", "world.db"], ["--pg-query-timeout-seconds", "0"],
                          ["--pg-max-exports", "0"], ["--pg-max-exports", "65"]]:
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

        snapshot_roundtrip(root, env)


if __name__ == "__main__":
    main()
