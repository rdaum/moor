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

use crate::feature_args::FeatureArgs;
use clap::builder::ValueHint;
use clap_derive::{Parser, ValueEnum};
use eyre::eyre;
use figment::Figment;
use figment::providers::{Format as ProviderFormat, Serialized, Yaml};
use moor_db::DatabaseConfig;
use moor_kernel::config::{Config, ImportExportConfig, ImportExportFormat, RuntimeConfig};
use moor_textdump::EncodingMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[allow(dead_code)]
#[derive(Parser, Debug, Serialize, Deserialize)] // requires `derive` feature
pub struct Args {
    #[arg(
        value_name = "data-dir",
        help = "Directory to store all database files under",
        value_hint = ValueHint::DirPath,
        default_value = "./moor-data"
    )]
    pub data_dir: PathBuf,

    #[command(flatten)]
    pub db_args: DatabaseArgs,

    #[command(flatten)]
    import_export_args: Option<ImportExportArgs>,

    #[command(flatten)]
    runtime_args: Option<RuntimeArgs>,

    #[command(flatten)]
    feature_args: Option<FeatureArgs>,

    #[arg(
        long,
        value_name = "config",
        help = "Path to configuration (YAML) file to use, if any. If not specified, defaults are used.\
                Configuration file values can be overridden by command line arguments.",
        value_hint = ValueHint::FilePath
    )]
    pub config_file: Option<PathBuf>,

    #[arg(
        short,
        long,
        value_name = "connections-db",
        help = "Path to connections database to use or create (relative to data-dir if not absolute)",
        value_hint = ValueHint::FilePath
    )]
    pub connections_file: Option<PathBuf>,

    #[arg(
        short = 'x',
        long,
        value_name = "tasks-db",
        help = "Path to persistent tasks database to use or create (relative to data-dir if not absolute)",
        value_hint = ValueHint::FilePath
    )]
    pub tasks_db: Option<PathBuf>,

    #[arg(
        short = 'e',
        long,
        value_name = "events-db",
        help = "Path to persistent events database to use or create (relative to data-dir if not absolute)",
        value_hint = ValueHint::FilePath
    )]
    pub events_db: Option<PathBuf>,

    #[arg(
        long,
        value_name = "rpc-listen",
        help = "RPC server address",
        default_value = "ipc:///tmp/moor_rpc.sock"
    )]
    pub rpc_listen: String,

    #[arg(
        long,
        value_name = "events-listen",
        help = "Events publisher listen address",
        default_value = "ipc:///tmp/moor_events.sock"
    )]
    pub events_listen: String,

    #[arg(
        long,
        value_name = "workers-response-listen",
        help = "Workers server RPC address for receiving attachment, responses, and pings etc",
        default_value = "ipc:///tmp/moor_workers_response.sock"
    )]
    pub workers_response_listen: String,

    #[arg(
        long,
        value_name = "workers-request-listen",
        help = "Workers server pub-sub address for broadcasting dispatch requests",
        default_value = "ipc:///tmp/moor_workers_request.sock"
    )]
    pub workers_request_listen: String,

    #[arg(
        long,
        value_name = "public_key",
        help = "file containing the PEM encoded public key (shared with the daemon), used for authenticating client & host connections",
        default_value = "moor-verifying-key.pem"
    )]
    pub public_key: PathBuf,

    #[arg(
        long,
        value_name = "private_key",
        help = "file containing an openssh generated ed25519 format private key (shared with the daemon), used for authenticating client & host connections",
        default_value = "moor-signing-key.pem"
    )]
    pub private_key: PathBuf,

    #[arg(
        long,
        value_name = "num-io-threads",
        help = "Number of ZeroMQ IO threads to use",
        default_value = "8"
    )]
    pub num_io_threads: i32,

    #[arg(long, help = "Enable debug logging", default_value = "false")]
    pub debug: bool,

    #[arg(long, help = "Generate ED25519 keypair and exit")]
    pub generate_keypair: bool,

    #[cfg(feature = "trace_events")]
    #[arg(
        long,
        value_name = "trace-output",
        help = "Path to output Chrome trace events JSON file. If not specified, tracing is disabled.",
        value_hint = ValueHint::FilePath
    )]
    pub trace_output: Option<PathBuf>,
}

/// Formats for import or export
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Default, Serialize, Deserialize,
)]
pub enum Format {
    /// Traditional LambdaMOO textdump format
    #[default]
    Textdump,
    /// New-style objdef/dirdump format
    Objdef,
}

#[allow(dead_code)]
#[derive(Parser, Debug, Serialize, Deserialize)]
pub struct ImportExportArgs {
    #[arg(short, long, value_name = "import", help = "Path to a textdump or objdef directory to import", value_hint = ValueHint::FilePath)]
    pub import: Option<PathBuf>,

    #[arg(
        long,
        value_name = "export",
        help = "Path to a textdump or objdef directory to export into",
        value_hint = ValueHint::FilePath
    )]
    pub export: Option<PathBuf>,

    #[arg(
        long,
        value_name = "import-format",
        help = "Format to import from.",
        value_enum,
        default_value_t = Format::Textdump
    )]
    pub import_format: Format,

    #[arg(
        long,
        value_name = "export-format",
        help = "Format to export into.",
        value_enum,
        default_value_t = Format::Objdef
    )]
    pub export_format: Format,

    #[arg(
        long,
        value_name = "checkpoint-interval-seconds",
        help = "Interval in seconds between database checkpoints"
    )]
    pub checkpoint_interval_seconds: Option<u16>,

    #[arg(
        long,
        value_name = "textdump-output-encoding",
        help = "Encoding to use for writing textdump files. utf8 or iso8859-1. \
          LambdaMOO textdumps that contain 8-bit strings are written using iso8859-1, so if you want to write a LambdaMOO-compatible textdump, choose iso8859-1. \
          (But make sure your features are set to match LambdaMOO's capabilities!)"
    )]
    pub textdump_output_encoding: Option<EncodingMode>,

    #[arg(
        long,
        value_name = "textdump-version-override",
        help = "Version override string to put into the textdump. \
          If None, the moor version + a serialization of the features config is used + the encoding. \
          If set, this string will be used instead. \
          This is useful for producing textdumps that are compatible with other servers, but be \
          careful to not lie about the features (and encoding) you support."
    )]
    pub version_override: Option<String>,
}

impl ImportExportArgs {
    pub fn merge_config(&self, config: &mut ImportExportConfig) -> Result<(), eyre::Report> {
        if let Some(args) = self.import.as_ref() {
            config.input_path = Some(args.clone());
        }
        if let Some(args) = self.export.as_ref() {
            config.output_path = Some(args.clone());
        }
        if let Some(args) = self.textdump_output_encoding {
            config.output_encoding = args;
        }
        if let Some(args) = self.checkpoint_interval_seconds {
            config.checkpoint_interval = Some(std::time::Duration::from_secs(u64::from(args)));
        }
        if let Some(args) = self.version_override.as_ref() {
            config.version_override = Some(args.clone());
        }
        config.import_format = match self.import_format {
            Format::Textdump => ImportExportFormat::Textdump,
            Format::Objdef => ImportExportFormat::Objdef,
        };
        config.export_format = match self.export_format {
            Format::Textdump => ImportExportFormat::Textdump,
            Format::Objdef => ImportExportFormat::Objdef,
        };
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Parser, Debug, Serialize, Deserialize)]
pub struct RuntimeArgs {
    #[arg(
        long,
        value_name = "gc-interval-seconds",
        help = "Interval in seconds for automatic garbage collection (default: 30)"
    )]
    pub gc_interval_seconds: Option<u16>,
}

impl RuntimeArgs {
    pub fn merge_config(&self, config: &mut RuntimeConfig) -> Result<(), eyre::Report> {
        if let Some(args) = self.gc_interval_seconds {
            config.gc_interval = Some(std::time::Duration::from_secs(u64::from(args)));
        }
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Parser, Debug, Serialize, Deserialize)]
pub struct DatabaseArgs {
    #[arg(
        long,
        value_name = "db",
        help = "Main database filename (relative to data-dir if not absolute)",
        value_hint = ValueHint::FilePath,
        default_value = "world.db"
    )]
    pub db: PathBuf,
}

impl DatabaseArgs {
    pub(crate) fn merge_config(&self, _db_config: &mut DatabaseConfig) -> Result<(), eyre::Report> {
        // Noop for now
        Ok(())
    }
}

impl Args {
    #[allow(dead_code)]
    fn merge_config(&self, mut config: Config) -> Result<Config, eyre::Report> {
        if let Some(args) = self.import_export_args.as_ref() {
            args.merge_config(&mut config.import_export)?;
        }
        if let Some(args) = self.runtime_args.as_ref() {
            args.merge_config(&mut config.runtime)?;
        }
        if let Some(args) = self.feature_args.as_ref() {
            let mut copy = config.features.as_ref().clone();
            args.merge_config(&mut copy)?;
            config.features = Arc::new(copy);
        }
        if let Some(database_config) = config.database.as_mut() {
            self.db_args.merge_config(database_config)?;
        }

        Ok(config)
    }

    /// Load the configuration file if we have it, and then we'll merge the arguments into it.
    pub fn load_config(&self) -> Result<Arc<Config>, eyre::Report> {
        // We use figment to load, but cannot use its merge functionality because our clap enums are
        // nested using flattening, and figment doesn't support that.
        let config_path = self.config_file.clone();
        let config = Config::default();
        let config = config_path
            .map(|config_path| {
                let f = Figment::new()
                    .merge(Serialized::defaults(config))
                    .merge(Yaml::file(config_path.clone()));

                f.extract::<Config>().map_err(|e| {
                    eyre!(
                        "Failed to parse configuration from {:?}: {}",
                        config_path,
                        e
                    )
                })
            })
            .unwrap_or_else(|| Ok(Config::default()))?;
        let config = self.merge_config(config)?;
        let config = Arc::new(config);
        Ok(config)
    }
}

// Helper functions for resolving database paths relative to data_dir
impl Args {
    /// Resolve the main database path relative to data_dir
    pub(crate) fn resolved_db_path(&self) -> PathBuf {
        if self.db_args.db.is_absolute() {
            self.db_args.db.clone()
        } else {
            self.data_dir.join(&self.db_args.db)
        }
    }

    /// Resolve the tasks database path relative to data_dir
    pub(crate) fn resolved_tasks_db_path(&self) -> PathBuf {
        match &self.tasks_db {
            Some(path) => {
                if path.is_absolute() {
                    path.clone()
                } else {
                    self.data_dir.join(path)
                }
            }
            None => self.data_dir.join("tasks.db"),
        }
    }

    /// Resolve the connections database path relative to data_dir
    pub(crate) fn resolved_connections_db_path(&self) -> Option<PathBuf> {
        match &self.connections_file {
            Some(path) => {
                if path.is_absolute() {
                    Some(path.clone())
                } else {
                    Some(self.data_dir.join(path))
                }
            }
            None => Some(self.data_dir.join("connections.db")),
        }
    }

    /// Resolve the events database path relative to data_dir
    pub(crate) fn resolved_events_db_path(&self) -> PathBuf {
        match &self.events_db {
            Some(path) => {
                if path.is_absolute() {
                    path.clone()
                } else {
                    self.data_dir.join(path)
                }
            }
            None => self.data_dir.join("events.db"),
        }
    }

    #[cfg(feature = "trace_events")]
    /// Resolve the trace output path relative to data_dir
    pub(crate) fn resolved_trace_output_path(&self) -> Option<PathBuf> {
        self.trace_output.as_ref().map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                self.data_dir.join(path)
            }
        })
    }
}
