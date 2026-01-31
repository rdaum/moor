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

# Script to fetch the Meadow web client.
# This allows the web frontend to be developed independently while still
# supporting local development builds via docker-compose.

set -e

# Path to the meadow directory relative to this script
MEADOW_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/meadow"
MEADOW_REPO="timbran/meadow"
CODEBERG_HOST="codeberg.org"

# Determine the protocol based on the current repository's 'origin' remote
# This helps use the same credentials/access method as the main repo.
CURRENT_REMOTE=$(git remote get-url origin 2>/dev/null || echo "https")

if [[ "$CURRENT_REMOTE" == git@* ]] || [[ "$CURRENT_REMOTE" == ssh://* ]]; then
    REPO_URL="git@${CODEBERG_HOST}:${MEADOW_REPO}.git"
else
    REPO_URL="https://${CODEBERG_HOST}/${MEADOW_REPO}.git"
fi

if [ ! -d "$MEADOW_DIR" ]; then
    echo "Cloning meadow from $REPO_URL..."
    git clone "$REPO_URL" "$MEADOW_DIR"
else
    echo "Meadow directory already exists at $MEADOW_DIR"
    if [ -d "$MEADOW_DIR/.git" ]; then
        echo "Attempting to update meadow..."
        (cd "$MEADOW_DIR" && git pull)
    else
        echo "Warning: $MEADOW_DIR exists but is not a git repository."
    fi
fi
