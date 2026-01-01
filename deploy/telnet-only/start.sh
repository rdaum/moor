#!/bin/bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

# Startup script for telnet-only deployment
# Handles user permissions and directory creation before starting containers

set -e

# Set user/group IDs for container processes
export USER_ID=$(id -u)
export GROUP_ID=$(id -g)

# Pre-create directories with correct ownership to prevent Docker from creating them as root
mkdir -p moor-data moor-ipc moor-config moor-local-share moor-telnet-host-data

# Start containers
docker compose up -d

echo ""
echo "Services started. Useful commands:"
echo "  View logs:        docker compose logs -f"
echo "  Stop services:    docker compose stop"
echo "  Restart services: docker compose restart"
