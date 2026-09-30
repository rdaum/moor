// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Config is created by the host daemon, and passed through the scheduler, whereupon it is
//! available to all components. Used to hold things typically configured by CLI flags, etc.

use moor_common::{config::MAX_CAPTURE_DEADLINE, threading::TaskPoolPinningMode};
use moor_db::DatabaseConfig;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub use moor_vm::FeaturesConfig;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub storage: moor_db::StorageSettings,
    pub database: Option<DatabaseConfig>,
    pub features: Arc<FeaturesConfig>,
    pub import_export: ImportExportConfig,
    pub runtime: RuntimeConfig,
}

/// Configuration for runtime/scheduler behavior
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    /// Interval between automatic garbage collection cycles.
    /// If None, automatic GC uses database settings or default.
    #[serde(deserialize_with = "parse_duration")]
    pub gc_interval: Option<Duration>,
    /// Scheduler tick interval - how often the scheduler wakes to check for events.
    /// Lower values provide better latency but higher CPU usage.
    /// If None, defaults to 10ms.
    #[serde(deserialize_with = "parse_duration")]
    pub scheduler_tick_duration: Option<Duration>,
    /// Whether perf timing is enabled. If None, defaults to true.
    pub perf_timing_enabled: Option<bool>,
    /// Sampling shift for hot-path timings (0 => exact, 6 => 1/64).
    pub perf_timing_hot_path_shift: Option<u32>,
    /// Task worker affinity policy.
    pub task_pool_pinning: Option<TaskPoolPinningMode>,
    /// Reserve detected performance cores for service/control-plane threads.
    pub service_perf_cores: Option<usize>,
    /// Longest deadline a client may ask for when it waits for a verb call's captured output.
    /// A request asking for more than this is rejected. If None, defaults to 60s.
    #[serde(deserialize_with = "parse_duration")]
    pub max_capture_deadline: Option<Duration>,
}

/// Deadline used for captured verb calls when the configuration does not set one.
pub const DEFAULT_MAX_CAPTURE_DEADLINE: Duration = Duration::from_secs(60);

impl RuntimeConfig {
    /// Longest deadline a captured verb call may wait for a result.
    #[must_use]
    pub fn max_capture_deadline(&self) -> Duration {
        self.max_capture_deadline
            .unwrap_or(DEFAULT_MAX_CAPTURE_DEADLINE)
    }

    /// Check settings that only make sense within a range, whatever source they came from.
    ///
    /// Each source merges into the config independently, so a value can only be judged once they
    /// have all been applied. Callers should run this after the last merge.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(deadline) = self.max_capture_deadline {
            if deadline.is_zero() {
                return Err("runtime.max_capture_deadline must be greater than zero".to_string());
            }
            if deadline > MAX_CAPTURE_DEADLINE {
                return Err(format!(
                    "runtime.max_capture_deadline of {}s exceeds the protocol maximum of {}s",
                    deadline.as_secs(),
                    MAX_CAPTURE_DEADLINE.as_secs()
                ));
            }
        }

        Ok(())
    }
}

impl Config {
    /// Check settings that only make sense within a range, whatever source they came from.
    pub fn validate(&self) -> Result<(), String> {
        self.import_export.validate()?;
        self.runtime.validate()
    }
}

/// Format for importing databases.
#[derive(Clone, Debug, Serialize, Deserialize, Default, Eq, PartialEq)]
pub enum ImportFormat {
    /// The legacy LambdaMOO textdump format.
    #[default]
    Textdump,
    /// The new-style directory based objectdef format.
    Objdef,
}

/// Configuration for database import and checkpoint export.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ImportExportConfig {
    /// Where to read the initial import from, if any.
    pub input_path: Option<PathBuf>,
    /// Directory to write periodic checkpoint exports of the database, if any.
    /// Checkpoints are always written in objdef format.
    pub output_path: Option<PathBuf>,
    /// Interval between database checkpoints.
    /// If None, no checkpoints will be made.
    #[serde(deserialize_with = "parse_duration")]
    pub checkpoint_interval: Option<Duration>,
    /// Which format to use for import.
    pub import_format: ImportFormat,
    /// Local remote-tracking ref used to find the Git ancestor for initial objdef import.
    pub git_upstream: Option<String>,
    /// Explicit upstream baseline source for initial objdef import, without Git.
    pub baseline_objdef_dir: Option<PathBuf>,
}

impl ImportExportConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.git_upstream.is_some() && self.baseline_objdef_dir.is_some() {
            return Err("choose either git_upstream or baseline_objdef_dir for import".into());
        }
        if (self.git_upstream.is_some() || self.baseline_objdef_dir.is_some())
            && (self.input_path.is_none() || self.import_format != ImportFormat::Objdef)
        {
            return Err(
                "baseline preparation requires an import path and objdef import format".into(),
            );
        }
        Ok(())
    }
}

// Use humantime to parse durations from strings
fn parse_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => humantime::parse_duration(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}
