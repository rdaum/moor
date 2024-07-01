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

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Isolation {
    ReadUncommitted,
    ReadCommitted,
    Snapshot,
}

impl Isolation {
    pub fn as_string(&self) -> &str {
        match self {
            Isolation::ReadUncommitted => "read-uncommitted",
            Isolation::ReadCommitted => "read-committed",
            Isolation::Snapshot => "snapshot",
        }
    }
}

#[derive(Clone)]
pub struct SessionConfig {
    cache_cursors: Option<bool>,
    cache_max_wait_ms: Option<i64>,
    ignore_cache_size: Option<bool>,
    isolation: Option<Isolation>,
    prefetch_enabled: Option<bool>,
}

#[allow(dead_code)]
impl Default for SessionConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl SessionConfig {
    pub fn new() -> Self {
        Self {
            cache_cursors: None,
            cache_max_wait_ms: None,
            ignore_cache_size: None,
            isolation: None,
            prefetch_enabled: None,
        }
    }

    pub fn cache_cursors(mut self, cache_cursors: bool) -> Self {
        self.cache_cursors = Some(cache_cursors);
        self
    }

    pub fn cache_max_wait_ms(mut self, cache_max_wait_ms: i64) -> Self {
        self.cache_max_wait_ms = Some(cache_max_wait_ms);
        self
    }

    pub fn ignore_cache_size(mut self, ignore_cache_size: bool) -> Self {
        self.ignore_cache_size = Some(ignore_cache_size);
        self
    }

    pub fn isolation(mut self, isolation: Isolation) -> Self {
        self.isolation = Some(isolation);
        self
    }

    pub fn prefetch_enabled(mut self, prefetch_enabled: bool) -> Self {
        self.prefetch_enabled = Some(prefetch_enabled);
        self
    }

    pub fn as_config_string(&self) -> String {
        let mut options = Vec::new();
        if let Some(cache_cursors) = &self.cache_cursors {
            options.push(format!("cache_cursors={}", cache_cursors));
        }

        if let Some(cache_max_wait_ms) = &self.cache_max_wait_ms {
            options.push(format!("cache_max_wait_ms={}", cache_max_wait_ms));
        }

        if let Some(ignore_cache_size) = &self.ignore_cache_size {
            options.push(format!("ignore_cache_size={}", ignore_cache_size));
        }

        if let Some(isolation) = &self.isolation {
            let isolation = match isolation {
                Isolation::ReadUncommitted => "read-uncommitted",
                Isolation::ReadCommitted => "read-committed",
                Isolation::Snapshot => "snapshot",
            };
            options.push(format!("isolation={}", isolation));
        }

        if let Some(prefetch_enabled) = &self.prefetch_enabled {
            options.push(format!("prefetch=(enabled={})", prefetch_enabled));
        }

        options.join(",")
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransactionConfig {
    /// The isolation level for this transaction; defaults to the session's isolation level
    isolation: Option<Isolation>,
    /// Name of the transaction for tracing and debugging
    name: Option<String>,
    /// Priority of the transaction for resolving conflicts. Transactions with higher values are less likely to abort
    priority: Option<i32>,
    /// Whether to sync log records when the transaction commits, inherited from wiredtiger_open transaction_sync
    sync: Option<bool>,
}

#[allow(dead_code)]
impl TransactionConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn isolation(mut self, isolation: Isolation) -> Self {
        self.isolation = Some(isolation);
        self
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn sync(mut self, sync: bool) -> Self {
        self.sync = Some(sync);
        self
    }

    pub fn as_config_string(&self) -> String {
        let mut options = Vec::new();
        if let Some(isolation) = &self.isolation {
            options.push(format!("isolation={}", isolation.as_string()));
        }

        if let Some(name) = &self.name {
            options.push(format!("name={}", name));
        }

        if let Some(priority) = &self.priority {
            options.push(format!("priority={}", priority));
        }

        if let Some(sync) = &self.sync {
            options.push(format!("sync={}", sync));
        }

        options.join(",")
    }
}

/*
drop	specify a list of checkpoints to drop. The list may additionally contain one of the following keys: "from=all" to drop all checkpoints, "from=<checkpoint>" to drop all checkpoints after and including the named checkpoint, or "to=<checkpoint>" to drop all checkpoints before and including the named checkpoint. Checkpoints cannot be dropped while a hot backup is in progress or if open in a cursor.	a list of strings; default empty.
force	by default, checkpoints may be skipped if the underlying object has not been modified, this option forces the checkpoint.	a boolean flag; default false.
name	if set, specify a name for the checkpoint (note that checkpoints including LSM trees may not be named).	a string; default empty.
target	if non-empty, checkpoint the list of objects.
 */

#[derive(Debug, Clone, Default)]
pub struct CheckpointConfig {
    drop: Option<Vec<String>>,
    force: Option<bool>,
    name: Option<String>,
    target: Option<String>,
}

#[allow(dead_code)]
impl CheckpointConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn drop(mut self, drop: Vec<String>) -> Self {
        self.drop = Some(drop);
        self
    }

    pub fn force(mut self, force: bool) -> Self {
        self.force = Some(force);
        self
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn target(mut self, target: String) -> Self {
        self.target = Some(target);
        self
    }

    pub fn as_config_string(&self) -> String {
        let mut options = Vec::new();
        if let Some(drop) = &self.drop {
            options.push(format!("drop={}", drop.join(",")));
        }

        if let Some(force) = &self.force {
            options.push(format!("force={}", force));
        }

        if let Some(name) = &self.name {
            options.push(format!("name={}", name));
        }

        if let Some(target) = &self.target {
            options.push(format!("target={}", target));
        }

        options.join(",")
    }
}
