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
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parent.parent
BINARIES = ["moor-daemon", "moor", "moorc", "moor-emh"]


def run(binary, args, env, success, stdout_only=False):
    result = subprocess.run([str(ROOT / "target/debug" / binary), *args], env=env,
                            text=True, capture_output=True, timeout=45)
    assert (result.returncode == 0) == success, (binary, result.returncode, result.stdout, result.stderr)
    return result.stdout if stdout_only else result.stdout + result.stderr


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

    validation = json.loads(run("moorc", [*pg, "--validate-storage"], env, True, stdout_only=True))
    assert validation["relation_rows"]["object_flags"] > 0
    original = contents(first)
    assert original and contents(postgres_dump) == original
    assert contents(final) == original
    backup_restore(root, env, first, features)
    empty = root / "empty"
    empty.mkdir()
    for storage in [pg, restored]:
        output = run("moorc", [*features, *storage, "--src-objdef-dir", str(empty),
                              "--run-tests=true", "--test-wizard=2", "--test-phases=3",
                              "--test-filter=#668:test_string_history_append", "--test-timeout=30",
                              "--test-args={2, 16, 128, 5, 0, 1, 1, 0}"], env, True)
        assert "Test #668:test_string_history_append passed" in output, output
    print(f"moorc: Fjall/objdef/PostgreSQL/objdef/Fjall round trip preserved {len(original)} files; both functional probes passed")


def backup_restore(root, env, first, features):
    """Restore schema and whole-database archives with distinct setup/runtime/read roles."""
    import configparser
    import hashlib
    import secrets

    suffix = uuid.uuid4().hex
    owner, runtime, reader = [f"{kind}_{suffix}" for kind in ["owner", "runtime", "reader"]]
    source, whole, scoped, interrupted, converted = [
        f"{kind}_{suffix}" for kind in ["source", "whole", "scoped", "interrupted", "converted"]]
    databases = [source, whole, scoped, interrupted, converted]
    service_file, pass_file = root / "restore-services", root / "restore-passwords"
    services = configparser.ConfigParser(interpolation=None)
    services.read(env["PGSERVICEFILE"])
    base = dict(services["moor_adapter"])
    passwords = {role: secrets.token_hex(24) for role in [owner, runtime, reader]}
    pass_file.write_text(Path(env["PGPASSFILE"]).read_text() + "".join(
        f"*:*:*:{role}:{password}\n" for role, password in passwords.items()), encoding="utf8")
    pass_file.chmod(0o600)
    test_env = {**env, "PGSERVICEFILE": str(service_file), "PGPASSFILE": str(pass_file)}
    for database in databases:
        for role in [owner, runtime, reader]:
            services[f"{database}_{role}"] = {**base, "dbname": database, "user": role}
        services[f"{database}_admin"] = {**base, "dbname": database}
    with service_file.open("w") as output:
        services.write(output, space_around_delimiters=False)
    service_file.chmod(0o600)

    def utility_command(program, args):
        # Match server/client major versions, including Docker fixtures with newer servers.
        container = env.get("MOOR_PG_TEST_CONTAINER_ID")
        if container:
            command = ["docker", "exec", "-i", container, program, "-h", "/moor-socket", "-U", "postgres", *args]
        else:
            bindir = env.get("MOOR_PG_TEST_NATIVE_BIN", "/usr/lib/postgresql/17/bin")
            command = [str(Path(bindir) / program), *args]
        return command

    def utility(program, args, input=None):
        result = subprocess.run(utility_command(program, args), env={**env, "PGSERVICE": "moor_adapter"},
                                input=input, capture_output=True, timeout=90)
        assert result.returncode == 0, (program, result.stderr.decode())
        return result.stdout

    def sql(database, statement):
        return utility("psql", ["-X", "-q", "-A", "-t", "-v", "ON_ERROR_STOP=1", "-d", database],
                       statement.encode()).decode().strip()

    def storage(database, role):
        return ["--storage-backend=postgres", "--pg-hostaddr=127.0.0.1", "--pg-schema=moor",
                f"--pg-service={database}_{role}"]

    def validate(database):
        return json.loads(run("moorc", [*storage(database, reader), "--validate-storage"], test_env, True, stdout_only=True))

    def physical(database):
        tables = sql(database, "SELECT tablename FROM pg_tables WHERE schemaname='moor' ORDER BY tablename;").splitlines()
        return {table: hashlib.sha256(sql(database,
                f'SELECT to_jsonb(t)::text FROM moor."{table}" t ORDER BY to_jsonb(t)::text;').encode()).hexdigest()
                for table in tables}

    def inspection(database):
        views = sql(database, "SELECT viewname FROM pg_views WHERE schemaname='moor' ORDER BY viewname;").splitlines()
        assert len(views) == 9, views
        return {view: hashlib.sha256(sql(database,
                f'SET ROLE {reader}; SELECT to_jsonb(t)::text FROM moor."{view}" t ORDER BY to_jsonb(t)::text;').encode()).hexdigest()
                for view in views}

    def grants(database):
        sql(database, f"""GRANT USAGE ON SCHEMA moor TO {runtime}, {reader};
            GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA moor TO {runtime};
            GRANT SELECT ON ALL TABLES IN SCHEMA moor TO {reader};""")

    def await_sql(database, statement):
        deadline = time.monotonic() + 20
        while sql(database, statement) != "t":
            assert time.monotonic() < deadline, "conversion fixture did not reach its SQL barrier"
            time.sleep(0.02)

    def interrupted_conversion():
        run("moorc", [*storage(interrupted, owner), "--init-storage"], test_env, True)
        grants(interrupted)
        blocker_name = "conversion_blocker_" + suffix
        blocker_key = int(suffix[:7], 16)
        # A trigger blocks execution only. A table lock could stop startup's prepared statements.
        sql(interrupted, f"""CREATE FUNCTION moor.block_import() RETURNS trigger LANGUAGE plpgsql AS
            $$ BEGIN PERFORM pg_advisory_xact_lock(347715,{blocker_key}); RETURN NEW; END $$;
            CREATE TRIGGER block_import BEFORE INSERT ON moor.object_propvalues
            FOR EACH ROW EXECUTE FUNCTION moor.block_import();""")
        blocker = subprocess.Popen(utility_command("psql", [
            "-X", "-q", "-v", "ON_ERROR_STOP=1", "-d", interrupted, "-c",
            f"SET application_name='{blocker_name}'; BEGIN; SELECT pg_advisory_xact_lock(347715,{blocker_key}); SELECT pg_sleep(60);"
        ]), env={**env, "PGSERVICE": "moor_adapter"}, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        importer = None
        try:
            await_sql(interrupted, f"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name='{blocker_name}' AND wait_event='PgSleep');")
            with (root / "interrupted-import.log").open("wb") as log:
                importer = subprocess.Popen([str(ROOT / "target/debug/moorc"), *features,
                    *storage(interrupted, runtime), "--src-objdef-dir", str(first)],
                    env=test_env, stdout=log, stderr=log)
                await_sql(interrupted, f"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname='{interrupted}' AND wait_event_type='Lock' AND query LIKE '%object_propvalues%');")
                importer.kill()
                assert importer.wait(timeout=10) != 0
        finally:
            if importer is not None and importer.poll() is None:
                importer.kill()
                importer.wait(timeout=10)
            sql(interrupted, f"SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='{blocker_name}';")
            try:
                blocker.wait(timeout=10)
            except subprocess.TimeoutExpired:
                blocker.kill()
                blocker.wait(timeout=10)
        # Recovery discards the interrupted destination; it never edits source format markers.
        assert physical(source) == rows
        assert validate(source) == before
        run("moorc", [*storage(converted, owner), "--init-storage"], test_env, True)
        grants(converted)
        exported = root / "converted-export"
        run("moorc", [*features, *storage(converted, runtime), "--src-objdef-dir", str(first),
                      "--out-objdef-dir", str(exported)], test_env, True)
        expected = {p.relative_to(first): p.read_bytes() for p in first.rglob("*") if p.is_file()}
        actual = {p.relative_to(exported): p.read_bytes() for p in exported.rglob("*") if p.is_file()}
        assert actual == expected
        assert validate(converted)["database_id"] != before["database_id"]
        empty = root / "conversion-empty"
        empty.mkdir()
        output = run("moorc", [*features, *storage(converted, runtime), "--src-objdef-dir", str(empty),
                              "--run-tests=true", "--test-wizard=2", "--test-phases=3",
                              "--test-filter=#668:test_string_history_append", "--test-timeout=30",
                              "--test-args={2, 16, 128, 5, 0, 1, 1, 0}"], test_env, True)
        assert "Test #668:test_string_history_append passed" in output
        validate(converted)
        assert physical(source) == rows
        print("PostgreSQL: interrupted objdef conversion left the source unchanged; a fresh destination passed export comparison, validation, and behavior probes")

    try:
        for role, password in passwords.items():
            sql("postgres", f"CREATE ROLE {role} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE PASSWORD '{password}';")
        for database in databases:
            sql("postgres", f"CREATE DATABASE {database} OWNER {owner} TEMPLATE template0 ENCODING 'UTF8';")
        run("moorc", [*storage(source, owner), "--init-storage"], test_env, True)
        grants(source)
        run("moorc", [*features, *storage(source, runtime), "--src-objdef-dir", str(first),
                      "--out-objdef-dir", str(root / "backup-original")], test_env, True)
        # Add a separate schema to prove whole-database versus schema-scoped selection.
        sql(source, f"CREATE SCHEMA auxiliary AUTHORIZATION {owner}; CREATE TABLE auxiliary.marker(value int); INSERT INTO auxiliary.marker VALUES(42); GRANT USAGE ON SCHEMA auxiliary TO {reader}; GRANT SELECT ON auxiliary.marker TO {reader};")
        before = validate(source)
        rows = physical(source)
        inspected = inspection(source)
        assert before["property_values"] > 0
        # A genuine read-only login can validate but cannot claim a writer epoch or initialize.
        run("moorc", [*storage(source, reader), "--src-objdef-dir", str(first),
                      "--out-objdef-dir", str(root / "forbidden")], test_env, False)
        assert validate(source) == before
        for database, scope in [(whole, []), (scoped, ["--schema=moor"])]:
            archive = utility("pg_dump", ["-d", source, "--format=custom", f"--role={reader}", *scope])
            utility("pg_restore", ["-d", database, "--no-owner", "--no-acl", "--exit-on-error",
                                   "--single-transaction", f"--role={owner}"], archive)
            grants(database)
            # No writer has opened the restore yet: identities, epochs, timestamps, counters,
            # physical UUIDs, source, property records and every other stored cell must match.
            assert physical(database) == rows
            assert validate(database) == before
            assert inspection(database) == inspected
            assert sql(database, "SELECT to_regclass('auxiliary.marker') IS NOT NULL;") == ("t" if database == whole else "f")
            exported = root / database
            empty = root / (database + "_empty")
            empty.mkdir()
            run("moorc", [*features, *storage(database, runtime), "--src-objdef-dir", str(empty),
                          "--out-objdef-dir", str(exported)], test_env, True)
            expected = {p.relative_to(first): p.read_bytes() for p in first.rglob("*") if p.is_file()}
            actual = {p.relative_to(exported): p.read_bytes() for p in exported.rglob("*") if p.is_file()}
            assert actual == expected
            output = run("moorc", [*features, *storage(database, runtime), "--src-objdef-dir", str(empty),
                                  "--run-tests=true", "--test-wizard=2", "--test-phases=3",
                                  "--test-filter=#668:test_string_history_append", "--test-timeout=30",
                                  "--test-args={2, 16, 128, 5, 0, 1, 1, 0}"], test_env, True)
            assert "Test #668:test_string_history_append passed" in output
            validate(database)
        interrupted_conversion()
        print("PostgreSQL: schema and whole-database restores preserved every stored cell; read-only validation, restricted runtime export, and functional probes passed")
    finally:
        for database in databases:
            sql("postgres", f"DROP DATABASE IF EXISTS {database} WITH (FORCE);")
        for role in [reader, runtime, owner]:
            sql("postgres", f"DROP ROLE IF EXISTS {role};")


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
            for seconds in ["0", "18446744073709551615"]:
                output = run(binary, [*positional, "--persistence-shutdown-timeout-seconds", seconds], env, False)
                assert "persistence shutdown timeout" in output, output
                assert list(root.iterdir()) == [], list(root.iterdir())
            selected = [*positional, "--storage-backend", "postgres"]
            if args.disabled:
                for flags in [[], ["--validate-storage"], ["--install-storage-views"]]:
                    output = run(binary, [*selected, *flags], env, False)
                    assert "PostgreSQL support is disabled" in output, output
                    assert list(root.iterdir()) == [], list(root.iterdir())
                print(f"{binary}: disabled feature rejected before filesystem changes")
                continue
            postgres = ["--pg-service", "moor_adapter", "--pg-hostaddr", "127.0.0.1",
                        "--pg-schema", "cli_" + uuid.uuid4().hex]
            for extra in [["--db", "world.db"], ["--pg-query-timeout-seconds", "0"],
                          ["--pg-max-exports", "0"], ["--pg-max-exports", "65"],
                          ["--pg-max-pending-bytes", "0"],
                          ["--persistence-shutdown-timeout-seconds", "0"],
                          ["--persistence-shutdown-timeout-seconds", "18446744073709551615"]]:
                run(binary, [*selected, *postgres, *extra, "--init-storage"], env, False)
                assert list(root.iterdir()) == [], list(root.iterdir())
            output = run(binary, [*selected, *postgres, "--init-storage"], env, True)
            assert "Initialized PostgreSQL database" in output, output
            assert list(root.iterdir()) == [], list(root.iterdir())
            output = run(binary, [*selected, *postgres, "--init-storage"], env, False)
            assert "42P06" in output, output
            assert list(root.iterdir()) == [], list(root.iterdir())
            output = run(binary, [*selected, *postgres, "--validate-storage"], env, True, stdout_only=True)
            report = json.loads(output)
            assert report["writer_epoch"] == 0 and report["relation_rows"]["object_flags"] == 0
            assert list(root.iterdir()) == [], list(root.iterdir())
            run(binary, [*selected, *postgres, "--validate-storage", "--init-storage"], env, False)
            run(binary, [*selected, *postgres, "--install-storage-views"], env, True)
            after = json.loads(run(binary, [*selected, *postgres, "--validate-storage"], env, True, stdout_only=True))
            assert after == report
            assert list(root.iterdir()) == [], list(root.iterdir())
            print(f"{binary}: explicit setup, view installation, read-only validation, and path rejection passed")

        if args.disabled:
            return
        for binary in ["moor-daemon", "moor"]:
            schema = "yaml_" + uuid.uuid4().hex
            config = root / "storage.yaml"
            config.write_text(f"storage:\n  backend: postgres\n  postgres:\n    service: moor_adapter\n    schema: {schema}\n    hostaddr: 127.0.0.1\n", encoding="utf8")
            invocation = [str(root / "data"), "--config-file", str(config), "--init-storage"]
            run(binary, [*invocation, "--pg-schema", schema + "_override"], env, True)
            run(binary, invocation, env, True)
            valid_config = config.read_text(encoding="utf8")
            config.write_text(valid_config.replace("  backend: postgres", "  backend: postgres\n  shutdown_timeout_seconds: 0"), encoding="utf8")
            output = run(binary, invocation, env, False)
            assert "persistence shutdown timeout" in output, output
            run(binary, [*invocation, "--pg-schema", schema + "_shutdown_override",
                         "--persistence-shutdown-timeout-seconds", "9"], env, True)
            config.write_text(valid_config + "    max_pending_bytes: 0\n", encoding="utf8")
            output = run(binary, invocation, env, False)
            assert "max_pending_bytes must be positive" in output, output
            run(binary, [*invocation, "--pg-schema", schema + "_bytes_override",
                         "--pg-max-pending-bytes", "8192"], env, True)
            config.write_text(valid_config, encoding="utf8")
            config.write_text(config.read_text(encoding="utf8") + "database: {}\n", encoding="utf8")
            output = run(binary, invocation, env, False)
            assert "Fjall database table settings" in output, output
            assert list(root.iterdir()) == [config], list(root.iterdir())
            config.unlink()
            print(f"{binary}: YAML selection and explicit CLI overrides passed")

        snapshot_roundtrip(root, env)


if __name__ == "__main__":
    main()
