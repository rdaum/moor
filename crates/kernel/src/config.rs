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

use moor_common::threading::TaskPoolPinningMode;
use moor_db::DatabaseConfig;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub use moor_vm::FeaturesConfig;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub database: Option<DatabaseConfig>,
    pub features: Arc<FeaturesConfig>,
    pub import_export: ImportExportConfig,
    pub runtime: RuntimeConfig,
}

/// Configuration for runtime/scheduler behavior
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
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
    /// Enable/disable latency duration timing globally.
    /// Invocation counters remain exact regardless of this setting.
    pub perf_timing_enabled: Option<bool>,
    /// Sampling shift for hot-path timings (0 => exact, 6 => 1/64, 7 => 1/128).
    pub perf_timing_hot_path_shift: Option<u32>,
    /// Sampling shift for medium-path timings (0 => exact, 3 => 1/8).
    pub perf_timing_medium_path_shift: Option<u32>,
    /// Task worker affinity policy.
    pub task_pool_pinning: Option<TaskPoolPinningMode>,
    /// Reserve detected performance cores for service/control-plane threads.
    pub service_perf_cores: Option<usize>,
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
