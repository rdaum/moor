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

#
# restore-from-export.sh - Restore moor database from export snapshot
#
# This script restores a moor database from the most recent export in the
# ./export directory. It will:
#   1. Stop the moor-daemon service (hosts remain running)
#   2. Backup the current database to moor-data-backup-<timestamp>
#   3. Delete the current database
#   4. Import from the most recent export snapshot using moorc
#   5. Leave daemon stopped for you to restart manually
#
# Usage:
#   ./restore-from-export.sh
#
# The script will prompt for confirmation before proceeding.
#
# Requirements:
#   - docker compose must be available
#   - Export snapshots must exist in ./export directory
#   - Must be run from the directory containing docker-compose.yml
#

set -euo pipefail

# Configuration - works from current directory
EXPORT_DIR="./export"
MOOR_DATA_DIR="./moor-data"
COMPOSE_FILE="docker-compose.yml"
DAEMON_SERVICE="moor-daemon"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if export directory exists
if [ ! -d "$EXPORT_DIR" ]; then
    log_error "Export directory not found: $EXPORT_DIR"
    exit 1
fi

# Find the most recent export by sorting directory names numerically
log_info "Finding most recent export..."
LATEST_EXPORT=$(find "$EXPORT_DIR" -maxdepth 1 -type d -name "textdump-*.moo" | \
    sed 's/.*textdump-\([0-9]*\)\.moo/\1 &/' | \
    sort -rn | \
    head -1 | \
    cut -d' ' -f2-)

if [ -z "$LATEST_EXPORT" ]; then
    log_error "No export directories found in $EXPORT_DIR"
    exit 1
fi

LATEST_EXPORT_NAME=$(basename "$LATEST_EXPORT")
EXPORT_TIMESTAMP=$(echo "$LATEST_EXPORT_NAME" | sed 's/textdump-\([0-9]*\)\.moo/\1/')
EXPORT_DATE=$(date -d "@$EXPORT_TIMESTAMP" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date -r "$EXPORT_TIMESTAMP" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "timestamp: $EXPORT_TIMESTAMP")

log_info "Most recent export: $LATEST_EXPORT_NAME"
log_info "Export date: $EXPORT_DATE"

# Confirm with user
echo ""
echo -e "${YELLOW}WARNING: This will replace the current database with the export from:${NC}"
echo -e "  ${GREEN}$LATEST_EXPORT_NAME${NC} ($EXPORT_DATE)"
echo ""
read -p "Are you sure you want to continue? (yes/no): " -r
echo
if [[ ! $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
    log_info "Restoration cancelled."
    exit 0
fi

# Stop only the daemon service (hosts can keep running)
log_info "Stopping moor-daemon service..."
docker compose -f "$COMPOSE_FILE" stop "$DAEMON_SERVICE"

# Backup current database if it exists
if [ -d "$MOOR_DATA_DIR" ]; then
    BACKUP_TIMESTAMP=$(date +%s)
    BACKUP_DIR="moor-data-backup-$BACKUP_TIMESTAMP"
    log_info "Backing up current database to $BACKUP_DIR..."
    cp -r "$MOOR_DATA_DIR" "$BACKUP_DIR"
    log_info "Backup completed: $BACKUP_DIR"

    # Second confirmation before deletion
    echo ""
    echo -e "${YELLOW}Backup saved at: ${GREEN}$(pwd)/$BACKUP_DIR${NC}"
    echo -e "${YELLOW}Please verify the backup exists before proceeding.${NC}"
    echo ""
    read -p "Ready to delete current database and proceed with restore? (yes/no): " -r
    echo
    if [[ ! $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
        log_info "Restoration cancelled. Your database and backup are unchanged."
        log_info "Restarting daemon..."
        docker compose -f "$COMPOSE_FILE" up -d "$DAEMON_SERVICE"
        exit 0
    fi

    # Save CURVE keys before deleting database
    CURVE_KEY="$MOOR_DATA_DIR/daemon-curve.key"
    CURVE_PUB="$MOOR_DATA_DIR/daemon-curve.pub"
    TEMP_CURVE_KEY=""
    TEMP_CURVE_PUB=""

    if [ -f "$CURVE_KEY" ] && [ -f "$CURVE_PUB" ]; then
        log_info "Preserving CURVE encryption keys..."
        TEMP_CURVE_KEY=$(mktemp)
        TEMP_CURVE_PUB=$(mktemp)
        cp "$CURVE_KEY" "$TEMP_CURVE_KEY"
        cp "$CURVE_PUB" "$TEMP_CURVE_PUB"
    else
        log_warn "No CURVE keys found (new keys will be generated)"
    fi

    # Remove current database
    log_info "Removing current database..."
    rm -rf "$MOOR_DATA_DIR"
else
    log_warn "No existing database found at $MOOR_DATA_DIR"
fi

# Import from the most recent export using moorc
log_info "Importing from $LATEST_EXPORT_NAME using moorc..."
log_info "This may take several minutes depending on database size..."

# Run moorc to import from the export directory and create the database
log_info "Running moorc to import objdef..."

docker compose -f "$COMPOSE_FILE" run --rm \
    "$DAEMON_SERVICE" \
    /moor/moorc \
        --src-objdef-dir="/db/export/${LATEST_EXPORT_NAME}" \
        --db-path="/db/moor-data/development.db" \
        --debug

# Restore CURVE keys if they were saved
if [ -n "$TEMP_CURVE_KEY" ] && [ -f "$TEMP_CURVE_KEY" ]; then
    log_info "Restoring CURVE encryption keys..."
    cp "$TEMP_CURVE_KEY" "$MOOR_DATA_DIR/daemon-curve.key"
    cp "$TEMP_CURVE_PUB" "$MOOR_DATA_DIR/daemon-curve.pub"
    rm -f "$TEMP_CURVE_KEY" "$TEMP_CURVE_PUB"
fi

if [ $? -ne 0 ]; then
    log_error "Import failed! Check the logs above for details."
    if [ -d "$BACKUP_DIR" ]; then
        log_warn "Your backup is preserved at: $BACKUP_DIR"
    fi
    exit 1
fi

log_info "Import completed successfully"

# Verify the database was created
if [ ! -d "$MOOR_DATA_DIR" ]; then
    log_error "Database directory was not created. Import may have failed."
    if [ -d "$BACKUP_DIR" ]; then
        log_warn "Your backup is preserved at: $BACKUP_DIR"
    fi
    exit 1
fi

log_info "Database restored successfully from $LATEST_EXPORT_NAME"
log_info ""
log_info "To start the moor-daemon service, run:"
log_info "  docker compose -f $COMPOSE_FILE up -d $DAEMON_SERVICE"
