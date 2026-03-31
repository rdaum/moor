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

# Script to fetch the Meadow web client.
# This allows the web frontend to be developed independently while still
# supporting local development builds via docker-compose.

set -e

# Path to the meadow directory relative to this script
MEADOW_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/meadow"
MEADOW_REPO="timbran/meadow"
CODEBERG_HOST="codeberg.org"
DEFAULT_BRANCH="v1.0-release"

# Determine the protocol based on the current repository's 'origin' remote
# This helps use the same credentials/access method as the main repo.
CURRENT_REMOTE=$(git remote get-url origin 2>/dev/null || echo "https")

if [[ "$CURRENT_REMOTE" == git@* ]] || [[ "$CURRENT_REMOTE" == ssh://* ]]; then
    REPO_URL="git@${CODEBERG_HOST}:${MEADOW_REPO}.git"
else
    REPO_URL="https://${CODEBERG_HOST}/${MEADOW_REPO}.git"
fi

CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "$DEFAULT_BRANCH")"
TARGET_BRANCH="$CURRENT_BRANCH"

remote_has_branch() {
    local repo_url="$1"
    local branch="$2"
    git ls-remote --exit-code --heads "$repo_url" "$branch" >/dev/null 2>&1
}

resolve_target_branch() {
    local repo_url="$1"
    local requested_branch="$2"

    if remote_has_branch "$repo_url" "$requested_branch"; then
        echo "$requested_branch"
        return
    fi

    echo "Warning: meadow does not have branch '$requested_branch'; using '$DEFAULT_BRANCH' instead." >&2
    echo "$DEFAULT_BRANCH"
}

TARGET_BRANCH="$(resolve_target_branch "$REPO_URL" "$TARGET_BRANCH")"

if [ ! -d "$MEADOW_DIR" ]; then
    echo "Cloning meadow from $REPO_URL on branch $TARGET_BRANCH..."
    git clone --branch "$TARGET_BRANCH" "$REPO_URL" "$MEADOW_DIR"
else
    echo "Meadow directory already exists at $MEADOW_DIR"
    if [ -d "$MEADOW_DIR/.git" ]; then
        echo "Attempting to update meadow to branch $TARGET_BRANCH..."
        (
            cd "$MEADOW_DIR"
            git fetch origin "$TARGET_BRANCH"
            if git show-ref --verify --quiet "refs/heads/$TARGET_BRANCH"; then
                git checkout "$TARGET_BRANCH"
            else
                git checkout -b "$TARGET_BRANCH" --track "origin/$TARGET_BRANCH"
            fi
            git pull --ff-only origin "$TARGET_BRANCH"
        )
    else
        echo "Warning: $MEADOW_DIR exists but is not a git repository."
    fi
fi
