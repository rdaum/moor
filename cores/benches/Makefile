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

OPTIONS = --use-boolean-returns true \
          --use-symbols-in-builtins true \
          --custom-errors true \
          --use-uuobjids true \
          --anonymous-objects false

DURATION ?= 30
SUBSCRIBERS ?= 256
WORK_PER_TICK ?= 20
INTENT_ITERATIONS ?= 20
APPEND_MODE ?= 0
HISTORY_WRITERS ?= 4
HISTORY_ENTRIES ?= 1024
HISTORY_ENTRY_BYTES ?= 2048
HISTORY_APPENDS ?= 20
HISTORY_APPEND_DELAY ?= 0
HISTORY_SETTLE_SECONDS ?= 5
HISTORY_APPEND_WIDTH ?= 1
HISTORY_MUTATION_MODE ?= 0
HISTORY_TIMEOUT ?= 360
HISTORY_PHASES ?= $(if $(filter 1,$(POSTGRES)),3,)
FANOUT ?= 4
CHECKS_PER_TICK ?= 2
STATE_WRITES ?= 4
DELAY_RATIO ?= 10
UPDATE_HZ ?= 20
SLOW_DB_PATH ?= /tmp/moor-slow-mount/bench.db
PERF_STAT_OUTPUT ?= perf-combat.stat
PERF_STAT_EVENTS ?= task-clock,cycles:u,instructions:u,branches:u,branch-misses:u,cache-references:u,cache-misses:u,context-switches,cpu-migrations,page-faults

POSTGRES ?= 0
EXPORT_ARGS = --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir
ifeq ($(POSTGRES),1)
ifneq ($(filter build output rebuild reformat roundtrip gen.objdir gen.moo-textdump %-perf %-perf-stat %-perf-sched %-slow,$(MAKECMDGOALS)),)
$(error POSTGRES=1 supports ordinary benchmark targets; export, perf, and slow-disk targets are not supported)
endif
MOORC = ../../scripts/bench-postgres.sh
EXPORT_ARGS =
endif

MOORC_PROFILE ?= release
TEST_WIZARD ?= 2
TEST_PROGRAMMER ?= 2
TEST_PLAYER ?= 4
CORE_HELP = 'make bench              Run all benchmark methods.' \
            'make bench-write-stress / bench-string-history / bench-combat-stress' \
            'make bench-intent-match Run intent matching measurements.'
include ../make/core.mk
ifeq ($(POSTGRES),1)
.DEFAULT_GOAL := bench
endif

.PHONY: bench bench-game-update bench-write-stress bench-string-history bench-combat-stress bench-intent-match bench-write-stress-perf bench-combat-stress-perf bench-combat-stress-perf-stat bench-write-stress-perf-sched bench-write-stress-slow

bench: check-sources
	$(CORE_COMPILER) --src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
	$(TEST_PRINCIPALS) --run-tests true

bench-game-update: check-sources
	$(CORE_COMPILER) --src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
	--test-wizard=2 --run-tests true --test-filter "#668:test_run_bench"

bench-write-stress: check-sources
	$(CORE_COMPILER) --src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
	--test-wizard=2 --run-tests true --test-phases 1 --test-filter "#668:test_write_stress" --test-timeout $$(($(DURATION) + 60)) --test-args "{$(DURATION), $(SUBSCRIBERS), $(WORK_PER_TICK), $(APPEND_MODE)}"

bench-string-history: check-sources
	$(CORE_COMPILER) --src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
	--test-wizard=2 --run-tests true --test-filter "#668:test_string_history_append" $(if $(HISTORY_PHASES),--test-phases $(HISTORY_PHASES)) --test-timeout $(HISTORY_TIMEOUT) --test-args "{$(HISTORY_WRITERS), $(HISTORY_ENTRIES), $(HISTORY_ENTRY_BYTES), $(HISTORY_APPENDS), $(HISTORY_APPEND_DELAY), $(HISTORY_SETTLE_SECONDS), $(HISTORY_APPEND_WIDTH), $(HISTORY_MUTATION_MODE)}"

bench-combat-stress: check-sources
	$(CORE_COMPILER) --src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
	--test-wizard=2 --run-tests true --test-phases 1 --test-filter "#668:test_combat_stress" --test-timeout $$(($(DURATION) + 60)) --test-args "{$(DURATION), $(SUBSCRIBERS), $(WORK_PER_TICK), $(FANOUT), $(CHECKS_PER_TICK), $(STATE_WRITES), $(DELAY_RATIO), $(UPDATE_HZ)}"

bench-intent-match: check-sources
	$(CORE_COMPILER) --src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
	--test-wizard=2 --run-tests true --test-phases 1 --test-filter "#669:test_intent_match" --test-args "{$(INTENT_ITERATIONS)}"

# Profile with perf to identify blocking spots
# After running, analyze with: perf report -g or perf report --stdio
bench-write-stress-perf: check-sources
	$(CARGO) build --release -p moorc
	perf record -g --call-graph dwarf -o perf.data -- \
		$(abspath $(CARGO_TARGET_DIR))/release/moorc $(OPTIONS) \
		--src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
		--test-wizard=2 --run-tests true --test-phases 1 --test-filter "#668:test_write_stress" --test-timeout 60
	@echo "Perf data written to perf.data"
	@echo "Analyze with: perf report -g"

bench-combat-stress-perf: check-sources
	$(CARGO) build --release -p moorc
	perf record -e cycles:u -F 999 -g --call-graph dwarf -o perf-combat.data -- \
		$(abspath $(CARGO_TARGET_DIR))/release/moorc $(OPTIONS) \
		--src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
		--test-wizard=2 --run-tests true --test-phases 1 --test-filter "#668:test_combat_stress" --test-timeout $$(($(DURATION) + 60)) --test-args "{$(DURATION), $(SUBSCRIBERS), $(WORK_PER_TICK), $(FANOUT), $(CHECKS_PER_TICK), $(STATE_WRITES), $(DELAY_RATIO), $(UPDATE_HZ)}"
	@echo "Perf data written to perf-combat.data"
	@echo "Analyze with: perf report -i perf-combat.data --stdio --sort comm,dso,symbol --no-children"

bench-combat-stress-perf-stat: check-sources
	$(CARGO) build --release -p moorc
	perf stat -x, -e $(PERF_STAT_EVENTS) -o $(PERF_STAT_OUTPUT) -- \
		$(abspath $(CARGO_TARGET_DIR))/release/moorc $(OPTIONS) \
		--src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
		--test-wizard=2 --run-tests true --test-phases 1 --test-filter "#668:test_combat_stress" --test-timeout $$(($(DURATION) + 60)) --test-args "{$(DURATION), $(SUBSCRIBERS), $(WORK_PER_TICK), $(FANOUT), $(CHECKS_PER_TICK), $(STATE_WRITES), $(DELAY_RATIO), $(UPDATE_HZ)}"
	@echo "Perf stat written to $(PERF_STAT_OUTPUT)"
	@echo "View with: cat $(PERF_STAT_OUTPUT)"

# Profile with scheduler tracing to see blocking/sleeping (requires root or perf_event_paranoid=-1)
# Build first without sudo, then run perf with sudo
# Also captures iostat for I/O analysis
bench-write-stress-perf-sched: check-sources
	$(CARGO) build --release -p moorc
	@echo "Running perf with sudo (will prompt for password)..."
	@echo "Starting iostat logging to iostat.log..."
	@rm -f iostat.log && \
	iostat -x 1 > iostat.log & \
	IOSTAT_PID=$$! && \
	sudo perf record -g --call-graph dwarf -e sched:sched_switch,sched:sched_stat_sleep -o perf-sched.data -- \
		$(abspath $(CARGO_TARGET_DIR))/release/moorc $(OPTIONS) \
		--src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
		--test-wizard=2 --run-tests true --test-phases 1 --test-filter "#668:test_write_stress" --test-timeout 60 ; \
	kill $$IOSTAT_PID 2>/dev/null || true
	sudo chown $(USER):$(USER) perf-sched.data
	@echo "Perf data written to perf-sched.data"
	@echo "I/O stats written to iostat.log"
	@echo "Analyze with: perf report -i perf-sched.data -g"

# Run benchmark on simulated slow storage (requires setup-slow-disk.sh first)
# Usage: make bench-write-stress-slow SLOW_DB_PATH=/tmp/moor-slow-mount/bench.db DURATION=300
bench-write-stress-slow: check-sources
	@if [ ! -d "$$(dirname $(SLOW_DB_PATH))" ]; then \
		echo "Error: Slow disk not mounted. Run: ../../scripts/setup-slow-disk.sh setup"; \
		exit 1; \
	fi
	@echo "Starting iostat logging to iostat-slow.log..."
	@rm -f iostat-slow.log && \
	iostat -x 1 > iostat-slow.log & \
	IOSTAT_PID=$$! && \
	$(CORE_COMPILER) --src-objdef-dir $(SRC_DIRECTORY) --include-baselines=false $(EXPORT_ARGS) \
	--db-path $(SLOW_DB_PATH) \
	--test-wizard=2 --run-tests true --test-phases 1 --test-filter "#668:test_write_stress" --test-timeout $$(($(DURATION) + 60)) --test-args "{$(DURATION), $(SUBSCRIBERS), $(WORK_PER_TICK), $(APPEND_MODE)}" ; \
	kill $$IOSTAT_PID 2>/dev/null || true
	@echo "I/O stats written to iostat-slow.log"
