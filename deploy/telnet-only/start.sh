#!/bin/bash
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

# Startup script for telnet-only deployment
# Handles user permissions and directory creation before starting containers

set -e

cd "$(dirname "$0")"

# Set user/group IDs for container processes
export USER_ID=$(id -u)
export GROUP_ID=$(id -g)

# Detect host architecture for pulling the correct image
case "$(uname -m)" in
    aarch64|arm64) export ARCH=aarch64 ;;
    *)             export ARCH=x86_64  ;;
esac

# Pre-create directories with correct ownership to prevent Docker from creating them as root.
# If a previous run left root-owned dirs (e.g. from running docker compose without start.sh),
# warn and exit so the user can fix ownership.
DIRS="moor-data moor-ipc moor-config moor-local-share moor-telnet-host-data"
for dir in $DIRS; do
    if [ -d "$dir" ] && [ "$(stat -c '%u' "$dir")" != "$USER_ID" ]; then
        echo "ERROR: $dir is not owned by you (uid $USER_ID). This usually happens when"
        echo "       docker compose was run directly instead of via start.sh."
        echo "       Fix with: sudo chown -R $USER_ID:$GROUP_ID $dir"
        exit 1
    fi
done
mkdir -p $DIRS

# Start containers
docker compose up -d

echo ""
echo "Services started. Useful commands:"
echo "  View logs:        docker compose logs -f"
echo "  Stop services:    docker compose stop"
echo "  Restart services: docker compose restart"
