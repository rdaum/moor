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

"""Check launcher arguments against Compose without building or starting containers."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class CowbellLauncherTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "checkout with spaces"
        self.root.mkdir()
        source = Path(__file__).resolve().parents[2]
        (self.root / "scripts").mkdir()
        shutil.copy(source / "scripts/start-moor-cowbell.sh", self.root / "scripts")
        shutil.copy(source / "docker-compose.yml", self.root)
        (self.root / "cores/cowbell/src").mkdir(parents=True)
        (self.root / "node_modules").mkdir()
        self.baseline = self.root / "release source"
        self.baseline.mkdir()
        self.tools = self.root / "tools"
        self.tools.mkdir()
        self.docker = shutil.which("docker")
        if not self.docker:
            self.skipTest("Docker Compose is required (no daemon needed)")
        fake_docker = self.tools / "docker"
        fake_docker.write_text('''#!/usr/bin/env python3
import json, os, subprocess, sys
if sys.argv[1] == "info":
    print("[]")
else:
    assert sys.argv[1:4] == ["compose", "up", "--build"], sys.argv
    result = subprocess.run([os.environ["TEST_DOCKER"], "compose", "config", "--format", "json"], check=True, capture_output=True, text=True)
    print("CONFIG=" + json.dumps({"args": sys.argv[1:], "config": json.loads(result.stdout)}))
''')
        fake_docker.chmod(0o755)
        self.environment = dict(os.environ)
        for name in ["MOOR_GIT_UPSTREAM", "MOOR_BASELINE_OBJDEF_DIR", "RUN_DIR", "MOOR_CONFIG_FILE", "COMPOSE_FILE"]:
            self.environment.pop(name, None)
        self.environment.update(PATH=f"{self.tools}:{os.environ['PATH']}", TEST_DOCKER=self.docker)

    def launch(self, *args):
        return subprocess.run(
            ["bash", str(self.root / "scripts/start-moor-cowbell.sh"), *args],
            cwd=self.root.parent, env=self.environment, capture_output=True, text=True,
        )

    def configuration(self, *args):
        result = self.launch(*args)
        self.assertEqual(result.returncode, 0, result.stderr)
        line = next(line for line in result.stdout.splitlines() if line.startswith("CONFIG="))
        return json.loads(line.removeprefix("CONFIG="))

    def test_ordinary_import_has_no_implicit_baseline(self):
        result = self.configuration("--debug", "--detach")
        service = result["config"]["services"]["moor"]
        self.assertIn("--enable-git-worker", service["command"])
        self.assertIn("--enable-curl-worker", service["command"])
        self.assertIsNone(service["environment"].get("MOOR_GIT_UPSTREAM"))
        self.assertIsNone(service["environment"].get("MOOR_BASELINE_OBJDEF_DIR"))
        self.assertEqual(service["build"]["args"]["BUILD_PROFILE"], "debug")
        self.assertEqual(result["args"][-1], "--detach")

    def test_explicit_baseline_and_source_paths_preserve_spaces(self):
        result = self.configuration("--core", str(self.baseline), "--baseline-objdef-dir", str(self.baseline))
        service = result["config"]["services"]["moor"]
        self.assertIn("--import=/db/release source", service["command"])
        self.assertEqual(service["environment"]["MOOR_BASELINE_OBJDEF_DIR"], "/db/release source")

    def test_git_reference_is_not_shell_input(self):
        reference = "origin/main;$(touch injected)"
        result = self.configuration("--git-upstream", reference)
        self.assertEqual(result["config"]["services"]["moor"]["environment"]["MOOR_GIT_UPSTREAM"], reference.replace("$", "$$"))
        self.assertFalse((self.root / "injected").exists())

    def test_rejects_ambiguous_or_missing_inputs_before_startup(self):
        for arguments in [
            ["--git-upstream"],
            ["--baseline-objdef-dir"],
            ["--core"],
            ["--git-upstream", "origin/main", "--baseline-objdef-dir", str(self.baseline)],
            ["--core", str(self.root.parent)],
        ]:
            with self.subTest(arguments=arguments):
                result = self.launch(*arguments)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("CONFIG=", result.stdout)


if __name__ == "__main__":
    unittest.main()
