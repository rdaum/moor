// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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
//! ## Migration Strategy
//!
//! - Migrations are triggered only on MAJOR version changes
//! - Each major version has a dedicated migration function (e.g., `fjall_migrate_v1_to_v2`)
//! - Migrations run sequentially: v1→v2→v3 if needed
//! - The database version marker is stored as a UTF-8 string in the sequences partition
//!
//! ## Version History
//!
//! - **1.0.0**: A pre-release format with u128 UUIDv7 timestamps
//! - **2.0.0**: Changed to u64 monotonic timestamps (current)
//!
//! *Note* that the DB version is separate from the main mooR project version, which has its own
//! sem-versioning.

use crate::provider::Migrator;
use byteview::ByteView;
use fjall::{Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use semver::Version;
use std::{fs, path::Path};
use tracing::{error, info, warn};

/// Current database format version using semver
/// 1.0.0 = Original format with u128 timestamps
/// 2.0.0 = Changed to u64 monotonic timestamps
const CURRENT_DB_VERSION: &str = "2.0.0";

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
        .map_err(|e| format!("Failed to open database to check version: {}", e))?;

    let sequences_partition = keyspace
        .open_partition("sequences", PartitionCreateOptions::default())
        .map_err(|e| format!("Failed to open sequences partition: {}", e))?;

    // Check version
    let current_version = get_db_version_static(&sequences_partition).unwrap_or_else(|_| {
        // No version marker = version 1.0.0
        Version::parse("1.0.0").unwrap()
    });

    let target_version = Version::parse(CURRENT_DB_VERSION).unwrap();

    // Drop handles before any file operations
    drop(sequences_partition);
    drop(keyspace);

    if current_version >= target_version {
        // No migration needed
        info!(
            "Database at {db_path:?} is up to date with {target_version} and needs no migrations..."
        );
        return Ok(());
    }

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
            .map_err(|e| format!("Failed to clean up old migration directory: {}", e))?;
    }
    if old_path.exists() {
        warn!("Found leftover old database directory, cleaning up");
        fs::remove_dir_all(&old_path)
            .map_err(|e| format!("Failed to clean up old database directory: {}", e))?;
    }

    info!("Copying database to {:?} for migration", migrating_path);

    // Copy entire database directory
    copy_dir_recursive(db_path, &migrating_path)?;

    info!("Opening copied database for migration");

    // Open the copy and perform migration
    let keyspace = Config::new(&migrating_path)
        .open()
        .map_err(|e| format!("Failed to open copied database: {}", e))?;

    let sequences_partition = keyspace
        .open_partition("sequences", PartitionCreateOptions::default())
        .map_err(|e| format!("Failed to open sequences partition in copy: {}", e))?;

    let migrator = FjallMigrator::new(keyspace, sequences_partition);

    info!("Running migration on copied database");

    // Run migration on the copy
    if let Err(e) = migrator.migrate_if_needed() {
        error!("Migration failed: {}", e);
        // Clean up failed migration
        drop(migrator);
        fs::remove_dir_all(&migrating_path).ok();
        return Err(format!("Migration failed: {}", e));
    }

    // Close the migrated copy
    drop(migrator);

    info!("Migration successful, swapping directories");

    // Atomically swap directories:
    // 1. Rename original to .old
    // 2. Rename .migrating to original name
    fs::rename(db_path, &old_path)
        .map_err(|e| format!("Failed to rename original database: {}", e))?;

    if let Err(e) = fs::rename(&migrating_path, db_path) {
        // Try to restore original
        error!("Failed to rename migrated database: {}", e);
        fs::rename(&old_path, db_path).ok();
        return Err(format!("Failed to swap migrated database: {}", e));
    }

    info!("Successfully swapped to migrated database");

    // Clean up old database
    fs::remove_dir_all(&old_path).map_err(|e| format!("Failed to remove old database: {}", e))?;

    info!("Migration complete: {} -> {}", from_version, to_version);

    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create destination directory: {}", e))?;

    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read source directory: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)
                .map_err(|e| format!("Failed to copy file {:?}: {}", file_name, e))?;
        }
    }

    Ok(())
}

/// Get database version from a sequences partition (static version for pre-open checks)
fn get_db_version_static(sequences_partition: &PartitionHandle) -> Result<Version, String> {
    let version_bytes = sequences_partition
        .get(VERSION_KEY)
        .map_err(|e| format!("Failed to read version: {}", e))?
        .ok_or_else(|| "No version marker found".to_string())?;

    let version_str = String::from_utf8(version_bytes.to_vec())
        .map_err(|e| format!("Invalid UTF-8 in version string: {}", e))?;

    Version::parse(&version_str)
        .map_err(|e| format!("Invalid semver version '{}': {}", version_str, e))
}

/// Fjall-specific migration handler
pub struct FjallMigrator {
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

    /// Get the current database version as a semver Version
    fn get_db_version(&self) -> Result<Version, String> {
        let version_bytes = self
            .sequences_partition
            .get(VERSION_KEY)
            .map_err(|e| format!("Failed to read version: {}", e))?
            .ok_or_else(|| "No version marker found".to_string())?;

        let version_str = String::from_utf8(version_bytes.to_vec())
            .map_err(|e| format!("Invalid UTF-8 in version string: {}", e))?;

        Version::parse(&version_str)
            .map_err(|e| format!("Invalid semver version '{}': {}", version_str, e))
    }

    /// Migration from version 1 (u128 timestamps) to version 2 (u64 timestamps)
    fn fjall_migrate_v1_to_v2(&self) -> Result<(), String> {
        warn!("Running Fjall migration v1 -> v2: Converting u128 timestamps to u64");

        let partition_names = vec![
            "object_location",
            "object_parent",
            "object_flags",
            "object_owner",
            "object_name",
            "object_verbdefs",
            "object_verbs",
            "object_propdefs",
            "object_propvalues",
            "object_propflags",
            "object_last_move",
            "anonymous_object_metadata",
        ];

        for name in partition_names {
            if self.keyspace.partition_exists(name) {
                let partition = self
                    .keyspace
                    .open_partition(name, PartitionCreateOptions::default())
                    .map_err(|e| format!("Failed to open partition {}: {}", name, e))?;
                Self::fjall_migrate_partition_u128_to_u64(&partition, name)?;
            }
        }

        // Reset the monotonic transaction counter (sequence #15) to 1
        // since we've zeroed all timestamps
        info!("Resetting monotonic transaction counter to 1");
        self.sequences_partition
            .insert(15_u64.to_le_bytes(), 1u64.to_le_bytes())
            .map_err(|e| format!("Failed to reset transaction counter: {}", e))?;

        Ok(())
    }

    /// Migrate a Fjall partition from u128 timestamps to u64 timestamps
    /// Simply zeros out all timestamps - ordering doesn't matter since we're resetting the system
    fn fjall_migrate_partition_u128_to_u64(
        partition: &PartitionHandle,
        partition_name: &str,
    ) -> Result<(), String> {
        warn!(
            "Migrating Fjall partition {} from u128 to u64 timestamps (zeroing all timestamps)",
            partition_name
        );

        let mut entry_count = 0;

        for entry in partition.iter() {
            let (key, value) = entry.map_err(|e| format!("Failed to read entry: {}", e))?;

            let value_bytes: ByteView = value.into();

            // Try to read as u128 timestamp (16 bytes)
            if value_bytes.len() < 16 {
                warn!(
                    "Skipping entry in {} with value too short: {} bytes",
                    partition_name,
                    value_bytes.len()
                );
                continue;
            }

            // Extract the codomain bytes (everything after the old u128 timestamp)
            let codomain_bytes = value_bytes.slice(16..);

            // Construct new value: u64 timestamp (set to 0) + codomain bytes
            let mut new_value = Vec::with_capacity(8 + codomain_bytes.len());
            new_value.extend_from_slice(&0u64.to_le_bytes());
            new_value.extend_from_slice(&codomain_bytes);

            partition
                .insert(ByteView::from(key.to_vec()), ByteView::from(new_value))
                .map_err(|e| format!("Failed to rewrite entry: {}", e))?;

            entry_count += 1;
        }

        info!(
            "Successfully migrated {} entries in partition {}",
            entry_count, partition_name
        );
        Ok(())
    }
}

impl Migrator for FjallMigrator {
    fn migrate_if_needed(&self) -> Result<(), String> {
        let current = self.get_db_version().unwrap_or_else(|_| {
            // No version marker = version 1.0.0 (original format with u128 timestamps)
            Version::parse("1.0.0").unwrap()
        });

        let target = Version::parse(CURRENT_DB_VERSION).unwrap();

        if current >= target {
            return Ok(()); // Already up to date
        }

        warn!(
            "Fjall database version {} needs migration to version {}",
            current, target
        );

        // Run migrations based on major version changes
        // Each migration handles moving from one major version to the next
        let mut from_version = current.major;
        while from_version < target.major {
            match from_version {
                1 => {
                    self.fjall_migrate_v1_to_v2()?;
                    from_version = 2;
                }
                _ => {
                    return Err(format!(
                        "Unknown Fjall migration path from version {}",
                        from_version
                    ));
                }
            }
        }

        // Mark migration complete
        self.mark_current_version()?;

        info!("Fjall database migration completed to version {}", target);
        Ok(())
    }

    fn mark_current_version(&self) -> Result<(), String> {
        self.sequences_partition
            .insert(VERSION_KEY, CURRENT_DB_VERSION.as_bytes())
            .map_err(|e| format!("Failed to write version marker: {}", e))?;

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

        // Fresh database should have no version marker (version 1)
        assert!(migrator.get_db_version().is_err());

        // Mark as current version
        migrator.mark_current_version().unwrap();

        // Should now have current version (2.0.0)
        let version = migrator.get_db_version().unwrap();
        assert_eq!(version, Version::parse("2.0.0").unwrap());
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

        // Verify version is updated to 2.0.0
        let version = migrator.get_db_version().unwrap();
        assert_eq!(version, Version::parse("2.0.0").unwrap());

        // Running migration again should be a no-op
        migrator.migrate_if_needed().unwrap();
    }

    #[test]
    fn test_fjall_u128_to_u64_migration() {
        let tmpdir = TempDir::new().unwrap();
        let keyspace = Config::new(tmpdir.path()).open().unwrap();
        let test_partition = keyspace
            .open_partition("test", PartitionCreateOptions::default())
            .unwrap();

        // Create some test data with u128 timestamps (16 bytes)
        let entries = vec![
            (b"key1".as_slice(), 1000u128, b"value1".as_slice()),
            (b"key2".as_slice(), 2000u128, b"value2".as_slice()),
            (b"key3".as_slice(), 1500u128, b"value3".as_slice()),
        ];

        // Write old format (u128 timestamp + value)
        for (key, ts, value) in &entries {
            let mut old_format = Vec::with_capacity(16 + value.len());
            old_format.extend_from_slice(&ts.to_le_bytes());
            old_format.extend_from_slice(value);
            test_partition
                .insert(ByteView::from(*key), ByteView::from(old_format))
                .unwrap();
        }

        // Perform migration
        FjallMigrator::fjall_migrate_partition_u128_to_u64(&test_partition, "test").unwrap();

        // Verify all entries are migrated to u64 format (8 bytes)
        for (key, _, expected_value) in &entries {
            let result = test_partition.get(*key).unwrap().unwrap();
            let result_bytes: ByteView = result.into();

            // Should now have 8-byte timestamp
            assert!(result_bytes.len() >= 8 + expected_value.len());

            // Extract timestamp (should be u64 now)
            let _ts = u64::from_le_bytes(result_bytes[0..8].try_into().unwrap());

            // Extract value
            let value_bytes = &result_bytes[8..];
            assert_eq!(value_bytes, *expected_value);
        }

        // Verify all timestamps are now zero (migration zeros all timestamps)
        let mut timestamps = Vec::new();
        for entry in test_partition.iter() {
            let (_, value) = entry.unwrap();
            let value_bytes: ByteView = value.into();
            let ts = u64::from_le_bytes(value_bytes[0..8].try_into().unwrap());
            timestamps.push(ts);
        }

        // All timestamps should be zero after migration
        assert_eq!(timestamps, vec![0, 0, 0]);
    }

    #[test]
    fn test_fjall_migration_timestamp_consistency() {
        let tmpdir = TempDir::new().unwrap();
        let keyspace = Config::new(tmpdir.path()).open().unwrap();
        let test_partition = keyspace
            .open_partition("test", PartitionCreateOptions::default())
            .unwrap();

        // Create entries where multiple keys share the same timestamp
        // This tests that all entries with the same old timestamp get the same new timestamp
        let entries = vec![
            (b"a".as_slice(), 100u128, b"first".as_slice()),
            (b"b".as_slice(), 100u128, b"also_first".as_slice()), // Same timestamp as 'a'
            (b"c".as_slice(), 200u128, b"second".as_slice()),
        ];

        // Write old format
        for (key, ts, value) in &entries {
            let mut old_format = Vec::with_capacity(16 + value.len());
            old_format.extend_from_slice(&ts.to_le_bytes());
            old_format.extend_from_slice(value);
            test_partition
                .insert(ByteView::from(*key), ByteView::from(old_format))
                .unwrap();
        }

        // Perform migration
        FjallMigrator::fjall_migrate_partition_u128_to_u64(&test_partition, "test").unwrap();

        // Read back and verify consistency
        let a_result = test_partition.get(b"a").unwrap().unwrap();
        let a_bytes: ByteView = a_result.into();
        let a_ts = u64::from_le_bytes(a_bytes[0..8].try_into().unwrap());

        let b_result = test_partition.get(b"b").unwrap().unwrap();
        let b_bytes: ByteView = b_result.into();
        let b_ts = u64::from_le_bytes(b_bytes[0..8].try_into().unwrap());

        let c_result = test_partition.get(b"c").unwrap().unwrap();
        let c_bytes: ByteView = c_result.into();
        let c_ts = u64::from_le_bytes(c_bytes[0..8].try_into().unwrap());

        // All timestamps should now be zero after migration
        assert_eq!(a_ts, 0, "All timestamps should be zero after migration");
        assert_eq!(b_ts, 0, "All timestamps should be zero after migration");
        assert_eq!(c_ts, 0, "All timestamps should be zero after migration");
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
        let version = migrator.get_db_version().unwrap();
        assert_eq!(version, Version::parse("2.0.0").unwrap());
        assert_eq!(version.major, 2);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
    }
}
