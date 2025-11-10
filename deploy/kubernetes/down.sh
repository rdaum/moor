#!/bin/bash
# Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# General Public License as published by the Free Software Foundation, version
# 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.
#

# Tear down mooR kind cluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test-helpers.sh"

CLUSTER_NAME="moor"

log_info "Tearing down mooR Kubernetes cluster"

if ! kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    log_info "Cluster '$CLUSTER_NAME' does not exist"
    exit 0
fi

log_info "Deleting cluster '$CLUSTER_NAME'..."
kind delete cluster --name "$CLUSTER_NAME"

log_info "✓ Cluster deleted"
