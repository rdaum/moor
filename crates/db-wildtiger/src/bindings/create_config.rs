// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::bindings::data::{pack_format_string, FormatType};

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum BlockAllocation {
    First,
    Best,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BlockCompressor {
    None,
    Snappy,
    Zlib,
    Custom(String),
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum BlockChecksum {
    On,
    Off,
    Uncompressed,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum HuffmanEncoding {
    None,
    English,
    Utf8,
    Utf16,
}

#[allow(dead_code)]
impl HuffmanEncoding {
    pub fn to_encoding(&self) -> String {
        match self {
            HuffmanEncoding::None => "none".into(),
            HuffmanEncoding::English => "english".into(),
            HuffmanEncoding::Utf8 => "utf8".into(),
            HuffmanEncoding::Utf16 => "utf16".into(),
        }
    }
}

/*

*/
#[derive(Debug, Default)]
pub struct CreateConfig {
    /// It is recommended that workloads that consist primarily of updates and/or point queries specify random.
    /// Workloads that do many cursor scans through large ranges of data should specify sequential and other workloads should specify none.
    /// The option leads to an appropriate operating system advisory call where available.
    access_pattern_hint: Option<String>,
    /// The file unit allocation size, in bytes, must be a power of two; smaller values decrease the file space required by overflow items,
    /// and the default value of 4KB is a good choice absent requirements from the operating system or storage device.
    allocation_size: Option<usize>,
    /// Application-owned metadata for this object.
    app_metadata: Option<String>,
    /// Declare timestamp usage.
    assert: Option<String>,
    /// Configure block allocation. Permitted values are "best" or "first"; the "best" configuration uses a best-fit algorithm,
    /// the "first" configuration uses a first-available algorithm during block allocation.
    block_allocation: Option<BlockAllocation>,
    /// Configure a compressor for file blocks. Permitted values are "none" or a custom compression engine name created with WT_CONNECTION::add_compressor.
    /// If WiredTiger has builtin support for "lz4", "snappy", "zlib" or "zstd" compression, these names are also available. See Compressors for more information.
    block_compressor: Option<BlockCompressor>,
    /// Do not ever evict the object's pages from cache. Not compatible with LSM tables; see Cache resident objects for more information.
    cache_resident: Option<bool>,
    /// Configure block checksums; the permitted values are on, off, uncompressed and unencrypted. The default is on, in which case all block writes include a checksum subsequently verified when the block is read.
    /// The off setting does no checksums, the uncompressed setting only checksums blocks that are not compressed, and the unencrypted setting only checksums blocks that are not encrypted. See Checksums for more information.
    checksum: Option<BlockChecksum>,
    /// Comma-separated list of names of column groups. Each column group is stored separately, keyed by the primary key of the table.
    /// If no column groups are specified, all columns are stored together in a single file. All value columns in the table must appear in at least one column group.
    /// Each column group must be created with a separate call to WT_SESSION::create using a colgroup: URI.
    colgroups: Option<Vec<String>>,
    /// Configure custom collation for keys. Permitted values are "none" or a custom collator name created with WT_CONNECTION::add_collator.
    collator: Option<String>,
    /// List of the column names. Comma-separated list of the form (column[,...]). For tables, the number of entries must match the total number of values in key_format and value_format.
    /// For colgroups and indices, all column names must appear in the list of columns for the table.
    columns: Option<Vec<String>>,
    /// The maximum number of unique values remembered in the row-store/variable-length column-store leaf page value dictionary; see File formats and compression for more information.
    dictionary: Option<u64>,
    /// Configure an encryptor for file blocks. When a table is created, its encryptor is not implicitly used for any related indices or column groups.
    encryption: Option<EncryptionConfig>,
    /// Fail if the object exists. When false (the default), if the object exists, check that its settings match the specified configuration.
    exclusive: Option<bool>,
    /// Configure a custom extractor for indices. Permitted values are "none" or an extractor name created with WT_CONNECTION::add_extractor.
    extractor: Option<String>,
    /// The file format. a string, chosen from the following options: "btree"; default btree.
    format: Option<String>,
    /// Allow update and insert operations to proceed even if the cache is already at capacity. Only valid in conjunction with in-memory databases.
    /// Should be used with caution - this configuration allows WiredTiger to consume memory over the configured cache limit.
    ignore_in_memory_cache_size: Option<bool>,
    /// Configure the index to be immutable – that is, the index is not changed by any update to a record in the table.
    immutable: Option<bool>,
    /// Configure import of an existing object into the currently running database.
    import: Option<ImportConfig>,
    /// This option is no longer supported, retained for backward compatibility.
    internal_key_max: Option<usize>,
    /// Configure internal key truncation, discarding unnecessary trailing bytes on internal keys (ignored for custom collators).
    internal_key_truncate: Option<bool>,
    /// The maximum page size for internal nodes, in bytes; the size must be a multiple of the allocation size and is significant for applications wanting to avoid excessive L2 cache misses while searching the tree.
    /// The page maximum is the bytes of uncompressed data, that is, the limit is applied before any block compression is done.
    internal_page_max: Option<usize>,
    /// The format of the data packed into key items. See Format types for details. By default, the key_format is 'u' and applications use WT_ITEM structures to manipulate raw byte arrays.
    /// By default, records are stored in row-store files: keys of type 'r' are record numbers and records referenced by record number are stored in column-store files.
    key_format: Option<Vec<FormatType>>,
    /// This option is no longer supported, retained for backward compatibility.
    key_gap: Option<usize>,
    /// The largest key stored in a leaf node, in bytes. If set, keys larger than the specified size are stored as overflow items (which may require additional I/O to access).
    /// The default value is one-tenth the size of a newly split leaf page.
    leaf_key_max: Option<usize>,
    /// The maximum page size for leaf nodes, in bytes; the size must be a multiple of the allocation size, and is significant for applications wanting to maximize sequential data transfer from a storage device.
    /// The page maximum is the bytes of uncompressed data, that is, the limit is applied before any block compression is done.
    /// For fixed-length column store, the size includes only the bitmap data; pages containing timestamp information can be larger, and the size is limited to 128KB rather than 512MB.
    leaf_page_max: Option<usize>,
    /// The largest value stored in a leaf node, in bytes. If set, values larger than the specified size are stored as overflow items (which may require additional I/O to access).
    /// If the size is larger than the maximum leaf page size, the page size is temporarily ignored when large values are written. The default is one-half the size of a newly split leaf page.
    leaf_value_max: Option<usize>,
    /// The maximum size a page can grow to in memory before being reconciled to disk. The specified size will be adjusted to a lower bound of leaf_page_max, and an upper bound of cache_size / 10.
    /// This limit is soft - it is possible for pages to be temporarily larger than this value. This setting is ignored for LSM trees, see chunk_size.
    memory_page_max: Option<usize>,
    /// Maximum dirty system buffer cache usage, in bytes. If non-zero, schedule writes for dirty blocks belonging to this object in the system buffer cache after that many bytes from this object are written into the buffer cache.
    os_cache_dirty_max: Option<usize>,
    /// Maximum system buffer cache usage, in bytes. If non-zero, evict object blocks from the system buffer cache after that many bytes from this object are read or written into the buffer cache.
    os_cache_max: Option<usize>,
    /// Configure prefix compression on row-store leaf pages.
    prefix_compression: Option<bool>,
    /// Minimum gain before prefix compression will be used on row-store leaf pages.
    prefix_compression_min: Option<usize>,
    /// The Btree page split size as a percentage of the maximum Btree page size, that is, when a Btree page is split, it will be split into smaller pages,
    /// where each page is the specified percentage of the maximum Btree page size.
    split_pct: Option<usize>,
    /// Set the type of data source used to store a column group, index or simple table. By default, a "file:" URI is derived from the object name.
    /// The type configuration can be used to switch to a different data source, such as LSM or an extension configured by the application.
    r#type: Option<String>,
    /// The format of the data packed into value items. See Format types for details. By default, the value_format is 'u' and applications use a WT_ITEM structure to manipulate raw byte arrays.
    /// Value items of type 't' are bitfields, and when configured with record number type keys, will be stored using a fixed-length store.
    value_format: Option<Vec<FormatType>>,
    /// Describe how timestamps are expected to be used on table modifications. The choices are the default, which ensures that once timestamps are used for a key, they are always used,
    /// and also that multiple updates to a key never use decreasing timestamps and never which enforces that timestamps are never used for a table.
    /// (The always, key_consistent, mixed_mode and ordered choices should not be used, and are retained for backward compatibility.).
    write_timestamp_usage: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct ImportConfig {
    /// Allow importing files with timestamps smaller or equal to the configured global timestamps.
    compare_timestamp: Option<String>,
    /// Whether to import the input URI from disk.
    enabled: Option<bool>,
    /// The file configuration extracted from the metadata of the export database.
    file_metadata: Option<String>,
    /// A text file that contains all the relevant metadata information for the URI to import.
    metadata_file: Option<String>,
    /// Whether to reconstruct the metadata from the raw file content.
    repair: Option<bool>,
}

impl ImportConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn compare_timestamp(mut self, compare_timestamp: String) -> Self {
        self.compare_timestamp = Some(compare_timestamp);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    pub fn file_metadata(mut self, file_metadata: String) -> Self {
        self.file_metadata = Some(file_metadata);
        self
    }

    pub fn metadata_file(mut self, metadata_file: String) -> Self {
        self.metadata_file = Some(metadata_file);
        self
    }

    pub fn repair(mut self, repair: bool) -> Self {
        self.repair = Some(repair);
        self
    }

    pub fn as_config_string(&self) -> String {
        let mut options = Vec::new();

        if let Some(compare_timestamp) = &self.compare_timestamp {
            options.push(format!("compare_timestamp={}", compare_timestamp));
        }

        if let Some(enabled) = &self.enabled {
            options.push(format!("enabled={}", enabled));
        }

        if let Some(file_metadata) = &self.file_metadata {
            options.push(format!("file_metadata={}", file_metadata));
        }

        if let Some(metadata_file) = &self.metadata_file {
            options.push(format!("metadata_file={}", metadata_file));
        }

        if let Some(repair) = &self.repair {
            options.push(format!("repair={}", repair));
        }

        options.join(",")
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EncryptionConfig {
    /// An identifier that identifies a unique instance of the encryptor. It is stored in clear text, and thus is available when the WiredTiger database is reopened.
    /// On the first use of a (name, keyid) combination, the WT_ENCRYPTOR::customize function is called with the keyid as an argument.
    keyid: Option<String>,
    /// Permitted values are "none" or a custom encryption engine name created with WT_CONNECTION::add_encryptor. See Encryptors for more information.
    name: Option<String>,
}

#[allow(dead_code)]
impl EncryptionConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn keyid(mut self, keyid: String) -> Self {
        self.keyid = Some(keyid);
        self
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn as_config_string(&self) -> String {
        let mut options = Vec::new();

        if let Some(keyid) = &self.keyid {
            options.push(format!("keyid={}", keyid));
        }

        if let Some(name) = &self.name {
            options.push(format!("name={}", name));
        }

        options.join(",")
    }
}

#[allow(dead_code)]
impl CreateConfig {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn access_pattern_hint(mut self, access_pattern_hint: String) -> Self {
        self.access_pattern_hint = Some(access_pattern_hint);
        self
    }

    pub fn allocation_size(mut self, allocation_size: usize) -> Self {
        self.allocation_size = Some(allocation_size);
        self
    }

    pub fn app_metadata(mut self, app_metadata: String) -> Self {
        self.app_metadata = Some(app_metadata);
        self
    }

    pub fn assert(mut self, assert: String) -> Self {
        self.assert = Some(assert);
        self
    }

    pub fn block_allocation(mut self, block_allocation: BlockAllocation) -> Self {
        self.block_allocation = Some(block_allocation);
        self
    }

    pub fn block_compressor(mut self, block_compressor: BlockCompressor) -> Self {
        self.block_compressor = Some(block_compressor);
        self
    }

    pub fn cache_resident(mut self, cache_resident: bool) -> Self {
        self.cache_resident = Some(cache_resident);
        self
    }

    pub fn checksum(mut self, checksum: BlockChecksum) -> Self {
        self.checksum = Some(checksum);
        self
    }

    pub fn colgroups(mut self, colgroups: &[&str]) -> Self {
        self.colgroups = Some(colgroups.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn collator(mut self, collator: String) -> Self {
        self.collator = Some(collator);
        self
    }

    pub fn columns(mut self, columns: &[&str]) -> Self {
        self.columns = Some(columns.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn dictionary(mut self, dictionary: u64) -> Self {
        self.dictionary = Some(dictionary);
        self
    }

    pub fn exclusive(mut self, exclusive: bool) -> Self {
        self.exclusive = Some(exclusive);
        self
    }

    pub fn extractor(mut self, extractor: String) -> Self {
        self.extractor = Some(extractor);
        self
    }

    pub fn immutable(mut self, immutable: bool) -> Self {
        self.immutable = Some(immutable);
        self
    }

    pub fn internal_key_max(mut self, internal_key_max: usize) -> Self {
        self.internal_key_max = Some(internal_key_max);
        self
    }

    pub fn internal_key_truncate(mut self, internal_key_truncate: bool) -> Self {
        self.internal_key_truncate = Some(internal_key_truncate);
        self
    }

    pub fn internal_page_max(mut self, internal_page_max: usize) -> Self {
        self.internal_page_max = Some(internal_page_max);
        self
    }

    pub fn key_format(mut self, key_format: &[FormatType]) -> Self {
        self.key_format = Some(key_format.to_vec());
        self
    }

    pub fn leaf_key_max(mut self, leaf_key_max: usize) -> Self {
        self.leaf_key_max = Some(leaf_key_max);
        self
    }

    pub fn leaf_page_max(mut self, leaf_page_max: usize) -> Self {
        self.leaf_page_max = Some(leaf_page_max);
        self
    }

    pub fn leaf_value_max(mut self, leaf_value_max: usize) -> Self {
        self.leaf_value_max = Some(leaf_value_max);
        self
    }

    pub fn memory_page_max(mut self, memory_page_max: usize) -> Self {
        self.memory_page_max = Some(memory_page_max);
        self
    }

    pub fn os_cache_dirty_max(mut self, os_cache_dirty_max: usize) -> Self {
        self.os_cache_dirty_max = Some(os_cache_dirty_max);
        self
    }

    pub fn os_cache_max(mut self, os_cache_max: usize) -> Self {
        self.os_cache_max = Some(os_cache_max);
        self
    }

    pub fn prefix_compression(mut self, prefix_compression: bool) -> Self {
        self.prefix_compression = Some(prefix_compression);
        self
    }

    pub fn prefix_compression_min(mut self, prefix_compression_min: usize) -> Self {
        self.prefix_compression_min = Some(prefix_compression_min);
        self
    }

    pub fn split_pct(mut self, split_pct: usize) -> Self {
        self.split_pct = Some(split_pct);
        self
    }

    pub fn r#type(mut self, r#type: String) -> Self {
        self.r#type = Some(r#type);
        self
    }

    pub fn value_format(mut self, value_format: &[FormatType]) -> Self {
        self.value_format = Some(value_format.to_vec());
        self
    }

    pub fn write_timestamp_usage(mut self, write_timestamp_usage: String) -> Self {
        self.write_timestamp_usage = Some(write_timestamp_usage);
        self
    }

    pub fn import(mut self, import: ImportConfig) -> Self {
        self.import = Some(import);
        self
    }

    pub fn as_config_string(&self) -> String {
        let mut options = Vec::new();
        if let Some(allocation_size) = &self.allocation_size {
            options.push(format!("allocation_size={}", allocation_size));
        }

        if let Some(app_metadata) = &self.app_metadata {
            options.push(format!("app_metadata={}", app_metadata));
        }

        if let Some(block_allocation) = &self.block_allocation {
            let block_allocation = match block_allocation {
                BlockAllocation::First => "first",
                BlockAllocation::Best => "best",
            };
            options.push(format!("block_allocation={}", block_allocation));
        }

        if let Some(block_compressor) = &self.block_compressor {
            let block_compressor = match block_compressor {
                BlockCompressor::None => "none",
                BlockCompressor::Snappy => "snappy",
                BlockCompressor::Zlib => "zlib",
                BlockCompressor::Custom(custom) => custom,
            };
            options.push(format!("block_compressor={}", block_compressor));
        }

        if let Some(cache_resident) = &self.cache_resident {
            options.push(format!("cache_resident={}", cache_resident));
        }

        if let Some(checksum) = &self.checksum {
            let checksum = match checksum {
                BlockChecksum::On => "on",
                BlockChecksum::Off => "off",
                BlockChecksum::Uncompressed => "uncompressed",
            };
            options.push(format!("checksum={}", checksum));
        }

        if let Some(colgroups) = &self.colgroups {
            options.push(format!("colgroups:({})", colgroups.join(",")));
        }

        if let Some(collator) = &self.collator {
            options.push(format!("collator={}", collator));
        }

        if let Some(columns) = &self.columns {
            options.push(format!("columns=({})", columns.join(",")));
        }

        if let Some(dictionary) = &self.dictionary {
            options.push(format!("dictionary={}", dictionary));
        }

        if let Some(exclusive) = &self.exclusive {
            options.push(format!("exclusive={}", exclusive));
        }

        if let Some(extractor) = &self.extractor {
            options.push(format!("extractor={}", extractor));
        }

        if let Some(immutable) = &self.immutable {
            options.push(format!("immutable={}", immutable));
        }

        if let Some(internal_key_max) = &self.internal_key_max {
            options.push(format!("internal_key_max={}", internal_key_max));
        }

        if let Some(internal_key_truncate) = &self.internal_key_truncate {
            options.push(format!("internal_key_truncate={}", internal_key_truncate));
        }

        if let Some(internal_page_max) = &self.internal_page_max {
            options.push(format!("internal_page_max={}", internal_page_max));
        }

        if let Some(key_format) = &self.key_format {
            options.push(format!(
                "key_format={}",
                pack_format_string(key_format).as_str()
            ));
        }

        if let Some(leaf_key_max) = &self.leaf_key_max {
            options.push(format!("leaf_key_max={}", leaf_key_max));
        }

        if let Some(leaf_page_max) = &self.leaf_page_max {
            options.push(format!("leaf_page_max={}", leaf_page_max));
        }

        if let Some(leaf_value_max) = &self.leaf_value_max {
            options.push(format!("leaf_value_max={}", leaf_value_max));
        }

        if let Some(memory_page_max) = &self.memory_page_max {
            options.push(format!("memory_page_max={}", memory_page_max));
        }

        if let Some(os_cache_dirty_max) = &self.os_cache_dirty_max {
            options.push(format!("os_cache_dirty_max={}", os_cache_dirty_max));
        }

        if let Some(os_cache_max) = &self.os_cache_max {
            options.push(format!("os_cache_max={}", os_cache_max));
        }

        if let Some(prefix_compression) = &self.prefix_compression {
            options.push(format!("prefix_compression={}", prefix_compression));
        }

        if let Some(prefix_compression_min) = &self.prefix_compression_min {
            options.push(format!("prefix_compression_min={}", prefix_compression_min));
        }

        if let Some(split_pct) = &self.split_pct {
            options.push(format!("split_pct={}", split_pct));
        }

        if let Some(r#type) = &self.r#type {
            options.push(format!("type={}", r#type));
        }

        if let Some(value_format) = &self.value_format {
            options.push(format!(
                "value_format={}",
                pack_format_string(value_format).as_str()
            ));
        }

        if let Some(write_timestamp_usage) = &self.write_timestamp_usage {
            options.push(format!("write_timestamp_usage={}", write_timestamp_usage));
        }

        if let Some(encryption) = &self.encryption {
            options.push(format!("encryption={}", encryption.as_config_string()));
        }

        if let Some(import) = &self.import {
            options.push(format!("import={}", import.as_config_string()));
        }

        if let Some(access_pattern_hint) = &self.access_pattern_hint {
            options.push(format!("access_pattern_hint={}", access_pattern_hint));
        }

        if let Some(assert) = &self.assert {
            options.push(format!("assert={}", assert));
        }

        if let Some(ignore_in_memory_cache_size) = &self.ignore_in_memory_cache_size {
            options.push(format!(
                "ignore_in_memory_cache_size={}",
                ignore_in_memory_cache_size
            ));
        }

        if let Some(key_gap) = &self.key_gap {
            options.push(format!("key_gap={}", key_gap));
        }

        if let Some(format) = &self.format {
            options.push(format!("format={}", format));
        }

        if let Some(internal_key_max) = &self.internal_key_max {
            options.push(format!("internal_key_max={}", internal_key_max));
        }

        options.join(",")
    }
}

#[derive(Debug, Default, Clone)]
pub struct DropConfig {
    force: Option<bool>,
    remove_files: Option<bool>,
}

#[allow(dead_code)]
impl DropConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn force(mut self, force: bool) -> Self {
        self.force = Some(force);
        self
    }

    pub fn remove_files(mut self, remove_files: bool) -> Self {
        self.remove_files = Some(remove_files);
        self
    }

    pub fn as_config_string(&self) -> String {
        let mut options = vec![];

        if let Some(force) = &self.force {
            options.push(format!("force={}", force));
        }

        if let Some(remove_files) = &self.remove_files {
            options.push(format!("remove_files={}", remove_files));
        }

        options.join(",")
    }
}
