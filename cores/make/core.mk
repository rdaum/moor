# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com>
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

# Include after core-specific settings. Specialized targets stay in each core.
include $(dir $(lastword $(MAKEFILE_LIST)))compiler.mk

.DEFAULT_GOAL := build
.DELETE_ON_ERROR:
# Exports and test fixtures use shared paths within each core.
.NOTPARALLEL:

CORE_OUTPUT_TARGET ?= gen.objdir
CHECK_TARGETS ?= test roundtrip
CLEAN_EXTRA ?=
EXPORT_FORBIDDEN_IDS ?=

.PHONY: build output gen.objdir gen.moo-textdump roundtrip reformat rebuild check check-make check-sources clean help FORCE

build: gen.objdir
output: $(CORE_OUTPUT_TARGET)
check: check-make $(CHECK_TARGETS)

check-make:
	$(PYTHON) -m unittest discover -s $(CORE_MAKE_DIR)tests -v
FORCE:

# Always read the complete tree, including nested edits and removed files.
check-sources:
	@test -n "$$(find "$(SRC_DIRECTORY)" -type f -name '*.moo' -print -quit)" || { \
		echo "FAIL: no objdef sources in $(SRC_DIRECTORY)" >&2; exit 1; }

gen.objdir: check-sources
	mkdir -p "$(OUTPUT_DIRECTORY)"
	rm -rf "$(OUTPUT_DIRECTORY)/gen.objdir" "$(OUTPUT_DIRECTORY)/gen.objdir.in-progress"
	$(CORE_COMPILER) --src-objdef-dir "$(SRC_DIRECTORY)" --include-baselines=false --out-objdef-dir "$(OUTPUT_DIRECTORY)/gen.objdir"
	@test -n "$$(find "$(OUTPUT_DIRECTORY)/gen.objdir" -type f -name '*.moo' -print -quit)" || { \
		echo "FAIL: compiler did not produce an objdef export" >&2; exit 1; }
ifneq ($(strip $(EXPORT_FORBIDDEN_IDS)),)
	@! grep -qE '$(EXPORT_FORBIDDEN_IDS)' "$(OUTPUT_DIRECTORY)/gen.objdir/constants.moo" || { \
		echo "FAIL: test fixtures entered shipped export" >&2; exit 1; }
endif

gen.moo-textdump: check-sources
	mkdir -p "$(OUTPUT_DIRECTORY)"
	$(CORE_COMPILER) --src-objdef-dir "$(SRC_DIRECTORY)" --out-textdump "$(OUTPUT_DIRECTORY)/gen.moo-textdump"
	@test -s "$(OUTPUT_DIRECTORY)/gen.moo-textdump" || { echo "FAIL: compiler did not produce a textdump" >&2; exit 1; }

roundtrip: gen.objdir
	rm -rf "$(OUTPUT_DIRECTORY)/gen.objdir.rt" "$(OUTPUT_DIRECTORY)/gen.objdir.rt.in-progress"
	$(CORE_COMPILER) --src-objdef-dir "$(OUTPUT_DIRECTORY)/gen.objdir" --include-baselines=false --out-objdef-dir "$(OUTPUT_DIRECTORY)/gen.objdir.rt"
	@test -n "$$(find "$(OUTPUT_DIRECTORY)/gen.objdir.rt" -type f -name '*.moo' -print -quit)" || { echo "FAIL: reimport export is empty" >&2; exit 1; }
	diff -r "$(OUTPUT_DIRECTORY)/gen.objdir" "$(OUTPUT_DIRECTORY)/gen.objdir.rt"

# This explicitly overwrites working sources. Export removes source comments.
# Verify reimport first; never replace sources after a failed roundtrip.
reformat: roundtrip
	cp -r "$(OUTPUT_DIRECTORY)/gen.objdir/." "$(SRC_DIRECTORY)/"

rebuild: reformat

clean:
	rm -rf "$(OUTPUT_DIRECTORY)/gen.objdir" "$(OUTPUT_DIRECTORY)/gen.objdir.in-progress" \
		"$(OUTPUT_DIRECTORY)/gen.objdir.rt" "$(OUTPUT_DIRECTORY)/gen.objdir.rt.in-progress"
	rm -f "$(OUTPUT_DIRECTORY)/gen.moo-textdump"
ifneq ($(strip $(CLEAN_EXTRA)),)
	rm -rf $(CLEAN_EXTRA)
endif

help:
	@printf '%s\n' \
		'make / make build      Export src/ to gen.objdir/ without baseline metadata.' \
		'make roundtrip         Export, reimport, and compare both objdef trees.' \
		'make reformat/rebuild  Check roundtrip, then overwrite src/ with the export.' \
		'make gen.moo-textdump  Export a traditional MOO textdump.' \
		'make test              Run core tests, or validate import if it has no suite.' \
		'make check             Run shared Makefile tests, then: $(CHECK_TARGETS).' \
		'make clean             Remove generated output; keep sources and running databases.' \
		'MOORC_TYPE=cargo|direct|docker  Choose the compiler launcher (default: cargo).' \
		'MOORC_PROFILE=dev|release      Choose the build profile (default: $(MOORC_PROFILE)).' \
		'MOORC=...              Override the compiler command; core OPTIONS still apply.' \
		'DEBUG=1                Run a host compiler under gdb.' \
		$(CORE_HELP)

include $(CORE_MAKE_DIR)tests.mk
