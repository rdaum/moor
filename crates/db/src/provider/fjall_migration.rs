// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Fjall-specific database migration utilities for format changes
//!
//! # Semantic Versioning for Database Formats
//!
//! This module uses semantic versioning (semver) for database format versions to clearly
//! communicate compatibility and migration requirements:
//!
//! ## Version Components
//!
//! - **MAJOR version** (X.0.0): Breaking changes that require data migration
//!   - Example: Changing timestamp format from u128 to u64 (v1.0.0 → v2.0.0)
//!   - Requires running migration code to convert existing data
//!   - Old code cannot read new format, new code cannot read old format
//!
//! - **MINOR version** (1.X.0): Backward-compatible additions to the schema
//!   - Example: Adding a new optional table or column
//!   - New code can read old databases (missing tables are treated as empty)
//!   - Old code might miss new features but can still function
//!   - No migration needed, but version marker should be updated
//!
//! - **PATCH version** (1.0.X): Bug fixes that don't change the format
//!   - Example: Fixing corrupted data, optimizing indexes
//!   - Fully backward and forward compatible
//!   - No migration needed
//!
//! ## Migration Strategy (1.0-beta)
//!
//! For the 1.0-beta release, we support migration from:
//! - **3.0.0** (pre-beta format) → **release-1.0.0** (stable 1.0 format)
//!
//! This is a simple version marker update with no data format changes.
//! Older pre-beta formats (1.0.0, 2.0.0) are no longer supported for migration.
//!
//! ## Supported Database Versions
//!
//! - **3.0.0**: Pre-beta development format (can be migrated to release-1.0.0)
//! - **release-1.0.0**: Stable database format for mooR 1.0 (current)
//!
//! *Note* that the DB version is separate from the main mooR project version, which has its own
//! sem-versioning.

use crate::provider::Migrator;
use fjall::{Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use semver::Version;
use std::{fs, path::Path};
use tracing::{error, info, warn};

/// Current database format version using semver with release prefix
/// Note: Database versions are independent of mooR release versions to avoid confusion
///
/// Supported database versions:
/// - 3.0.0 = Pre-beta development format (will be migrated to release-1.0.0)
/// - release-1.0.0 = Stable database format for mooR 1.0 (current)
const CURRENT_DB_VERSION: &str = "release-1.0.0";

/// Database version marker key in sequences partition
const VERSION_KEY: &[u8] = b"__db_version__";

/// Check if a database at the given path needs migration, and perform it if needed.
/// This is called BEFORE opening the database.
///
/// Strategy:
/// 1. Check version marker by opening DB read-only
/// 2. If migration needed:
///    - Copy entire DB directory to `<path>.migrating`
///    - Open the copy and run migration on it
///    - Close the migrated copy
///    - Atomically swap: `<path>` → `<path>.old`, `<path>.migrating` → `<path>`
///    - Delete `<path>.old` after successful swap
/// 3. Return Ok(()) - caller can now safely open the DB
pub fn fjall_check_and_migrate(db_path: &Path) -> Result<(), String> {
    // If database doesn't exist, no migration needed
    if !db_path.exists() {
        return Ok(());
    }

    // Open database read-only to check version
    let keyspace = Config::new(db_path)
        .open()
        .map_err(|e| format!("Failed to open database to check version: {e}"))?;

    let sequences_partition = keyspace
        .open_partition("sequences", PartitionCreateOptions::default())
        .map_err(|e| format!("Failed to open sequences partition: {e}"))?;

    // Check version (read the full version string from database)
    let current_version_str = sequences_partition
        .get(VERSION_KEY)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok());

    // Drop handles before any file operations
    drop(sequences_partition);
    drop(keyspace);

    // If database already has the current release version, no migration needed
    if let Some(ref version_str) = current_version_str {
        info!("Database version marker: {version_str:?}, current: {CURRENT_DB_VERSION:?}");
        if version_str == CURRENT_DB_VERSION {
            info!(
                "Database at {db_path:?} is already at {CURRENT_DB_VERSION} and needs no migrations..."
            );
            return Ok(());
        }
    }

    // For 1.0-beta, only support migration from 3.0.0 to release-1.0.0
    // If no version marker, assume it's 3.0.0 (pre-beta format)
    let current_version_str = current_version_str.unwrap_or_else(|| "3.0.0".to_string());

    // Validate that we're migrating from a supported version
    if current_version_str != "3.0.0" {
        return Err(format!(
            "Unsupported database version '{}': Only version 3.0.0 can be migrated to {}",
            current_version_str, CURRENT_DB_VERSION
        ));
    }

    // Parse versions for logging
    let (_current_prefix, current_version) = parse_version_string(&current_version_str)
        .unwrap_or_else(|_| (None, Version::parse("3.0.0").unwrap()));

    let (_target_prefix, target_version) = parse_version_string(CURRENT_DB_VERSION)
        .unwrap_or_else(|_| (None, Version::parse("1.0.0").unwrap()));

    warn!("Database at {db_path:?} needs migration from {current_version} to {target_version}");

    // Perform migration via copy-and-swap
    fjall_migrate_via_copy(db_path, current_version, target_version)
}

/// Perform migration by copying database, migrating the copy, and swapping
fn fjall_migrate_via_copy(
    db_path: &Path,
    from_version: Version,
    to_version: Version,
) -> Result<(), String> {
    let migrating_path = db_path.with_extension("migrating");
    let old_path = db_path.with_extension("old");

    // Clean up any leftover migration artifacts from previous attempts
    if migrating_path.exists() {
        warn!("Found leftover migration directory, cleaning up");
        fs::remove_dir_all(&migrating_path)
            .map_err(|e| format!("Failed to clean up old migration directory: {e}"))?;
    }
    if old_path.exists() {
        warn!("Found leftover old database directory, cleaning up");
        fs::remove_dir_all(&old_path)
            .map_err(|e| format!("Failed to clean up old database directory: {e}"))?;
    }

    info!("Copying database to {:?} for migration", migrating_path);

    // Copy entire database directory
    copy_dir_recursive(db_path, &migrating_path)?;

    info!("Opening copied database for migration");

    // Open the copy and perform migration
    let keyspace = Config::new(&migrating_path)
        .open()
        .map_err(|e| format!("Failed to open copied database: {e}"))?;

    let sequences_partition = keyspace
        .open_partition("sequences", PartitionCreateOptions::default())
        .map_err(|e| format!("Failed to open sequences partition in copy: {e}"))?;

    let migrator = FjallMigrator::new(keyspace, sequences_partition);

    info!("Running migration on copied database");

    // Run migration on the copy
    if let Err(e) = migrator.migrate_if_needed() {
        error!("Migration failed: {e}");
        // Clean up failed migration
        drop(migrator);
        fs::remove_dir_all(&migrating_path).ok();
        return Err(format!("Migration failed: {e}"));
    }

    // Close the migrated copy and ensure all writes are flushed to disk
    drop(migrator);

    info!("Migration successful, swapping directories");

    // Atomically swap directories:
    // 1. Rename original to .old
    // 2. Rename .migrating to original name
    fs::rename(db_path, &old_path)
        .map_err(|e| format!("Failed to rename original database: {e}"))?;

    if let Err(e) = fs::rename(&migrating_path, db_path) {
        // Try to restore original
        error!("Failed to rename migrated database: {e}");
        fs::rename(&old_path, db_path).ok();
        return Err(format!("Failed to swap migrated database: {e}"));
    }

    info!("Successfully swapped to migrated database");

    // Clean up old database
    fs::remove_dir_all(&old_path).map_err(|e| format!("Failed to remove old database: {e}"))?;

    info!("Migration complete: {} -> {}", from_version, to_version);

    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("Failed to create destination directory: {e}"))?;

    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read source directory: {e}"))? {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)
                .map_err(|e| format!("Failed to copy file {file_name:?}: {e}"))?;
        }
    }

    Ok(())
}

/// Parse a version string to extract the numeric semver part
/// For "1.0.0" returns (None, Version 1.0.0)
/// For "release-1.0.0" returns (Some("release"), Version 1.0.0)
fn parse_version_string(version_str: &str) -> Result<(Option<&str>, Version), String> {
    let (prefix, version_part) = if let Some(pos) = version_str.rfind('-') {
        let (prefix_part, version) = version_str.split_at(pos + 1);
        let prefix_without_dash = &prefix_part[..prefix_part.len() - 1];
        // If the prefix part contains only alphabetic characters, it's a release prefix
        if prefix_without_dash.chars().all(|c| c.is_alphabetic()) {
            (Some(prefix_without_dash), version)
        } else {
            (None, version_str)
        }
    } else {
        (None, version_str)
    };

    let version = Version::parse(version_part)
        .map_err(|e| format!("Invalid semver version '{version_str}': {e}"))?;

    Ok((prefix, version))
}

/// Fjall-specific migration handler
pub struct FjallMigrator {
    #[allow(dead_code)]
    keyspace: Keyspace,
    sequences_partition: PartitionHandle,
}

impl FjallMigrator {
    pub fn new(keyspace: Keyspace, sequences_partition: PartitionHandle) -> Self {
        Self {
            keyspace,
            sequences_partition,
        }
    }

    /// Get the current database version as a full string (e.g., "3.0.0" or "release-1.0.0")
    fn get_db_version_string(&self) -> Result<String, String> {
        let version_bytes = self
            .sequences_partition
            .get(VERSION_KEY)
            .map_err(|e| format!("Failed to read version: {e}"))?
            .ok_or_else(|| "No version marker found".to_string())?;

        String::from_utf8(version_bytes.to_vec())
            .map_err(|e| format!("Invalid UTF-8 in version string: {e}"))
    }

    /// Migration from version 3.0.0 to release-1.0.0
    /// No format changes required - this is purely a version marker update for the 1.0 release.
    fn fjall_migrate_v3_to_release_1_0_0(&self) -> Result<(), String> {
        info!(
            "Migrating database from 3.0.0 to release-1.0.0: Version alignment for mooR 1.0 stable release"
        );
        // No actual data migration needed, just the version marker will be updated
        // by mark_current_version() at the end of migrate_if_needed()
        Ok(())
    }
}

impl Migrator for FjallMigrator {
    fn migrate_if_needed(&self) -> Result<(), String> {
        let current_version_str = self
            .get_db_version_string()
            .unwrap_or_else(|_| "3.0.0".to_string());

        // Parse current version for comparison
        let (current_prefix, current_version) = parse_version_string(&current_version_str)
            .unwrap_or_else(|_| (None, Version::parse("3.0.0").unwrap()));

        // Parse target version
        let (_target_prefix, target_version) = parse_version_string(CURRENT_DB_VERSION)
            .map_err(|e| format!("Invalid target database version: {e}"))?;

        // If not prefixed by "release-", treat unprefixed version 3.0.0 as 0.9.0 for comparison
        let effective_current = if current_prefix.is_none() && current_version_str == "3.0.0" {
            Version::parse("0.9.0").unwrap()
        } else {
            current_version
        };

        // Check if migration is needed
        if effective_current >= target_version {
            info!("Database is already at {CURRENT_DB_VERSION}, no migration needed");
            return Ok(());
        }

        // For 1.0-beta, we only support migration from 3.0.0 (treated as 0.9.0)
        if current_version_str != "3.0.0" {
            return Err(format!(
                "Unsupported database version '{}': Only version 3.0.0 can be migrated to {}",
                current_version_str, CURRENT_DB_VERSION
            ));
        }

        warn!(
            "Fjall database version {} needs migration to {}",
            effective_current, target_version
        );

        // Migration from 3.0.0 to release-1.0.0
        self.fjall_migrate_v3_to_release_1_0_0()?;

        // Mark migration complete
        self.mark_current_version()?;

        info!(
            "Fjall database migration completed to version {}",
            target_version
        );
        Ok(())
    }

    fn mark_current_version(&self) -> Result<(), String> {
        self.sequences_partition
            .insert(VERSION_KEY, CURRENT_DB_VERSION.as_bytes())
            .map_err(|e| format!("Failed to write version marker: {e}"))?;

        info!("Marked Fjall database as version {}", CURRENT_DB_VERSION);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Migrator;
    use byteview::ByteView;
    use fjall::{Config, PartitionCreateOptions};
    use semver::Version;
    use tempfile::TempDir;

    #[test]
    fn test_fjall_migration_detection() {
        let tmpdir = TempDir::new().unwrap();
        let keyspace = Config::new(tmpdir.path()).open().unwrap();
        let sequences_partition = keyspace
            .open_partition("sequences", PartitionCreateOptions::default())
            .unwrap();

        let migrator = FjallMigrator::new(keyspace.clone(), sequences_partition.clone());

        // Fresh database should have no version marker
        assert!(migrator.get_db_version_string().is_err());

        // Mark as current version
        migrator.mark_current_version().unwrap();

        // Should now have current version (release-1.0.0)
        let version_str = migrator.get_db_version_string().unwrap();
        assert_eq!(version_str, "release-1.0.0");

        // Parse and verify it's version 1.0.0
        let (_prefix, version) = parse_version_string(&version_str).unwrap();
        assert_eq!(version, Version::parse("1.0.0").unwrap());
    }

    #[test]
    fn test_fjall_migrate_if_needed_fresh_db() {
        let tmpdir = TempDir::new().unwrap();
        let keyspace = Config::new(tmpdir.path()).open().unwrap();
        let sequences_partition = keyspace
            .open_partition("sequences", PartitionCreateOptions::default())
            .unwrap();

        // Create a test partition with old format data
        let test_partition = keyspace
            .open_partition("test", PartitionCreateOptions::default())
            .unwrap();

        // Write old format data (u128 timestamp)
        let mut old_format = Vec::new();
        old_format.extend_from_slice(&1000u128.to_le_bytes());
        old_format.extend_from_slice(b"test_value");
        test_partition
            .insert(
                ByteView::from(b"test_key".as_slice()),
                ByteView::from(old_format),
            )
            .unwrap();

        let migrator = FjallMigrator::new(keyspace, sequences_partition);

        // Run migration
        migrator.migrate_if_needed().unwrap();

        // Verify version is updated to release-1.0.0
        let version_str = migrator.get_db_version_string().unwrap();
        assert_eq!(version_str, "release-1.0.0");

        // Running migration again should be a no-op
        migrator.migrate_if_needed().unwrap();
    }

    #[test]
    fn test_fjall_semver_version_comparison() {
        // Test that semver comparisons work as expected for our use case
        let v1_0_0 = Version::parse("1.0.0").unwrap();
        let v1_1_0 = Version::parse("1.1.0").unwrap();
        let v1_0_1 = Version::parse("1.0.1").unwrap();
        let v2_0_0 = Version::parse("2.0.0").unwrap();

        // Major version changes require migration
        assert!(v1_0_0 < v2_0_0);
        assert_eq!(v1_0_0.major, 1);
        assert_eq!(v2_0_0.major, 2);

        // Minor version changes are backward compatible (no migration needed)
        assert!(v1_0_0 < v1_1_0);
        assert_eq!(v1_0_0.major, v1_1_0.major);

        // Patch version changes are bug fixes (no migration needed)
        assert!(v1_0_0 < v1_0_1);
        assert_eq!(v1_0_0.major, v1_0_1.major);
    }

    #[test]
    fn test_fjall_version_marker_persistence() {
        let tmpdir = TempDir::new().unwrap();
        let keyspace = Config::new(tmpdir.path()).open().unwrap();
        let sequences_partition = keyspace
            .open_partition("sequences", PartitionCreateOptions::default())
            .unwrap();

        let migrator = FjallMigrator::new(keyspace, sequences_partition.clone());

        // Write a version marker
        sequences_partition
            .insert(b"__db_version__", "2.0.0".as_bytes())
            .unwrap();

        // Read it back
        let version_str = migrator.get_db_version_string().unwrap();
        assert_eq!(version_str, "2.0.0");

        // Parse and verify version components
        let (_prefix, version) = parse_version_string(&version_str).unwrap();
        assert_eq!(version, Version::parse("2.0.0").unwrap());
        assert_eq!(version.major, 2);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
    }
}
