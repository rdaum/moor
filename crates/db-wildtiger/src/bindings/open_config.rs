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

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct OpenConfig {
    /// If non-empty and restoring from a backup, restore only the table object targets listed.
    /// WiredTiger will remove all the metadata entries for the tables that are not listed in the
    /// list from the reconstructed metadata. The target list must include URIs of type table:
    backup_restore_options: Option<Vec<String>>,
    /// in-memory alignment (in bytes) for buffers used for I/O. The default value of -1 indicates
    /// a platform-specific alignment value should be used (4KB on Linux systems when direct I/O is
    /// configured, zero elsewhere). If the configured alignment is larger than default or configured
    /// object page sizes, file allocation and page sizes are silently increased to the buffer
    /// alignment size.
    buffer_alignment: Option<i32>,
    /// enable caching of cursors for reuse. This is the default value for any sessions created,
    /// and can be overridden in configuring cache_cursors in WT_CONNECTION.open_session.
    cache_cursors: Option<bool>,
    /// the maximum number of milliseconds an application thread will wait for space to be available
    /// in cache before giving up. Default will wait forever.
    cache_max_wait_ms: Option<i64>,
    /// assume the heap allocator overhead is the specified percentage, and adjust the cache usage
    /// by that amount (for example, if there is 10GB of data in cache, a percentage of 10 means
    /// WiredTiger treats this as 11GB). This value is configurable because different heap
    /// allocators have different overhead and different workloads will have different heap
    /// allocation sizes and patterns, therefore applications may need to adjust this value based
    /// on allocator choice and behavior in measured workloads.
    cache_overhead: Option<i32>,
    /// maximum heap memory to allocate for the cache. A database should configure either cache_size
    /// or shared_cache but not both.
    cache_size: Option<i64>,
    /// the number of milliseconds to wait before a stuck cache times out in diagnostic mode.
    /// Default will wait for 5 minutes, 0 will wait forever.
    cache_stuck_timeout_ms: Option<i64>,
    /// create the database if it does not exist.
    create: Option<bool>,
    /// fail if the database already exists, generally used with the create option.
    exclusive: Option<bool>,
    /// control the settings of various extended debugging features.
    debug_mode: Option<OpenDebug>,
    /// enable logging. Enabling logging uses three sessions from the configured session_max.
    log: Option<LogConfig>,
    /// enable tracking of performance-critical functions. See Track function calls for more information.
    operation_tracking: Option<OperationTracking>,
    /// enable additional diagnostics.
    extra_diagnostics: Option<Vec<String>>,
    /// keep data in memory only. See In-memory databases for more information.	a boolean flag; default false
    in_memory: Option<bool>,
    /// enable messages for various subsystems and operations. Options are given as a list, where
    /// each message type can optionally define an associated verbosity level,
    /// such as "verbose=[evictserver,read:1,rts:0]". Verbosity levels that can be provided include
    /// 0 (INFO) and 1 (DEBUG).
    verbose: Option<Vec<(Verbosity, Option<i8>)>>,
    /// how to sync log records when the transaction commits.
    transaction_sync: Option<TransactionSync>,
    // TODO add more.
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TransactionSync {
    /// whether to sync the log on every commit by default, can be overridden by the sync setting to WT_SESSION::commit_transaction.
    /// a boolean flag; default false.
    enabled: Option<bool>,
    /// the method used to ensure log records are stable on disk, see Commit-level durability for more information.
    method: Option<SyncMethod>,
}

impl TransactionSync {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn as_option_string(&self) -> String {
        let mut options = vec![];

        if let Some(enabled) = &self.enabled {
            options.push(format!("enabled={}", enabled));
        }

        if let Some(method) = &self.method {
            options.push(format!("method={}", method.to_string()));
        }
        options.join(",")
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    pub fn method(mut self, method: SyncMethod) -> Self {
        self.method = Some(method);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum SyncMethod {
    ///  By default, the configured value is fsync, which calls the operating system's fsync call
    /// (or fdatasync if available) as each commit completes.
    Fsync,
    /// If the value is set to dsync, the O_DSYNC or O_SYNC flag to the operating system's open call
    /// will be specified when the file is opened. (The durability guarantees of the fsync and dsync
    /// configurations are the same, and in our experience the open flags are slower; this
    /// configuration is only included for systems where that may not be the case.)
    Dsync,
    /// If the value is set to none, the operating system's write call will be called as each commit
    /// completes but no explicit disk flush is made. This setting gives durability across application
    /// failure, but likely not across system failure (depending on operating system guarantees).
    None,
}

#[allow(dead_code)]
impl SyncMethod {
    pub fn to_string(&self) -> String {
        match self {
            SyncMethod::Fsync => "fsync".to_string(),
            SyncMethod::Dsync => "dsync".to_string(),
            SyncMethod::None => "none".to_string(),
        }
    }
}

#[allow(dead_code)]
impl OpenConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn as_option_string(&self) -> String {
        let mut options = vec![];

        if let Some(backup_restore_options) = &self.backup_restore_options {
            let backup_restore_options = backup_restore_options.join(",");
            options.push(format!("backup_restore_options={}", backup_restore_options));
        }

        if let Some(buffer_alignment) = &self.buffer_alignment {
            options.push(format!("buffer_alignment={}", buffer_alignment));
        }

        if let Some(cache_cursors) = &self.cache_cursors {
            options.push(format!("cache_cursors={}", cache_cursors));
        }

        if let Some(cache_max_wait_ms) = &self.cache_max_wait_ms {
            options.push(format!("cache_max_wait_ms={}", cache_max_wait_ms));
        }

        if let Some(cache_overhead) = &self.cache_overhead {
            options.push(format!("cache_overhead={}", cache_overhead));
        }

        if let Some(cache_size) = &self.cache_size {
            options.push(format!("cache_size={}", cache_size));
        }

        if let Some(cache_stuck_timeout_ms) = &self.cache_stuck_timeout_ms {
            options.push(format!("cache_stuck_timeout_ms={}", cache_stuck_timeout_ms));
        }

        if let Some(create) = &self.create {
            options.push(format!("create={}", create));
        }

        if let Some(exclusive) = &self.exclusive {
            options.push(format!("exclusive={}", exclusive));
        }

        if let Some(debug) = &self.debug_mode {
            options.push(format!("debug_mode=({})", debug.as_option_string()));
        }

        if let Some(log) = &self.log {
            options.push(format!("log=({})", log.as_option_string()));
        }

        if let Some(operation_tracking) = &self.operation_tracking {
            options.push(format!(
                "operation_tracking=({})",
                operation_tracking.as_option_string()
            ));
        }

        if let Some(extra_diagnostics) = &self.extra_diagnostics {
            options.push(format!(
                "extra_diagnostics=[{}]",
                extra_diagnostics.join(",")
            ));
        }

        if let Some(in_memory) = &self.in_memory {
            options.push(format!("in_memory={}", in_memory));
        }

        if let Some(verbose) = &self.verbose {
            let verbose = verbose
                .iter()
                .map(|(v, l)| {
                    if let Some(l) = l {
                        format!("{}:{}", v.to_string(), l)
                    } else {
                        v.to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join(",");
            options.push(format!("verbose=[{}]", verbose));
        }

        if let Some(transaction_sync) = &self.transaction_sync {
            options.push(format!(
                "transaction_sync=({})",
                transaction_sync.as_option_string()
            ));
        }

        options.join(",")
    }

    pub fn backup_restore_options(mut self, backup_restore_options: Vec<String>) -> Self {
        self.backup_restore_options = Some(backup_restore_options);
        self
    }

    pub fn buffer_alignment(mut self, buffer_alignment: i32) -> Self {
        self.buffer_alignment = Some(buffer_alignment);
        self
    }

    pub fn cache_cursors(mut self, cache_cursors: bool) -> Self {
        self.cache_cursors = Some(cache_cursors);
        self
    }

    pub fn cache_max_wait_ms(mut self, cache_max_wait_ms: i64) -> Self {
        self.cache_max_wait_ms = Some(cache_max_wait_ms);
        self
    }

    pub fn cache_overhead(mut self, cache_overhead: i32) -> Self {
        self.cache_overhead = Some(cache_overhead);
        self
    }

    pub fn cache_size(mut self, cache_size: i64) -> Self {
        self.cache_size = Some(cache_size);
        self
    }

    pub fn cache_stuck_timeout_ms(mut self, cache_stuck_timeout_ms: i64) -> Self {
        self.cache_stuck_timeout_ms = Some(cache_stuck_timeout_ms);
        self
    }
    pub fn create(mut self, create: bool) -> Self {
        self.create = Some(create);
        self
    }

    pub fn exclusive(mut self, exclusive: bool) -> Self {
        self.exclusive = Some(exclusive);
        self
    }

    pub fn debug(mut self, debug: OpenDebug) -> Self {
        self.debug_mode = Some(debug);
        self
    }

    pub fn log(mut self, log: LogConfig) -> Self {
        self.log = Some(log);
        self
    }

    pub fn operation_tracking(mut self, operation_tracking: OperationTracking) -> Self {
        self.operation_tracking = Some(operation_tracking);
        self
    }

    pub fn extra_diagnostics(mut self, extra_diagnostics: Vec<String>) -> Self {
        self.extra_diagnostics = Some(extra_diagnostics);
        self
    }

    pub fn in_memory(mut self, in_memory: bool) -> Self {
        self.in_memory = Some(in_memory);
        self
    }

    pub fn verbose(mut self, verbose: Vec<(Verbosity, Option<i8>)>) -> Self {
        self.verbose = Some(verbose);
        self
    }

    pub fn transaction_sync(mut self, transaction_sync: TransactionSync) -> Self {
        self.transaction_sync = Some(transaction_sync);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct OpenDebug {
    /// if true, write transaction related information to the log for all operations, even operations
    /// for tables with logging turned off. This additional logging information is intended for debugging
    /// and is informational only, that is, it is ignored during recovery.
    table_logging: Option<bool>,
}

#[allow(dead_code)]
impl OpenDebug {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn as_option_string(&self) -> String {
        let mut options = vec![];

        if let Some(table_logging) = &self.table_logging {
            options.push(format!("table_logging={}", table_logging));
        }
        options.join(",")
    }

    pub fn table_logging(mut self, table_logging: bool) -> Self {
        self.table_logging = Some(table_logging);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LogConfig {
    /// configure a compressor for log records. Permitted values are "none" or custom compression engine name created with WT_CONNECTION::add_compressor. If WiredTiger has builtin support for "lz4", "snappy", "zlib" or "zstd" compression, these names are also available. See Compressors for more information.
    compressor: Option<String>,
    /// enable logging subsystem. a boolean flag; default false.
    enabled: Option<bool>,
    /// the maximum size of log files. an integer between 100KB and 2GB; default 100MB.
    file_max: Option<i32>,
    /// maximum dirty system buffer cache usage, as a percentage of the log's file_max. If non-zero, schedule writes for dirty blocks belonging to the log in the system buffer cache after that percentage of the log has been written into the buffer cache without an intervening file sync.
    os_cache_dirty_pct: Option<i32>,
    /// the name of a directory into which log files are written. The directory must already exist. If the value is not an absolute path, the path is relative to the database home (see Absolute paths for more information).
    path: Option<String>,
    /// pre-allocate log files. a boolean flag; default true.
    prealloc: Option<bool>,
    /// run recovery or error if recovery needs to run after an unclean shutdown. a string, chosen from the following options: "error", "on"; default on.
    recover: Option<String>,
    /// automatically remove unneeded log files. a boolean flag; default true.
    remove: Option<bool>,
    /// manually write zeroes into log files. a boolean flag; default false.
    zero_fill: Option<bool>,
}

#[allow(dead_code)]
impl LogConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn as_option_string(&self) -> String {
        let mut options = vec![];

        if let Some(compressor) = &self.compressor {
            options.push(format!("compressor={}", compressor));
        }

        if let Some(enabled) = &self.enabled {
            options.push(format!("enabled={}", enabled));
        }

        if let Some(file_max) = &self.file_max {
            options.push(format!("file_max={}", file_max));
        }

        if let Some(os_cache_dirty_pct) = &self.os_cache_dirty_pct {
            options.push(format!("os_cache_dirty_pct={}", os_cache_dirty_pct));
        }

        if let Some(path) = &self.path {
            options.push(format!("path={}", path));
        }

        if let Some(prealloc) = &self.prealloc {
            options.push(format!("prealloc={}", prealloc));
        }

        if let Some(recover) = &self.recover {
            options.push(format!("recover={}", recover));
        }

        if let Some(remove) = &self.remove {
            options.push(format!("remove={}", remove));
        }

        if let Some(zero_fill) = &self.zero_fill {
            options.push(format!("zero_fill={}", zero_fill));
        }
        options.join(",")
    }

    pub fn compressor(mut self, compressor: String) -> Self {
        self.compressor = Some(compressor);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    pub fn file_max(mut self, file_max: i32) -> Self {
        self.file_max = Some(file_max);
        self
    }

    pub fn os_cache_dirty_pct(mut self, os_cache_dirty_pct: i32) -> Self {
        self.os_cache_dirty_pct = Some(os_cache_dirty_pct);
        self
    }

    pub fn path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    pub fn prealloc(mut self, prealloc: bool) -> Self {
        self.prealloc = Some(prealloc);
        self
    }

    pub fn recover(mut self, recover: String) -> Self {
        self.recover = Some(recover);
        self
    }

    pub fn remove(mut self, remove: bool) -> Self {
        self.remove = Some(remove);
        self
    }

    pub fn zero_fill(mut self, zero_fill: bool) -> Self {
        self.zero_fill = Some(zero_fill);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct OperationTracking {
    /// enable operation tracking subsystem. a boolean flag; default false.
    enabled: Option<bool>,
    /// the name of a directory into which operation tracking files are written. The directory must already exist. If the value is not an absolute path, the path is relative to the database home (see Absolute paths for more information).
    path: Option<PathBuf>,
}

#[allow(dead_code)]
impl OperationTracking {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn as_option_string(&self) -> String {
        let mut options = vec![];

        if let Some(enabled) = &self.enabled {
            options.push(format!("enabled={}", enabled));
        }

        if let Some(path) = &self.path {
            options.push(format!("path={}", path.to_string_lossy()));
        }
        options.join(",")
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum Verbosity {
    Api,
    Backup,
    Block,
    BlockCache,
    Checkpoint,
    CheckpointCleanup,
    CheckpointProgress,
    Compact,
    CompactProgress,
    ErrorReturns,
    Evict,
    EvictStuck,
    EvictServer,
    FileOps,
    Generation,
    HandleOps,
    HistoryStore,
    HistoryStoreActivity,
    Log,
    Lsm,
    LsmManager,
    Metadata,
    Mutex,
    OutOfOrder,
    Overflow,
    Read,
    Reconcile,
    Recovery,
    RecoveryProgress,
    Rts,
    Salvage,
    SharedCache,
    Split,
    Temporary,
    ThreadGroup,
    Tiered,
    Timestamp,
    Transaction,
    Verify,
    Version,
    Write,
}

impl Verbosity {
    pub fn to_string(&self) -> String {
        match self {
            Verbosity::Api => "api".to_string(),
            Verbosity::Backup => "backup".to_string(),
            Verbosity::Block => "block".to_string(),
            Verbosity::BlockCache => "block_cache".to_string(),
            Verbosity::Checkpoint => "checkpoint".to_string(),
            Verbosity::CheckpointCleanup => "checkpoint_cleanup".to_string(),
            Verbosity::CheckpointProgress => "checkpoint_progress".to_string(),
            Verbosity::Compact => "compact".to_string(),
            Verbosity::CompactProgress => "compact_progress".to_string(),
            Verbosity::ErrorReturns => "error_returns".to_string(),
            Verbosity::Evict => "evict".to_string(),
            Verbosity::EvictStuck => "evict_stuck".to_string(),
            Verbosity::EvictServer => "evictserver".to_string(),
            Verbosity::FileOps => "fileops".to_string(),
            Verbosity::Generation => "generation".to_string(),
            Verbosity::HandleOps => "handleops".to_string(),
            Verbosity::HistoryStore => "history_store".to_string(),
            Verbosity::HistoryStoreActivity => "history_store_activity".to_string(),
            Verbosity::Log => "log".to_string(),
            Verbosity::Lsm => "lsm".to_string(),
            Verbosity::LsmManager => "lsm_manager".to_string(),
            Verbosity::Metadata => "metadata".to_string(),
            Verbosity::Mutex => "mutex".to_string(),
            Verbosity::OutOfOrder => "out_of_order".to_string(),
            Verbosity::Overflow => "overflow".to_string(),
            Verbosity::Read => "read".to_string(),
            Verbosity::Reconcile => "reconcile".to_string(),
            Verbosity::Recovery => "recovery".to_string(),
            Verbosity::RecoveryProgress => "recovery_progress".to_string(),
            Verbosity::Rts => "rts".to_string(),
            Verbosity::Salvage => "salvage".to_string(),
            Verbosity::SharedCache => "shared_cache".to_string(),
            Verbosity::Split => "split".to_string(),
            Verbosity::Temporary => "temporary".to_string(),
            Verbosity::ThreadGroup => "thread_group".to_string(),
            Verbosity::Tiered => "tiered".to_string(),
            Verbosity::Timestamp => "timestamp".to_string(),
            Verbosity::Transaction => "transaction".to_string(),
            Verbosity::Verify => "verify".to_string(),
            Verbosity::Version => "version".to_string(),
            Verbosity::Write => "write".to_string(),
        }
    }
}
