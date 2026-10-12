# Copyright (C) 2026 The mooR Authors
# This program is free software: you can redistribute it and/or modify it under
# the terms of the GNU General Public License as published by the Free Software
# Foundation, version 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.

"""Verify Cowbell telnet connections and native schedule persistence across server restart."""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import tempfile
import time


# Reuse Snore's socket transport; only synthetic test markers need exact line matching.
_client_spec = importlib.util.spec_from_file_location(
    "snore_wire", Path(__file__).resolve().parents[2] / "snore/tests/wire.py")
_snore_wire = importlib.util.module_from_spec(_client_spec)
_client_spec.loader.exec_module(_snore_wire)

# Test-only Argon2id fixture: password cowbell-wire-test, salt isolated-test-salt.
PASSWORD_HASH = (
    "$argon2id$v=19$m=4096,t=3,p=1$aXNvbGF0ZWQtdGVzdC1zYWx0$"
    "k+rBchONAiUKtn5ec4FRSnEd9vMGXWRo0AsP85vakQk"
)


class Client(_snore_wire.Client):
    def expect_line(self, texts, timeout=15):
        """Consume complete lines until an exact marker; echoed code cannot satisfy it."""
        if isinstance(texts, str):
            texts = (texts,)
        deadline = time.monotonic() + timeout
        received = []
        while True:
            while b"\n" in self.buffer:
                line, _, self.buffer = self.buffer.partition(b"\n")
                text = line.removesuffix(b"\r").decode(errors="replace")
                received.append(text)
                assert "Traceback" not in text, f"Unexpected task exception: {text}"
                assert "Confunc failed:" not in text, f"Connection hook failed: {text}"
                if text in texts:
                    return text
            assert time.monotonic() < deadline, (
                f"Timed out waiting for exact line {texts!r}; received {received!r}; "
                f"partial {self.buffer!r}"
            )
            try:
                data = self.socket.recv(65536)
            except socket.timeout:
                continue
            assert data, f"Connection closed waiting for exact line {texts!r}: {received!r}"
            self.buffer += data

    def eval_marker(self, code, markers, timeout=15):
        self.send("eval eval(" + json.dumps(code) + ")")
        return self.expect_line(markers, timeout)


def run(server, core_dir):
    with tempfile.TemporaryDirectory(prefix="cowbell-wire-") as directory:
        root = Path(directory)
        core = root / "src"
        shutil.copytree(core_dir, core)
        arch = core / "arch_wizard.moo"
        source, replacements = re.subn(
            r'(override password(?:\s*\([^;\n]*\))?\s*=\s*).*?;',
            lambda match: match[1] + "<PASSWORD, {" + json.dumps(PASSWORD_HASH) + "}>;",
            arch.read_text(),
        )
        assert replacements == 1, "Expected exactly one ArchWizard password fixture"
        arch.write_text(source)
        # Request checkpoints explicitly so startup snapshots cannot satisfy the assertion.
        sysobj = core / "sysobj.moo"
        source, replacements = re.subn(
            r"(?m)^  property dump_interval\b[^\n]*;\n", "", sysobj.read_text())
        assert replacements == 1, "Expected exactly one periodic checkpoint setting"
        sysobj.write_text(source)
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        config = root / "config.yaml"
        config.write_text(f"""features:
  persistent_tasks: true
  rich_notify: true
  type_dispatch: true
  flyweight_type: true
  bool_type: true
  use_boolean_returns: true
  custom_errors: true
  use_uuobjids: true
  anonymous_objects: true
  enable_eventlog: true
  symbol_type: true
  use_symbols_in_builtins: true
services:
  telnet:
    enabled: true
    address: 127.0.0.1
    port: {port}
    health_check_port: 0
  web:
    enabled: false
  curl_worker:
    enabled: false
""")
        process = None
        clients = []
        logs = []

        def stop():
            nonlocal process
            for client in clients:
                client.close()
            clients.clear()
            if process is None:
                return
            if process.poll() is None:
                process.terminate()
            try:
                code = process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise AssertionError("Owned test server did not stop cleanly")
            process = None
            assert code == 0, f"Test server stopped with status {code}"

        def start(first):
            nonlocal process
            path = root / ("first.log" if first else "resumed.log")
            logs.append(path)
            argv = [str(server), str(root / "data"), "--config-file", str(config),
                    "--private-key", str(root / "signing.pem"),
                    "--public-key", str(root / "verifying.pem"),
                    "--enrollment-token-file", str(root / "enrollment-token"),
                    "--export", str(root / "checkpoint")]
            if first:
                argv += ["--generate-keypair", "--import", str(core), "--import-format", "objdef"]
            with path.open("w") as output:
                process = subprocess.Popen(
                    argv, cwd=root, stdout=output, stderr=output,
                    env=dict(os.environ, XDG_CONFIG_HOME=str(root / "config"),
                             XDG_DATA_HOME=str(root / "state")),
                )
            deadline = time.monotonic() + 30
            while True:
                assert process.poll() is None, f"Test server exited: {path.read_text()[-6000:]}"
                try:
                    client = Client(port)
                    clients.append(client)
                    break
                except ConnectionRefusedError:
                    assert time.monotonic() < deadline, "Test server startup timed out"
                    time.sleep(0.05)
            client.command("connect ArchWizard cowbell-wire-test", "*** Connected ***")
            return client

        try:
            first = start(True)
            second = Client(port)
            clients.append(second)
            second.command("connect ArchWizard cowbell-wire-test", "*** Connected ***")
            second.eval_marker(
                'length(connections(player)) == 2 || raise(E_ASSERT); '
                'notify(connection(), "TWO_CONNECTIONS_OK"); return true;', "TWO_CONNECTIONS_OK")
            print("PASS same player has two live telnet connections", flush=True)
            first.eval_marker(
                '$housekeeping.sweep_interval = 1; $housekeeping:start(); '
                'add_property(#0, "wire_saved_schedule", $housekeeping.schedule_id, {player, "r"}); '
                'add_property(#0, "wire_saved_value", 42, {player, "r"}); '
                'notify(connection(), "SCHEDULE_REGISTERED"); return true;', "SCHEDULE_REGISTERED")
            deadline = time.monotonic() + 8
            while True:
                marker = first.eval_marker(
                    'const info = schedule_info($wire_saved_schedule); '
                    'notify(connection(), info["run_count"] > 0 ? "NATIVE_TICK_READY" | "NATIVE_TICK_WAIT"); '
                    'return true;', ("NATIVE_TICK_READY", "NATIVE_TICK_WAIT"),
                    timeout=min(2, max(0.1, deadline - time.monotonic())))
                if marker == "NATIVE_TICK_READY":
                    break
                assert time.monotonic() < deadline, "Native callback did not fire within eight seconds"
                time.sleep(0.1)
            first.eval_marker(
                'const info = schedule_info($wire_saved_schedule); '
                'info["fault_count"] == 0 || raise(E_ASSERT); '
                'add_property(#0, "wire_saved_run_count", info["run_count"], {player, "r"}); '
                'notify(connection(), "NATIVE_CALLBACK_OK"); return true;', "NATIVE_CALLBACK_OK")
            print("PASS native housekeeping callback fires without faults", flush=True)
            stop()
            resumed = start(False)
            resumed.eval_marker(
                '$wire_saved_value == 42 || raise(E_ASSERT); '
                '$housekeeping.schedule_id == $wire_saved_schedule || raise(E_ASSERT); '
                'schedule_valid($wire_saved_schedule) || raise(E_ASSERT); '
                'const info = schedule_info($wire_saved_schedule); '
                'info["run_count"] >= $wire_saved_run_count && info["fault_count"] == 0 || raise(E_ASSERT); '
                'notify(connection(), "RESTART_PERSISTED"); return true;', "RESTART_PERSISTED")
            deadline = time.monotonic() + 8
            while True:
                marker = resumed.eval_marker(
                    'const info = schedule_info($wire_saved_schedule); '
                    'notify(connection(), info["fault_count"] > 0 ? "NATIVE_RESUMED_FAULT" | '
                    'info["run_count"] > $wire_saved_run_count ? "NATIVE_RESUMED_READY" | "NATIVE_RESUMED_WAIT"); '
                    'return true;', ("NATIVE_RESUMED_READY", "NATIVE_RESUMED_WAIT", "NATIVE_RESUMED_FAULT"),
                    timeout=min(2, max(0.1, deadline - time.monotonic())))
                assert marker != "NATIVE_RESUMED_FAULT", "Native callback faulted after restart"
                if marker == "NATIVE_RESUMED_READY":
                    break
                assert time.monotonic() < deadline, "Native callback did not resume within eight seconds"
                time.sleep(0.1)
            print("PASS world property and native schedule state persist; callback resumes without faults", flush=True)
            resumed.eval_marker(
                '$housekeeping:stop(); notify(connection(), "STOP_QUEUED"); return true;', "STOP_QUEUED")
            resumed.eval_marker(
                '!schedule_valid($wire_saved_schedule) || raise(E_ASSERT); '
                'notify(connection(), "STOP_COMMITTED"); return true;', "STOP_COMMITTED")
            print("PASS native cancellation committed", flush=True)
            resumed.eval_marker(
                'dump_database(1) || raise(E_ASSERT, "Checkpoint export failed"); '
                'notify(connection(), "CHECKPOINT_COMPLETE"); return true;',
                "CHECKPOINT_COMPLETE", timeout=30)
            stop()
            exported = [path for path in (root / "checkpoint").rglob("*.moo") if path.is_file()]
            assert exported, "Checkpoint export is empty"
            assert any("wire_saved_value" in path.read_text() for path in exported), (
                "Checkpoint export lacks the committed test property"
            )
            print("PASS checkpoint export contains committed world state", flush=True)
        except Exception:
            for path in logs:
                print(f"--- {path.name} ---\n{path.read_text()[-6000:]}", flush=True)
            raise
        finally:
            stop()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server", type=Path, required=True)
    parser.add_argument("--core-dir", type=Path, required=True)
    arguments = parser.parse_args()
    run(arguments.server.resolve(), arguments.core_dir.resolve())
