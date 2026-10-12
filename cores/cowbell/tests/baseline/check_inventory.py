# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com>
# This program is free software under the GNU General Public License, version 3.
# It is distributed without any warranty. See <https://www.gnu.org/licenses/>.

"""Check nested source edits, additions, and deletions in a disposable core."""

from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile


command = sys.argv[1:]
if "/" in command[0]:
    command[0] = str(Path(command[0]).resolve())
if "cargo" in command[0] and "run" in command:
    index = command.index("run") + 1
    command[index:index] = ["--manifest-path", str(Path("../../Cargo.toml").resolve())]


def build(core):
    subprocess.run(
        ["make", "-C", str(core), "gen.objdir", "MOORC=" + shlex.join(command)],
        check=True,
        timeout=120,
    )


with tempfile.TemporaryDirectory(prefix="cowbell-inventory-") as temporary:
    core = Path(temporary) / "cores/cowbell"
    core.mkdir(parents=True)
    shutil.copytree("../make", core.parent / "make", ignore=shutil.ignore_patterns("__pycache__"))
    shutil.copy("Makefile", core)
    # Source discovery belongs to the Makefile. A small core tests it without
    # compiling all of Cowbell four more times; roundtrip covers the shipped core.
    (core / "src").mkdir()
    (core / "src/constants.moo").write_text("define ROOT = #1;\ndefine ARCH_WIZARD = #2;\n")
    (core / "src/root.moo").write_text(
        'object ROOT\nname: "Root"\nowner: ARCH_WIZARD\nendobject\n'
        'object ARCH_WIZARD\nname: "Wizard"\nparent: ROOT\nowner: ARCH_WIZARD\n'
        'wizard: true\nprogrammer: true\nendobject\n'
    )
    build(core)
    nested = core / "src" / "inventory" / "nested"
    nested.mkdir(parents=True)
    probe = nested / "inventory_probe.moo"
    source = '''object #90200 [
  import_export_id -> "inventory_probe",
  import_export_hierarchy -> {"inventory", "nested"}
]
  name: "inventory probe added"
  parent: #1
  owner: #2
endobject
'''
    probe.write_text(source)
    build(core)
    exported = core / "gen.objdir" / "inventory" / "nested" / "inventory_probe.moo"
    assert "inventory probe added" in exported.read_text(), "nested addition missing"
    probe.write_text(source.replace("probe added", "probe edited"))
    build(core)
    assert "inventory probe edited" in exported.read_text(), "nested edit missing"
    probe.unlink()
    build(core)
    assert not exported.exists(), "deleted source survived export"
    assert "#90200" not in (core / "gen.objdir" / "constants.moo").read_text()
print("PASS: nested additions, edits, and deletions rebuild the export")
