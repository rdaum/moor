# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com>
# This program is free software under the GNU General Public License, version 3.
# It is distributed without any warranty. See <https://www.gnu.org/licenses/>.

"""Exercise shared make rules in disposable cores without rewriting real sources."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

CORES = Path(__file__).resolve().parents[2]
NAMES = ("cowbell", "snore", "lambda-moor", "minimal-core", "benches")
FAKE_COMPILER = '''import json, os, pathlib, shutil, sys
args = sys.argv[1:]
with open(os.environ["COMPILER_LOG"], "a") as log:
    log.write(json.dumps(args) + "\\n")
source = pathlib.Path(args[args.index("--src-objdef-dir") + 1])
if "--out-objdef-dir" in args:
    output = pathlib.Path(args[args.index("--out-objdef-dir") + 1])
    shutil.copytree(source, output)
    if os.environ.get("BREAK_ROUNDTRIP") and output.name.endswith(".rt"):
        (output / "extra.moo").write_text("changed")
elif "--out-textdump" in args:
    pathlib.Path(args[args.index("--out-textdump") + 1]).write_text("textdump")
'''


class CoreMakefiles(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        shutil.copytree(CORES / "make", self.root / "cores/make", ignore=shutil.ignore_patterns("__pycache__"))
        for name in NAMES:
            directory = self.root / "cores" / name
            (directory / "src/nested/deep").mkdir(parents=True)
            shutil.copy(CORES / name / "Makefile", directory)
            (directory / "src/constants.moo").write_text("define ROOT = #1;\n")
            (directory / "src/nested/deep/item.moo").write_text("object #1\nendobject\n")
        self.compiler = self.root / "compiler.py"
        self.compiler.write_text(FAKE_COMPILER)
        self.log = self.root / "calls.jsonl"
        # These tests launch independent make processes, not recursive builds.
        # Do not inherit the invoking core's overrides or jobserver descriptors.
        overrides = {"MAKEFLAGS", "MFLAGS", "MAKEOVERRIDES", "MAKELEVEL", "CARGO", "DEBUG",
                     "MOORC", "MOORC_TYPE", "MOORC_PROFILE", "MOORC_IMAGE", "OPTIONS", "FEATURES",
                     "SRC_DIRECTORY", "OUTPUT_DIRECTORY", "CHECK_TARGETS", "TEST_MODE",
                     "CORE_OUTPUT_TARGET"}
        self.env = {key: value for key, value in os.environ.items() if key not in overrides}
        self.env["COMPILER_LOG"] = str(self.log)

    def make(self, core, *args, fake=True, success=True, env=None):
        command = ["make", "--no-print-directory", "-C", str(self.root / "cores" / core)]
        if fake:
            command.append(f"MOORC={sys.executable} {self.compiler}")
        result = subprocess.run(command + list(args), env=env or self.env, text=True, capture_output=True)
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result.stdout + result.stderr

    def test_default_build_and_recursive_source_changes(self):
        import json
        for core in NAMES:
            with self.subTest(core=core):
                self.make(core)
                directory = self.root / "cores" / core
                self.assertTrue((directory / "gen.objdir/nested/deep/item.moo").is_file())
                (directory / "src/nested/deep/item.moo").unlink()
                self.make(core)
                self.assertFalse((directory / "gen.objdir/nested/deep/item.moo").exists())
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertTrue(all("--include-baselines=false" in args for args in calls))
        self.assertEqual(len(calls), 2 * len(NAMES))

    def test_rebuild_requires_stable_roundtrip_and_clean_keeps_sources_and_state(self):
        for core in NAMES:
            with self.subTest(core=core):
                directory = self.root / "cores" / core
                original = (directory / "src/constants.moo").read_bytes()
                self.make(core, "rebuild", success=False, env={**self.env, "BREAK_ROUNDTRIP": "1"})
                self.assertEqual((directory / "src/constants.moo").read_bytes(), original)
                self.assertFalse((directory / "src/extra.moo").exists())
                self.make(core, "rebuild")
                (directory / "gen.run").mkdir()
                (directory / "gen.run/world.db").write_text("keep")
                self.make(core, "clean")
                self.assertFalse((directory / "gen.objdir").exists())
                self.assertFalse((directory / "gen.objdir.rt").exists())
                self.assertEqual((directory / "src/constants.moo").read_bytes(), original)
                self.assertEqual((directory / "gen.run/world.db").read_text(), "keep")

    def test_compiler_modes_and_core_options(self):
        for core in NAMES:
            with self.subTest(core=core):
                profile = "release" if core == "benches" else "dev"
                cargo = self.make(core, "-n", "build", "MOORC_TYPE=cargo", fake=False)
                self.assertIn(f"cargo run --profile {profile} -p moorc --", cargo)
                direct = self.make(core, "-n", "build", "MOORC_TYPE=direct", "CARGO_TARGET_DIR=/tmp/core-target", fake=False)
                self.assertIn(f"/tmp/core-target/{'release' if core == 'benches' else 'debug'}/moorc", direct)
                docker = self.make(core, "-n", "test", "MOORC_TYPE=docker", fake=False)
                self.assertIn('-w /work moor-moor-daemon /moor/moorc', docker)
                self.assertIn("--use-boolean-returns true", docker)
                debug = self.make(core, "-n", "build", "MOORC_TYPE=direct", "DEBUG=1", fake=False)
                self.assertIn('gdb --batch', debug)
                self.make(core, "-n", "build", "MOORC_TYPE=unknown", fake=False, success=False)
        self.assertIn('--flyweight-type false', self.make('snore', '-n', 'build'))
        self.assertIn('--anonymous-objects true', self.make('cowbell', '-n', 'build'))
        self.assertIn('--anonymous-objects false', self.make('benches', '-n', 'build'))

    def test_cowbell_rejects_fixture_ids_in_shipped_export(self):
        (self.root / "cores/cowbell/src/constants.moo").write_text("define FIXTURE = #90100;\n")
        self.assertIn("test fixtures entered shipped export", self.make("cowbell", success=False))

    def test_cowbell_check_builds_once_and_runs_binaries_directly(self):
        for target in ("check", "check-full"):
            output = self.make("cowbell", "-n", target, fake=False)
            cargo = [line for line in output.splitlines() if line.startswith("cargo ")]
            self.assertEqual(len(cargo), 1, output)
            self.assertTrue(cargo[0].startswith("cargo build --locked "), cargo)
            self.assertNotIn("cargo run", output)
            self.assertEqual("/debug/core-session-runner --cowbell" in output, target == "check-full")

    def test_textdump_and_legacy_aliases(self):
        self.make("lambda-moor", "output")
        self.assertTrue((self.root / "cores/lambda-moor/gen.moo-textdump").is_file())
        for core in ("cowbell", "snore"):
            for new, old in (("test-headless", "runtime-headless"), ("test-session", "runtime-session")):
                # These aliases must select the same commands, including test discovery guards.
                self.assertEqual(self.make(core, "-n", new), self.make(core, "-n", old))

    def test_check_stops_at_failure_even_with_parallel_make(self):
        rules = self.root / "check-order.mk"
        log = self.root / "check-order.log"
        rules.write_text(f"check-make:\n\t@echo shared > {log}\n"
                         f"first:\n\t@echo first >> {log}\n"
                         f"second:\n\t@echo second >> {log}\n\t@false\n"
                         f"third:\n\t@echo third >> {log}\n")
        self.make("minimal-core", "-f", "Makefile", "-f", str(rules), "-j8", "check",
                  "CHECK_TARGETS=first second third", success=False)
        self.assertEqual(log.read_text().splitlines(), ["shared", "first", "second"])

    def test_method_runner_rejects_empty_filters_and_preserves_logs(self):
        runner = CORES / "make/checked_methods.py"
        for output, code, expected in [("Found 3 tests", 0, 0), ("Found 0 tests", 0, 1),
                                       ("Found 3 tests\nFiltered tests from 3 to 0", 0, 1),
                                       ("Found 3 tests", 4, 4)]:
            result = subprocess.run([sys.executable, str(runner), "--log", str(self.log),
                                     sys.executable, "-c", f"print({output!r}); raise SystemExit({code})"],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
            self.assertEqual(self.log.read_text(), output + "\n")

    def test_inventory_fixture_includes_shared_rules(self):
        import json
        scripts = self.root / "cores/cowbell/tests/baseline"
        scripts.mkdir(parents=True)
        shutil.copy(CORES / "cowbell/tests/baseline/check_inventory.py", scripts)
        output = self.make("cowbell", "test-build-inventory", "CARGO=true")
        self.assertIn("PASS: nested additions, edits, and deletions", output)
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertEqual(len(calls), 4)
        self.assertTrue(all(args.count("--use-boolean-returns") == 1 for args in calls))

    def test_expected_failure_requires_both_error_and_nonzero_exit(self):
        runner = CORES / "make/expect_failure.py"
        for output, status, expected in [
            ("expected diagnostic", 1, 0),
            ("expected diagnostic", 0, 1),
            ("unrelated error", 1, 1),
        ]:
            with self.subTest(output=output, status=status):
                result = subprocess.run(
                    [sys.executable, str(runner), "--name", "negative-fixture",
                     "--match", "expected diagnostic", "--", sys.executable, "-c",
                     f"print({output!r}); raise SystemExit({status})"],
                    capture_output=True, text=True,
                )
                self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
                if expected == 0:
                    self.assertEqual(result.stdout, "PASS negative-fixture (expected failure)\n")
                    self.assertEqual(result.stderr, "")
                else:
                    self.assertIn(output, result.stderr)


if __name__ == "__main__":
    unittest.main()
