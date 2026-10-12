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

# Cores with method suites opt in and provide their principals and overlay.
TEST_MODE ?= import
TEST_SRC_DIRECTORY ?= $(SRC_DIRECTORY)
TEST_PREREQUISITES ?= check-sources
TEST_TIMEOUT ?= 30
TEST_FILTER ?=
TEST_TASK_PLAYER ?=
CHECKED_METHODS = $(PYTHON) $(CORE_MAKE_DIR)checked_methods.py
TEST_PRINCIPALS = --test-wizard $(TEST_WIZARD) --test-programmer $(TEST_PROGRAMMER) --test-player $(TEST_PLAYER)

.PHONY: test
ifeq ($(TEST_MODE),methods)
test: $(TEST_PREREQUISITES)
	$(CHECKED_METHODS) $(if $(TEST_LOG),--log "$(TEST_LOG)",) $(CORE_COMPILER) --src-objdef-dir "$(TEST_SRC_DIRECTORY)" \
		--run-tests true $(TEST_PRINCIPALS) --test-timeout $(TEST_TIMEOUT) \
		$(if $(TEST_TASK_PLAYER),--test-task-player $(TEST_TASK_PLAYER),) \
		$(if $(TEST_FILTER),--test-filter "$(TEST_FILTER)",)
else ifeq ($(TEST_MODE),import)
test: build
	@echo "Import/export validated. This core has no default method test suite."
else
$(error Unknown TEST_MODE '$(TEST_MODE)'; use import or methods)
endif
