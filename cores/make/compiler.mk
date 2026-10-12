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

# Compiler selection is shared by every core. OPTIONS stays core-specific.
CORE_MAKE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
MOOR_ROOT := $(abspath $(CORE_MAKE_DIR)/../..)
CARGO ?= cargo
PYTHON ?= python3
export CARGO_TARGET_DIR ?= $(MOOR_ROOT)/target
MOORC_TYPE ?= cargo
MOORC_PROFILE ?= dev
MOORC_IMAGE ?= moor-moor-daemon
DEBUG ?= 0
SRC_DIRECTORY ?= src
OUTPUT_DIRECTORY ?= .
MOORC_BIN_DIR = $(abspath $(CARGO_TARGET_DIR))/$(if $(filter dev,$(MOORC_PROFILE)),debug,$(MOORC_PROFILE))

ifeq ($(MOORC_TYPE),cargo)
MOORC ?= $(CARGO) run --profile $(MOORC_PROFILE) -p moorc --
else ifeq ($(MOORC_TYPE),direct)
MOORC ?= $(MOORC_BIN_DIR)/moorc
else ifeq ($(MOORC_TYPE),docker)
# Relative source and fixture paths work unchanged inside the mounted core.
MOORC ?= docker run --rm -v "$(CURDIR):/work" -w /work $(MOORC_IMAGE) /moor/moorc
else
$(error Unknown MOORC_TYPE '$(MOORC_TYPE)'; use cargo, direct, or docker)
endif

ifeq ($(DEBUG),1)
ifeq ($(MOORC_TYPE),docker)
$(error DEBUG=1 requires MOORC_TYPE=cargo or direct)
endif
CORE_COMPILER = gdb --batch --ex "handle SIGUSR1 nostop noprint pass" --ex run --ex bt --ex quit --args $(MOORC) $(OPTIONS)
else
CORE_COMPILER = $(MOORC) $(OPTIONS)
endif
