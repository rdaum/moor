"""Run bounded LLM scenarios through a disposable mooR server and its curl worker."""

import argparse
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request

from wire import Client, PASSWORD_HASH

MODEL = "deepseek/deepseek-v4.1-flash"
ENDPOINT = "https://openrouter.ai/api/v1/chat/completions"
MAX_BODY = 32768
MAX_RESPONSE = 1048576
MAX_TOKENS = 768


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # Credentials belong only to the fixed endpoint.
        return None


class Provider:
    """Hold credentials outside the world and cap every outgoing request."""

    def __init__(self, live=False, max_requests=12, timeout=240):
        self.key = os.environ.get("OPENROUTER_API_KEY") if live else None
        if live and not self.key:
            raise ValueError("OPENROUTER_API_KEY must be set for --live")
        if not 1 <= max_requests <= (20 if live else 40) or not 1 <= timeout <= 600:
            raise ValueError("Invalid request or timeout limit")
        self.token = secrets.token_urlsafe(32)
        self.max_requests = max_requests
        self.deadline = time.monotonic() + timeout
        self.rows = []
        self.phase = "startup"
        self.snapshots = {}
        self.restart_started = threading.Event()
        self.restart_release = threading.Event()
        self.opener = urllib.request.build_opener(NoRedirect)

    def prepare(self, payload):
        if time.monotonic() >= self.deadline:
            raise ValueError("Harness time limit reached")
        if len(self.rows) >= self.max_requests:
            raise ValueError("Harness request limit reached")
        if not isinstance(payload, dict) or payload.get("model") != MODEL:
            raise ValueError("Unexpected model")
        if payload.get("stream") is not False:
            raise ValueError("Streaming is not supported by this harness")
        messages = payload.get("messages")
        if not isinstance(messages, list) or not messages:
            raise ValueError("Expected nonempty messages")
        cap = payload.get("max_tokens", MAX_TOKENS)
        if type(cap) is not int or not 1 <= cap <= MAX_TOKENS:
            raise ValueError("Invalid output token limit")
        # The fixture has no need for provider routing or arbitrary extension fields.
        allowed = {"model", "messages", "stream", "tools", "temperature", "max_tokens", "tool_choice"}
        if set(payload) - allowed:
            raise ValueError("Unexpected request options")
        if "tool_choice" in payload and payload["tool_choice"] != "none":
            raise ValueError("Unexpected tool choice")
        return dict(payload, max_tokens=cap, reasoning={"enabled": False})

    def request(self, payload):
        payload = self.prepare(payload)
        row = {"scenario": self.phase, "status": "pending"}
        self.rows.append(row)  # Charge attempts before I/O, including failed requests.
        started = time.monotonic()
        try:
            if self.key:
                request = urllib.request.Request(
                    ENDPOINT, data=json.dumps(payload).encode(), method="POST",
                    headers={"Authorization": "Bearer " + self.key,
                             "Content-Type": "application/json"})
                remaining = max(0.1, min(25, self.deadline - time.monotonic()))
                with self.opener.open(request, timeout=remaining) as response:
                    raw = response.read(MAX_RESPONSE + 1)
                if len(raw) > MAX_RESPONSE:
                    raise ValueError("Provider response exceeds size limit")
                result = json.loads(raw)
            else:
                result = self.mock(payload)
            if not isinstance(result, dict) or not result.get("choices"):
                raise ValueError("Provider returned no choices")
            row["status"] = "ok"
            usage = result.get("usage", {})
            row["usage"] = {key: value for key, value in usage.items()
                            if key in {"prompt_tokens", "completion_tokens", "total_tokens", "cost"}
                            and type(value) in {int, float}}
            return result
        except urllib.error.HTTPError as error:
            row["status"] = f"http_{error.code}"
            error.close()
            raise ValueError(f"Provider HTTP status {error.code}") from None
        except Exception:
            row["status"] = "failed"
            # Provider error bodies can contain request data; never log or relay them.
            raise ValueError("Provider request failed") from None
        finally:
            row["seconds"] = round(time.monotonic() - started, 3)

    def fault(self, payload):
        assert not self.key, "Fault cases must never contact a paid provider"
        prepared = self.prepare(payload)
        previous = self.snapshots.setdefault(self.phase, prepared)
        assert previous == prepared, "A retry changed the request snapshot"
        attempt = sum(row["scenario"] == self.phase for row in self.rows) + 1
        case = self.phase.removeprefix("fault/")
        status = 200
        if case in {"retry429", "retry503"} and attempt < 3:
            status = int(case[-3:])
        elif case in {"401", "403", "400"}:
            status = int(case)
        elif case == "configuration_change":
            status = 503
        row = {"scenario": self.phase, "status": status, "seconds": 0.0}
        self.rows.append(row)
        started = time.monotonic()
        if case == "configuration_change":
            time.sleep(0.4)
        elif case == "timeout":
            time.sleep(31)
        elif case == "restart":
            self.restart_started.set()
            if not self.restart_release.wait(timeout=90):
                raise ValueError("Restart fixture was not released")
        row["seconds"] = round(time.monotonic() - started, 3)
        if status != 200:
            return status, b'{"error":"PRIVATE_PROVIDER_BODY"}'
        if case == "invalid_json":
            return 200, b"PRIVATE_PROVIDER_BODY not JSON"
        if case == "empty":
            return 200, b""
        if case == "oversized":
            return 200, b"x" * (MAX_RESPONSE + 1)
        result = self.mock(prepared)
        if case == "missing_usage":
            del result["usage"]
        return 200, json.dumps(result).encode()

    @staticmethod
    def mock(payload):
        messages = payload["messages"]
        if payload.get("tool_choice") == "none":
            unrelated = any("Visitors discuss their weekend with each other." in m.get("content", "")
                            for m in messages)
            message = {"role": "assistant", "content": "IGNORE" if unrelated else "ENGAGE"}
        elif payload.get("tools") and not any(m.get("role") == "tool" for m in messages):
            assert payload["tools"][0]["function"]["name"] == "mark"
            message = {"role": "assistant", "content": "", "tool_calls": [
                {"id": "fixture-call", "type": "function",
                 "function": {"name": "mark", "arguments": "{}"}}]}
        else:
            message = {"role": "assistant", "content": "READY"}
        return {"choices": [{"message": message, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 20, "completion_tokens": 4, "total_tokens": 24}}


@contextmanager
def proxy(provider):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_GET(self):
            if (provider.key or self.path != "/models" or
                    self.headers.get("Authorization") != "Bearer " + provider.token or
                    len(provider.rows) >= provider.max_requests or time.monotonic() >= provider.deadline):
                self.send_error(403)
                return
            provider.rows.append({"scenario": provider.phase, "status": "ok", "seconds": 0.0})
            body = json.dumps({"data": [{"id": MODEL, "owned_by": "deepseek"}]}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            self.connection.settimeout(5)
            try:
                if self.path != "/chat/completions":
                    raise ValueError("Unexpected path")
                if self.headers.get("Authorization") != "Bearer " + provider.token:
                    raise ValueError("Invalid fixture token")
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= MAX_BODY:
                    raise ValueError("Request exceeds size limit")
                payload = json.loads(self.rfile.read(length))
                if provider.phase.startswith("fault/"):
                    status, body = provider.fault(payload)
                    result = None
                else:
                    result = provider.request(payload)
                    status = 200
            except (ValueError, OSError):
                result, status = {"error": "Bounded test provider rejected the request"}, 502
            if result is not None:
                body = json.dumps(result).encode()
            try:
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                pass

    server = HTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/chat/completions"
    finally:
        provider.restart_release.set()
        server.shutdown()
        server.server_close()
        thread.join(timeout=30)


class World:
    """Own one disposable persistent server and reconnect after a clean restart."""

    def __init__(self, argv, import_args, root, environment, port):
        self.argv = argv
        self.import_args = import_args
        self.root = root
        self.environment = environment
        self.port = port
        self.process = None
        self.client = None
        self.starts = 0

    def start(self):
        assert self.process is None, "Test server is already running"
        argv = self.argv + (self.import_args if self.starts == 0 else [])
        log = self.root / f"server-{self.starts}.log"
        self.starts += 1
        with log.open("w") as output:
            self.process = subprocess.Popen(argv, cwd=self.root, env=self.environment,
                                            stdout=output, stderr=output)
        deadline = time.monotonic() + 45
        while self.client is None:
            assert self.process.poll() is None, "Test server exited: " + log.read_text()[-6000:]
            try:
                self.client = Client(self.port)
            except ConnectionRefusedError:
                assert time.monotonic() < deadline, "Test server startup timed out"
                time.sleep(0.05)
        self.client.command("connect ArchWizard cowbell-wire-test", "*** Connected ***")
        return self.client

    def stop(self):
        if self.client:
            self.client.close()
            self.client = None
        if self.process is None:
            return
        process, self.process = self.process, None
        if process.poll() is None:
            process.terminate()
        try:
            code = process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
            raise AssertionError("Owned LLM test server did not stop cleanly")
        assert code == 0, f"LLM test server stopped with status {code}"


@contextmanager
def world(server, core_dir):
    with tempfile.TemporaryDirectory(prefix="cowbell-llm-") as directory:
        root = Path(directory)
        core = root / "src"
        shutil.copytree(core_dir, core)
        shutil.copy(Path(__file__).parent / "llm/provider_scenarios.moo", core)
        arch = core / "arch_wizard.moo"
        source, count = re.subn(
            r'(override password(?:\s*\([^;\n]*\))?\s*=\s*).*?;',
            lambda match: match[1] + "<PASSWORD, {" + json.dumps(PASSWORD_HASH) + "}>;",
            arch.read_text())
        assert count == 1, "Missing wizard password fixture"
        arch.write_text(source)
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
    enabled: true
    health_check_port: 0
""")
        argv = [str(server), str(root / "data"), "--config-file", str(config),
                "--private-key", str(root / "signing.pem"),
                "--public-key", str(root / "verifying.pem"),
                "--enrollment-token-file", str(root / "enrollment-token"),
                "--export", str(root / "checkpoint")]
        import_args = ["--generate-keypair", "--import", str(core), "--import-format", "objdef"]
        environment = dict(os.environ, XDG_CONFIG_HOME=str(root / "config"),
                           XDG_DATA_HOME=str(root / "state"))
        environment.pop("OPENROUTER_API_KEY", None)
        running = World(argv, import_args, root, environment, port)
        try:
            running.start()
            yield running
        finally:
            running.stop()


def run_restart(running, provider, endpoint):
    """Interrupt real local HTTP work, then verify persisted jobs and exactly-once settlement."""
    assert not provider.key, "Restart cases must never contact a paid provider"
    provider.phase = "fault/restart"
    before = len(provider.rows)
    code = (f'#90032:start_restart({json.dumps(endpoint)}, {json.dumps(provider.token)}, '
            f'{json.dumps(MODEL)}); notify(connection(), "LLM_RESTART_STARTED"); return true;')
    running.client.eval_marker(code, "LLM_RESTART_STARTED")
    assert provider.restart_started.wait(timeout=15), "No actual HTTP request reached the fixture"
    running.client.eval_marker(
        '#90032:capture_restart(); notify(connection(), "LLM_REQUEST_HELD"); return true;',
        "LLM_REQUEST_HELD")
    running.stop()
    provider.restart_release.set()
    client = running.start()
    deadline = min(provider.deadline, time.monotonic() + 80)
    while True:
        marker = client.eval_marker(
            'notify(connection(), #90032:restart_ready() ? "LLM_RESTART_READY" | "LLM_RESTART_WAIT"); return true;',
            ("LLM_RESTART_READY", "LLM_RESTART_WAIT"))
        if marker == "LLM_RESTART_READY":
            break
        assert time.monotonic() < deadline, "LLM job or reservation did not reconcile after restart"
        time.sleep(0.2)
    client.eval_marker(
        '#90032:verify_restart(); notify(connection(), "LLM_RESTART_VERIFIED"); return true;',
        "LLM_RESTART_VERIFIED")
    assert len(provider.rows) == before + 1, "Restart replayed an interrupted model request"
    print("PASS LLM restart: interrupted HTTP, job cleanup, conservative settlement, no replay", flush=True)
    running.stop()
    client = running.start()
    client.eval_marker(
        '#90032:verify_restart(); notify(connection(), "LLM_SECOND_RESTART_OK"); return true;',
        "LLM_SECOND_RESTART_OK")
    provider.phase = "restart/fresh-request"
    client.eval_marker(
        '#90032:request_after_restart(); notify(connection(), "LLM_FRESH_REQUEST_OK"); return true;',
        "LLM_FRESH_REQUEST_OK")
    assert len(provider.rows) == before + 2, "Fresh request must execute exactly once"
    print("PASS LLM restart: settlement persists and saved client serves a fresh request", flush=True)


def run(args):
    provider = Provider(args.live, args.max_requests if args.max_requests is not None else (12 if args.live else 40), args.timeout)
    try:
        with proxy(provider) as endpoint, world(args.server.resolve(), args.core_dir.resolve()) as running:
            if args.restart_only:
                run_restart(running, provider, endpoint)
                return
            client = running.client
            for name, prototype, delegate in (("llm", "$llm_agent", "$llm_agent_tool"),):
                for scenario in ("completion", "tool", "compaction"):
                    provider.phase = f"{name}/{scenario}"
                    remaining = provider.deadline - time.monotonic()
                    assert remaining > 0, "Harness time limit reached"
                    code = (f'#90032:run_case({prototype}, {delegate}, '
                            f'{json.dumps(endpoint)}, {json.dumps(provider.token)}, '
                            f'{json.dumps(MODEL)}, {json.dumps(scenario)}); '
                            'notify(connection(), "LLM_CASE_OK"); return true;')
                    client.eval_marker(code, "LLM_CASE_OK", timeout=min(90, remaining))
                    print(f"PASS {provider.phase}", flush=True)
            provider.phase = "observer/triage"
            code = (f'#90032:run_observer({json.dumps(endpoint)}, {json.dumps(provider.token)}, '
                    f'{json.dumps(MODEL)}); notify(connection(), "LLM_OBSERVER_OK"); return true;')
            remaining = provider.deadline - time.monotonic()
            assert remaining > 0, "Harness time limit reached"
            client.eval_marker(code, "LLM_OBSERVER_OK", timeout=min(90, remaining))
            print("PASS observer/triage", flush=True)
            assert provider.rows and all(row["status"] == "ok" for row in provider.rows), (
                "Provider failures occurred during the normal scenarios")
            if not args.live:
                provider.phase = "accounting"
                code = (f'#90032:run_accounting({json.dumps(endpoint)}, {json.dumps(provider.token)}, '
                        f'{json.dumps(MODEL)}); notify(connection(), "LLM_ACCOUNTING_OK"); return true;')
                client.eval_marker(code, "LLM_ACCOUNTING_OK", timeout=90)
                print("PASS accounting/purposes-and-npc", flush=True)
                for scenario in ("retry429", "retry503", "401", "403", "400", "invalid_json",
                                 "empty", "missing_usage", "configuration_change", "oversized", "timeout"):
                    provider.phase = "fault/" + scenario
                    before = len(provider.rows)
                    code = (f'#90032:run_fault({json.dumps(endpoint)}, {json.dumps(provider.token)}, '
                            f'{json.dumps(MODEL)}, {json.dumps(scenario)}); '
                            'notify(connection(), "LLM_FAULT_OK"); return true;')
                    client.eval_marker(code, "LLM_FAULT_OK", timeout=90)
                    expected = 3 if scenario.startswith("retry") else 1
                    assert len(provider.rows) - before == expected, "Unexpected HTTP attempt count"
                    print(f"PASS {provider.phase} ({expected} attempt(s))", flush=True)
                run_restart(running, provider, endpoint)
    finally:
        print(json.dumps({"mode": "live" if args.live else "fixture", "model": MODEL,
                          "requests": len(provider.rows), "measurements": provider.rows}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server", type=Path, required=True)
    parser.add_argument("--core-dir", type=Path, default=Path("src"))
    parser.add_argument("--live", action="store_true", help="Spend the environment API key's budget")
    parser.add_argument("--restart-only", action="store_true", help="Run only the local persistent restart scenario")
    parser.add_argument("--max-requests", type=int)
    parser.add_argument("--timeout", type=int, default=240)
    args = parser.parse_args()
    if args.live and args.restart_only:
        parser.error("--restart-only cannot use --live")
    run(args)
