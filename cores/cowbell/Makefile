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

HEADLESS_FILTERS ?= 90104 90000 90001 90002 90003 90004 90005 90006 90007 90010 90011 90012 90013 90014 90015 90016 90017 90018
HEADLESS_PLAYER_FILTERS ?= 90008:test_headless_builder_commands_use_stored_grants 90008:test_headless_builder_commands_missing_grants_deny_cleanly 90008:test_headless_builder_message_commands_dispatch_as_player 90014:test_inspection_transfer_reachability 90014:test_inspection_transfers_recheck_after_policy_yield 90014:test_exit_command_annotations 90014:test_travel_rechecks_after_pre_exit_yield 90014:test_contextual_suggestions
HEADLESS_WIZARD_FILTERS ?= 90008:test_headless_builder_commands_wizard_builds_without_grants
HEADLESS_TASK_PLAYER ?= 90102
HEADLESS_TIMEOUT ?= 10
TEST_SRC_DIRECTORY ?= .runtime-test-src
HEADLESS_SRC_DIRECTORY ?= .runtime-headless-src
HEADLESS_SRC_STAMP = $(HEADLESS_SRC_DIRECTORY)/.prepared
TEST_WIZARD ?= 90100
TEST_PROGRAMMER ?= 90101
TEST_PLAYER ?= 90102
STYLE_CONVERTED_UNITS = $(shell cat tests/style/converted.txt)
TEST_FILTER ?=
HASH := \#
HEADLESS_FILTER_IDS = $(patsubst $(HASH)%,%,$(HEADLESS_FILTERS))
HEADLESS_WIZARD_FILTER_IDS = $(patsubst $(HASH)%,%,$(HEADLESS_WIZARD_FILTERS))
HEADLESS_PLAYER_FILTER_IDS = $(patsubst $(HASH)%,%,$(HEADLESS_PLAYER_FILTERS))

OPTIONS = --use-boolean-returns true \
          --use-symbols-in-builtins true \
          --custom-errors true \
          --use-uuobjids true \
          --anonymous-objects true

TEST_MODE = methods
TEST_TIMEOUT = $(HEADLESS_TIMEOUT)
TEST_PREREQUISITES = runtime-test-src
EXPORT_FORBIDDEN_IDS = = \#(900[0-9]+|901[0-9]+);
CHECK_TARGETS = check-style test test-headless test-session test-harness roundtrip test-wire test-git-worker test-build-inventory
CLEAN_EXTRA = $(HEADLESS_SRC_DIRECTORY) $(TEST_SRC_DIRECTORY) gen.empty.log gen.exception.log gen.event-control.log gen.style.log gen.style-strict.log
CORE_HELP = 'make test-headless      Run headless methods (alias: runtime-headless).' \
            'make test-session       Run session scenarios (alias: runtime-session).' \
            'make test-wire          Run TCP and restart tests.' \
            'make test-git-worker    Run Git worker integration tests.'
include ../make/core.mk

TEST_BIN_DIR = $(MOORC_BIN_DIR)
SESSION_RUNNER = $(TEST_BIN_DIR)/core-session-runner
EXPECT_FAILURE = $(PYTHON) $(CORE_MAKE_DIR)expect_failure.py
TEST_TOOL_PACKAGES = moorc core-style-audit core-session-runner moor-server moor-daemon moor-telnet-host moor-git-worker

.PHONY: build-test-tools check-style runtime-test-src runtime-headless-src runtime-headless runtime-session test-headless test-session test-harness test-build-inventory test-wire test-git-worker
# Keep one dependency feature set for every binary used by these checks.
build-test-tools:
	$(CARGO) build --locked --manifest-path $(MOOR_ROOT)/Cargo.toml --profile $(MOORC_PROFILE) $(foreach package,$(TEST_TOOL_PACKAGES),-p $(package)) --bins

RUNTIME_CHECK_TARGETS = $(filter-out roundtrip test-headless test-session,$(CHECK_TARGETS)) runtime-headless runtime-session
$(RUNTIME_CHECK_TARGETS): build-test-tools
ifeq ($(MOORC_TYPE),cargo)
check $(RUNTIME_CHECK_TARGETS): MOORC = $(MOORC_BIN_DIR)/moorc
endif

test-headless: runtime-headless
test-session: runtime-session

check-style:
	@$(TEST_BIN_DIR)/core-style-audit src --baseline tests/style/baseline.tsv > gen.style.log 2>&1 || { cat gen.style.log; exit 1; }
	@echo "PASS style baseline (details: gen.style.log)"
	@$(TEST_BIN_DIR)/core-style-audit src --strict $(foreach unit,$(STYLE_CONVERTED_UNITS),--check $(unit)) > gen.style-strict.log 2>&1 || { cat gen.style-strict.log; exit 1; }
	@echo "PASS converted source style (details: gen.style-strict.log)"
	bash tests/style/check-audit.sh $(TEST_BIN_DIR)/core-style-audit

runtime-test-src: FORCE
	rm -rf $(TEST_SRC_DIRECTORY)
	mkdir -p $(TEST_SRC_DIRECTORY)
	rsync -a src/ $(TEST_SRC_DIRECTORY)/
	rsync -a tests/baseline/fixtures/ $(TEST_SRC_DIRECTORY)/
	cat tests/baseline/fixtures/baseline_constants.moo >> $(TEST_SRC_DIRECTORY)/constants.moo
	rm -f $(TEST_SRC_DIRECTORY)/baseline_constants.moo

$(HEADLESS_SRC_STAMP): runtime-test-src
	rm -rf $(HEADLESS_SRC_DIRECTORY)
	cp -r $(TEST_SRC_DIRECTORY) $(HEADLESS_SRC_DIRECTORY)
	rsync -a tests/headless/ $(HEADLESS_SRC_DIRECTORY)/
	cat tests/headless/headless_constants.moo >> $(HEADLESS_SRC_DIRECTORY)/constants.moo
	rm -f $(HEADLESS_SRC_DIRECTORY)/headless_constants.moo
	touch $(HEADLESS_SRC_STAMP)

runtime-headless-src: $(HEADLESS_SRC_STAMP)

runtime-headless: runtime-headless-src
	@test -n "$(strip $(HEADLESS_FILTER_IDS) $(HEADLESS_PLAYER_FILTER_IDS) $(HEADLESS_WIZARD_FILTER_IDS))" || { echo "FAIL: no headless filters selected" >&2; exit 1; }
	set -e; for filter in $(HEADLESS_FILTER_IDS); do \
		$(CHECKED_METHODS) $(CORE_COMPILER) --src-objdef-dir $(HEADLESS_SRC_DIRECTORY) \
		--test-wizard=$(TEST_WIZARD) --test-programmer=$(TEST_PROGRAMMER) --test-player=$(TEST_PLAYER) --run-tests true \
		--test-filter "#$$filter" --test-timeout $(HEADLESS_TIMEOUT); \
	done
	set -e; for filter in $(HEADLESS_PLAYER_FILTER_IDS); do \
		$(CHECKED_METHODS) $(CORE_COMPILER) --src-objdef-dir $(HEADLESS_SRC_DIRECTORY) \
		--test-wizard=$(TEST_WIZARD) --test-programmer=$(TEST_PROGRAMMER) --test-player=$(TEST_PLAYER) --test-task-player=$(HEADLESS_TASK_PLAYER) --run-tests true \
		--test-filter "#$$filter" --test-timeout $(HEADLESS_TIMEOUT); \
	done

	set -e; for filter in $(HEADLESS_WIZARD_FILTER_IDS); do \
		$(CHECKED_METHODS) $(CORE_COMPILER) --src-objdef-dir $(HEADLESS_SRC_DIRECTORY) \
		--test-wizard=$(TEST_WIZARD) --test-programmer=$(TEST_PROGRAMMER) --test-player=$(TEST_PLAYER) --test-task-player=$(TEST_WIZARD) --run-tests true \
		--test-filter "#$$filter" --test-timeout $(HEADLESS_TIMEOUT); \
	done

runtime-session: runtime-headless-src
	$(SESSION_RUNNER) --cowbell \
		--core-dir $(HEADLESS_SRC_DIRECTORY) --moot tests/baseline/sessions \
		--wizard $(TEST_WIZARD) --programmer $(TEST_PROGRAMMER) --nonprogrammer $(TEST_PLAYER)

test-harness: runtime-headless-src
	@$(EXPECT_FAILURE) --name empty-method-filter --match 'FAIL: filter selected no tests' -- \
		$(CHECKED_METHODS) $(CORE_COMPILER) --src-objdef-dir $(HEADLESS_SRC_DIRECTORY) --run-tests true \
		--test-wizard=$(TEST_WIZARD) --test-filter '#999999'

	@$(EXPECT_FAILURE) --name command-exception --match 'caught command exception for connection' -- \
		$(SESSION_RUNNER) --cowbell \
		--core-dir $(HEADLESS_SRC_DIRECTORY) --moot tests/baseline/negative/command_exception.moot \
		--wizard $(TEST_WIZARD) --programmer $(TEST_PROGRAMMER) --nonprogrammer $(TEST_PLAYER)

	@set -e; for scenario in wrong_type wrong_audience wrong_recipient; do \
		$(EXPECT_FAILURE) --name $$scenario --match 'event contract failed|no committed event for recipient' -- \
			$(SESSION_RUNNER) --cowbell \
			--core-dir $(HEADLESS_SRC_DIRECTORY) --moot tests/baseline/negative/$$scenario.moot \
			--wizard $(TEST_WIZARD) --programmer $(TEST_PROGRAMMER) --nonprogrammer $(TEST_PLAYER); \
	done

	@$(EXPECT_FAILURE) --name empty-scenario --match 'scenario contains no command or eval steps' -- \
		$(SESSION_RUNNER) --cowbell \
		--core-dir $(HEADLESS_SRC_DIRECTORY) --moot tests/baseline/negative/empty.moot \
		--wizard $(TEST_WIZARD) --programmer $(TEST_PROGRAMMER) --nonprogrammer $(TEST_PLAYER)

test-build-inventory:
	python3 tests/baseline/check_inventory.py $(MOORC)

# Verify actual telnet connections and persistent native schedules after restart.
test-wire:
	python3 -m unittest discover -s tests -p test_wire.py
	python3 tests/wire.py --server $(TEST_BIN_DIR)/moor --core-dir src

# Exercise the Cowbell wrapper through the standalone worker and a local Git HTTP fixture.
test-git-worker:
	python3 tests/git_worker.py --bin-dir $(TEST_BIN_DIR) --core-dir src
