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

"""Exercise development launcher arguments without building or starting services."""

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import unittest


class DevLauncherTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / "checkout with spaces"
        (self.root / "scripts").mkdir(parents=True)
        source = Path(__file__).resolve().parents[2]
        shutil.copy(source / "scripts/dev.sh", self.root / "scripts")
        (self.root / "cores/cowbell/src").mkdir(parents=True)
        (self.root / "cores/cowbell/src/llm").mkdir()
        (self.root / "cores/cowbell/src/llm/llm_client.moo").touch()
        self.data = self.root / "moor-data"
        self.data.mkdir()
        (self.data / "old-database").touch()
        tools = self.root / "fake-tools"
        tools.mkdir()
        cargo = tools / "cargo"
        cargo.write_text('''#!/usr/bin/env python3
import json, os, sys
if sys.argv[1] == "metadata":
    print(json.dumps({"target_directory": os.path.join(os.getcwd(), "target")}))
else:
    assert sys.argv[1] == "build", sys.argv
''')
        cargo.chmod(0o755)
        npm = tools / "npm"
        npm.write_text("#!/usr/bin/env bash\nexit 0\n")
        npm.chmod(0o755)
        concurrent = self.root / "node_modules/.bin/concurrently"
        concurrent.parent.mkdir(parents=True)
        concurrent.write_text('''#!/usr/bin/env node
const fs = require("node:fs");
const {spawnSync} = require("node:child_process");
const commands = process.argv.slice(2).filter(argument => argument.startsWith("exec "));
const configDir = fs.readdirSync(process.env.TMPDIR).find(name => name.startsWith("moor-dev-llm."));
if (configDir) {
    // Run only the daemon command, as concurrently would, against the fake daemon.
    const path = process.env.TMPDIR + "/" + configDir + "/config.json";
    fs.writeFileSync("startup-mode.json", JSON.stringify(fs.statSync(path).mode & 0o777));
    const daemon = commands.find(command => command.includes("moor-daemon"));
    const result = spawnSync("bash", ["-c", daemon], {stdio: "inherit"});
    if (result.status !== 0) process.exit(1);
}
console.log("COMMANDS=" + JSON.stringify(process.argv.slice(2)));
''')
        concurrent.chmod(0o755)
        daemon = self.root / "target/debug/moor-daemon"
        daemon.parent.mkdir(parents=True)
        daemon.write_text('''#!/usr/bin/env python3
import json, os, sys
path = sys.argv[sys.argv.index("--startup-config") + 1]
config = json.loads(open(path).read())
with open("startup-capture.json", "w") as capture:
    json.dump({"config": config, "pipe": not os.path.isfile(path),
               "keyInherited": bool(os.environ.get("OPENROUTER_KEY") or os.environ.get("OPENROUTER_API_KEY")),
               "configDirs": [name for name in os.listdir(os.environ["TMPDIR"]) if name.startswith("moor-dev-llm.")]},
              capture)
''')
        daemon.chmod(0o755)
        self.environment = dict(os.environ)
        for name in ("MOOR_CORE", "MOOR_DATA_DIR", "MOOR_DB", "MOOR_EXPORT", "OPENROUTER_KEY", "OPENROUTER_API_KEY"):
            self.environment.pop(name, None)
        self.environment["PATH"] = f"{tools}:{os.environ['PATH']}"
        self.environment["TMPDIR"] = str(self.root.parent)

    def launch(self, *arguments):
        return subprocess.run(
            ["bash", str(self.root / "scripts/dev.sh"), *arguments],
            cwd=self.root.parent, env=self.environment, capture_output=True, text=True,
        )

    def commands(self, *arguments):
        result = self.launch(*arguments)
        self.assertEqual(result.returncode, 0, result.stderr)
        line = next(line for line in result.stdout.splitlines() if line.startswith("COMMANDS="))
        return [shlex.split(command)[1:] for command in json.loads(line.removeprefix("COMMANDS="))
                if command.startswith("exec ")]

    def daemon(self, commands):
        return next(command for command in commands if Path(command[0]).name == "moor-daemon")

    def test_clean_start_prepares_git_baselines_with_both_workers(self):
        commands = self.commands("--curl-worker", "--git-worker", "--debug", "--clean")
        daemon = self.daemon(commands)
        self.assertEqual(daemon[daemon.index("--git-upstream") + 1], "origin/main")
        self.assertNotIn("--baseline-objdef-dir", daemon)
        self.assertEqual(daemon[daemon.index("--import") + 1], "cores/cowbell/src")
        self.assertTrue(any(Path(command[0]).name == "moor-curl-worker" for command in commands))
        self.assertTrue(any(Path(command[0]).name == "moor-git-worker" for command in commands))
        self.assertFalse((self.data / "old-database").exists())

    def test_baseline_default_does_not_require_git_worker_or_clean(self):
        commands = self.commands()
        daemon = self.daemon(commands)
        self.assertEqual(daemon[daemon.index("--git-upstream") + 1], "origin/main")
        self.assertFalse(any(Path(command[0]).name == "moor-git-worker" for command in commands))
        self.assertTrue((self.data / "old-database").exists())
        self.assertNotIn("--startup-config", daemon)

    def test_openrouter_configures_world_without_exposing_key_to_services(self):
        key = 'fixture-"key\\with$syntax'
        self.environment["OPENROUTER_KEY"] = key
        commands = self.commands()
        self.assertIn("--startup-config", self.daemon(commands))
        captured = json.loads((self.root / "startup-capture.json").read_text())
        self.assertEqual(captured["config"], {"llm": {
            "endpoint": "https://openrouter.ai/api/v1/chat/completions",
            "model": "deepseek/deepseek-v4.1-flash", "key": key,
        }})
        self.assertEqual(json.loads((self.root / "startup-mode.json").read_text()), 0o600)
        self.assertTrue(captured["pipe"])
        self.assertFalse(captured["keyInherited"])
        # The daemon has read the whole pipe, so the key file is already gone.
        self.assertEqual(captured["configDirs"], [])
        self.assertNotIn(key, str(commands))
        self.assertTrue(any(Path(command[0]).name == "moor-curl-worker" for command in commands))
        self.assertTrue((self.data / "old-database").exists())

    def test_openrouter_api_key_alias_and_precedence(self):
        self.environment["OPENROUTER_API_KEY"] = "fixture-api-key"
        for preferred in (None, "", "fixture-preferred-key"):
            with self.subTest(preferred=preferred):
                if preferred is not None:
                    self.environment["OPENROUTER_KEY"] = preferred
                commands = self.commands()
                self.assertIn("--startup-config", self.daemon(commands))
                captured = json.loads((self.root / "startup-capture.json").read_text())
                self.assertEqual(captured["config"]["llm"]["key"], preferred or "fixture-api-key")
                self.assertFalse(captured["keyInherited"])

    def test_explicit_upstream_replaces_default(self):
        daemon = self.daemon(self.commands("--git-upstream", "origin/stable"))
        self.assertEqual(daemon.count("--git-upstream"), 1)
        self.assertEqual(daemon[daemon.index("--git-upstream") + 1], "origin/stable")

    def test_baseline_directory_replaces_git_default(self):
        baseline = str(self.root / "baseline source")
        daemon = self.daemon(self.commands("--baseline-objdef-dir", baseline))
        self.assertNotIn("--git-upstream", daemon)
        self.assertEqual(daemon[daemon.index("--baseline-objdef-dir") + 1], baseline)

    def test_explicit_conflicting_sources_fail_before_clean(self):
        result = self.launch("--clean", "--git-upstream", "origin/stable", "--baseline-objdef-dir", "base")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Choose either", result.stderr)
        self.assertNotIn("COMMANDS=", result.stdout)
        self.assertTrue((self.data / "old-database").exists())


if __name__ == "__main__":
    unittest.main()
