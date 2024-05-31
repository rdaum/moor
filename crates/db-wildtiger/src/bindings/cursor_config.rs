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

use crate::bindings::{DumpFormat, Statistics};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CursorConfig {
    /// append the value as a new record, creating a new record number key; valid only for cursors
    /// with record number keys.
    append: Option<bool>,
    /// configure the cursor for bulk-loading, a fast, initial load path (see Bulk-load for more information).
    /// Bulk-load may only be used for newly created objects and cursors configured for bulk-load
    /// only support the WT_CURSOR::insert and WT_CURSOR::close methods.
    /// When bulk-loading row-store objects, keys must be loaded in sorted order. The value is
    /// usually a true/false flag; when bulk-loading fixed-length column store objects,
    /// the special value bitmap allows chunks of a memory resident bitmap to be loaded directly
    /// into a file by passing a WT_ITEM to WT_CURSOR::set_value where the size field indicates
    /// the number of records in the bitmap (as specified by the object's value_format configuration).
    /// Bulk-loaded bitmap values must end on a byte boundary relative to the bit count
    /// (except for the last set of values loaded).
    bulk: Option<String>,
    /// the name of a checkpoint to open (the reserved name "WiredTigerCheckpoint" opens the most
    /// recent internal checkpoint taken for the object). The cursor does not support data modification.
    checkpoint: Option<String>,
    /// configure the cursor for dump format inputs and outputs: "hex" selects a simple hexadecimal
    /// format, "json" selects a JSON format with each record formatted as fields named by column
    /// names if available, and "print" selects a format where only non-printing characters are
    /// hexadecimal encoded. These formats are compatible with the wt dump and wt load commands.
    dump: Option<DumpFormat>,
    /// configure the cursor to return a pseudo-random record from the object; valid only for
    /// row-store cursors. Cursors configured with next_random=true only support the WT_CURSOR::next
    /// and WT_CURSOR::close methods. See Cursor random for details.
    next_random: Option<bool>,
    /// configures whether the cursor's insert, update and remove methods check the existing state
    /// of the record. If overwrite is false, WT_CURSOR::insert fails with WT_DUPLICATE_KEY if the
    /// record exists, WT_CURSOR::update and WT_CURSOR::remove fail with WT_NOTFOUND if the record
    /// does not exist.
    overwrite: Option<bool>,
    /// ignore the encodings for the key and value, manage data as if the formats were "u". See
    /// Raw mode for details.
    raw: Option<bool>,
    /// only query operations are supported by this cursor. An error is returned if a modification
    /// is attempted using the cursor. The default is false for all cursor types except for log and
    /// metadata cursors.
    readonly: Option<bool>,
    /// Specify the statistics to be gathered. Choosing "all" gathers statistics regardless of
    /// cost and may include traversing on-disk files; "fast" gathers a subset of relatively
    /// inexpensive statistics. The selection must agree with the database statistics configuration
    /// specified to wiredtiger_open or WT_CONNECTION::reconfigure. For example, "all" or "fast"
    /// can be configured when the database is configured with "all", but the cursor open will fail
    /// if "all" is specified when the database is configured with "fast", and the cursor open will
    /// fail in all cases when the database is configured with "none". If statistics is not configured,
    /// the default configuration is the database configuration. The "clear" configuration resets
    /// statistics after gathering them, where appropriate (for example, a cache size statistic is
    /// not cleared, while the count of cursor insert operations will be cleared). See Statistics
    /// for more information.
    statistics: Option<Statistics>,
    /// if non-empty, backup the list of objects; valid only for a backup data source.
    target: Option<Vec<String>>,
}

#[allow(dead_code)]
impl CursorConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_option_string(&self) -> String {
        let mut options = vec![];

        if let Some(append) = &self.append {
            options.push(format!("append={}", append));
        }

        if let Some(bulk) = &self.bulk {
            options.push(format!("bulk={}", bulk));
        }

        if let Some(checkpoint) = &self.checkpoint {
            options.push(format!("checkpoint={}", checkpoint));
        }

        if let Some(dump) = &self.dump {
            options.push(format!("dump={}", dump.as_str()));
        }

        if let Some(next_random) = &self.next_random {
            options.push(format!("next_random={}", next_random));
        }

        if let Some(overwrite) = &self.overwrite {
            options.push(format!("overwrite={}", overwrite));
        }

        if let Some(true) = &self.raw {
            options.push("raw".to_string());
        }

        if let Some(readonly) = &self.readonly {
            options.push(format!("readonly={}", readonly));
        }

        if let Some(statistics) = &self.statistics {
            options.push(format!("statistics={}", statistics.as_str()));
        }

        if let Some(target) = &self.target {
            let target = target.join(",");
            options.push(format!("target={}", target));
        }

        options.join(",")
    }

    pub fn append(mut self, append: bool) -> Self {
        self.append = Some(append);
        self
    }

    pub fn bulk(mut self, bulk: String) -> Self {
        self.bulk = Some(bulk);
        self
    }

    pub fn checkpoint(mut self, checkpoint: String) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    pub fn dump(mut self, dump: DumpFormat) -> Self {
        self.dump = Some(dump);
        self
    }

    pub fn next_random(mut self, next_random: bool) -> Self {
        self.next_random = Some(next_random);
        self
    }

    pub fn overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = Some(overwrite);
        self
    }

    pub fn raw(mut self, raw: bool) -> Self {
        self.raw = Some(raw);
        self
    }

    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = Some(readonly);
        self
    }

    pub fn statistics(mut self, statistics: Statistics) -> Self {
        self.statistics = Some(statistics);
        self
    }

    pub fn target(mut self, target: Vec<String>) -> Self {
        self.target = Some(target);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CursorReconfigureOptions {
    /// append the value as a new record, creating a new record number key; valid only for cursors with record number keys.
    append: Option<bool>,
    /// configure the cursor for bulk-loading, a fast, initial load path (see Bulk-load for more information). Bulk-load may only be used for newly created objects and cursors configured for bulk-load only support the WT_CURSOR::insert and WT_CURSOR::close methods. When bulk-loading row-store objects, keys must be loaded in sorted order. The value is usually a true/false flag; when bulk-loading fixed-length column store objects, the special value bitmap allows chunks of a memory resident bitmap to be loaded directly into a file by passing a WT_ITEM to WT_CURSOR::set_value where the size field indicates the number of records in the bitmap (as specified by the object's value_format configuration). Bulk-loaded bitmap values must end on a byte boundary relative to the bit count (except for the last set of values loaded).
    overwrite: Option<bool>,
}

#[allow(dead_code)]
impl CursorReconfigureOptions {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn as_config_string(&self) -> String {
        let mut options = vec![];

        if let Some(append) = &self.append {
            options.push(format!("append={}", append));
        }

        if let Some(overwrite) = &self.overwrite {
            options.push(format!("overwrite={}", overwrite));
        }

        options.join(",")
    }

    pub fn append(mut self, append: bool) -> Self {
        self.append = Some(append);
        self
    }

    pub fn overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = Some(overwrite);
        self
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum Bounds {
    Lower,
    Upper,
}

impl Bounds {
    pub fn as_str(&self) -> &str {
        match self {
            Bounds::Lower => "lower",
            Bounds::Upper => "upper",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum BoundsAction {
    Clear,
    Set,
}

impl BoundsAction {
    pub fn as_str(&self) -> &str {
        match self {
            BoundsAction::Clear => "clear",
            BoundsAction::Set => "set",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoundsConfig {
    /// configures whether this call into the API will set or clear range bounds on the given cursor.
    action: Option<BoundsAction>,
    /// configures which bound is being operated on.
    bound: Option<Bounds>,
    /// configures whether the given bound is inclusive or not.
    inclusive: Option<bool>,
}

#[allow(dead_code)]
impl BoundsConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn as_config_string(&self) -> String {
        let mut options = vec![];

        if let Some(action) = &self.action {
            options.push(format!("action={}", action.as_str()));
        }

        if let Some(bound) = &self.bound {
            options.push(format!("bound={}", bound.as_str()));
        }

        if let Some(inclusive) = &self.inclusive {
            options.push(format!("inclusive={}", inclusive));
        }

        options.join(",")
    }

    pub fn action(mut self, action: BoundsAction) -> Self {
        self.action = Some(action);
        self
    }

    pub fn bound(mut self, bound: Bounds) -> Self {
        self.bound = Some(bound);
        self
    }

    pub fn inclusive(mut self, inclusive: bool) -> Self {
        self.inclusive = Some(inclusive);
        self
    }
}
