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

use clap::builder::ValueHint;
use clap_derive::Parser;
use moor_db::DatabaseConfig;
use moor_kernel::config::{Config, FeaturesConfig, TextdumpConfig};
use moor_kernel::textdump::EncodingMode;
use std::path::PathBuf;
use std::time::Duration;

#[allow(dead_code)]
#[derive(Parser, Debug)] // requires `derive` feature
pub struct Args {
    #[command(flatten)]
    pub db_args: DatabaseArgs,

    #[command(flatten)]
    textdump_args: Option<TextdumpArgs>,

    #[command(flatten)]
    feature_args: Option<FeatureArgs>,

    #[arg(
        long,
        value_name = "write-merged-config",
        help = "If set, this is a path to write the current configuration (with merged values from command line arguments), in JSON format",
        value_hint = ValueHint::FilePath
    )]
    pub write_merged_config: Option<PathBuf>,

    #[arg(
        long,
        value_name = "config",
        help = "Path to configuration (json) file to use, if any. If not specified, defaults are used.\
                Configuration file values can be overridden by command line arguments.",
        value_hint = ValueHint::FilePath
    )]
    pub config_file: Option<PathBuf>,

    #[arg(
        short,
        long,
        value_name = "connections-db",
        help = "Path to connections database to use or create",
        value_hint = ValueHint::FilePath,
        default_value = "connections.db"
    )]
    pub connections_file: PathBuf,

    #[arg(
        short = 'x',
        long,
        value_name = "tasks-db",
        help = "Path to persistent tasks database to use or create",
        value_hint = ValueHint::FilePath,
        default_value = "tasks.db"
    )]
    pub tasks_db: PathBuf,

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
}

#[derive(Parser, Debug)]
pub struct FeatureArgs {
    /// Whether to allow notify() to send arbitrary MOO common to players. The interpretation of
    /// the common varies depending on host/client.
    /// If this is false, only strings are allowed, as in LambdaMOO.
    #[arg(
        long,
        help = "Enable rich_notify, allowing notify() to send arbitrary MOO common to players. \
                The interpretation of the common varies depending on host/client. \
                If this is false, only strings are allowed, as in LambdaMOO."
    )]
    pub rich_notify: Option<bool>,

    #[arg(
        long,
        help = "Enable block-level lexical scoping in programs. \
                Adds the `begin`/`end` syntax for creating lexical scopes, and `let` and `global`
                for declaring variables. \
                This is a feature that is not present in LambdaMOO, so if you need backwards compatibility, turn this off."
    )]
    pub lexical_scopes: Option<bool>,

    #[arg(
        long,
        help = "Enable the Map datatype ([ k -> v, .. ]) compatible with Stunt/ToastStunt"
    )]
    pub map_type: Option<bool>,

    #[arg(
        long,
        help = "Enable primitive-type verb dispatching. E.g. \"test\":reverse() becomes $string:reverse(\"test\")"
    )]
    pub type_dispatch: Option<bool>,

    #[arg(
        long,
        help = "Enable flyweight types. Flyweights are a lightweight, object delegate"
    )]
    pub flyweight_type: Option<bool>,

    #[arg(long, help = "Enable boolean true/false literals and a boolean type")]
    pub bool_type: Option<bool>,

    #[arg(
        long,
        help = "Whether to have builtins that return truth values return boolean types instead of integer 1 or 0. Same goes for binary value operators like <, !, ==, <= etc."
    )]
    pub use_boolean_returns: Option<bool>,

    #[arg(long, help = "Enable 'symbol literals")]
    pub symbol_type: Option<bool>,

    #[arg(
        long,
        help = "Enable error symbols beyond the standard builtin set, with no integer conversions for them."
    )]
    pub custom_errors: Option<bool>,

    #[arg(
        long,
        help = "Whether to have certain builtins use or return symbols instead of strings for things like property names, etc."
    )]
    pub use_symbols_in_builtins: Option<bool>,

    #[arg(
        long,
        help = "Enable support for list / range comprehensions in the language"
    )]
    pub list_comprehensions: Option<bool>,

    #[arg(
        long,
        help = "Enable persistent tasks, which persist the state of suspended/forked tasks between restarts. \
                Note that this is the default behaviour in LambdaMOO."
    )]
    pub persistent_tasks: Option<bool>,
}

impl FeatureArgs {
    pub fn merge_config(&self, config: &mut FeaturesConfig) {
        if let Some(args) = self.rich_notify {
            config.rich_notify = args;
        }
        if let Some(args) = self.lexical_scopes {
            config.lexical_scopes = args;
        }
        if let Some(args) = self.map_type {
            config.map_type = args;
        }
        if let Some(args) = self.type_dispatch {
            config.type_dispatch = args;
        }
        if let Some(args) = self.flyweight_type {
            config.flyweight_type = args;
        }
        if let Some(args) = self.bool_type {
            config.bool_type = args;
        }
        if let Some(args) = self.use_boolean_returns {
            config.use_boolean_returns = args;
        }
        if let Some(args) = self.custom_errors {
            config.custom_errors = args;
        }
        if let Some(args) = self.symbol_type {
            config.symbol_type = args;
        }
        if let Some(args) = self.use_symbols_in_builtins {
            config.use_symbols_in_builtins = args;
        }
        if let Some(args) = self.persistent_tasks {
            config.persistent_tasks = args;
        }
        if let Some(args) = self.list_comprehensions {
            config.list_comprehensions = args;
        }
    }
}

#[allow(dead_code)]
#[derive(Parser, Debug)]
pub struct TextdumpArgs {
    #[arg(short, long, value_name = "textdump", help = "Path to the textdump to import", value_hint = ValueHint::FilePath)]
    pub textdump: Option<PathBuf>,

    #[arg(
        long,
        help = "Use the new-style \"dirdump\" importer to load object definition files from the textdump directory instead of the legacy LambdaMOO format.",
        default_value = "false"
    )]
    pub import_dirdump_format: bool,

    #[arg(
        long,
        help = "Use the new-style \"dirdump\" importer to dump object definition files into textdump directory instead of the legacy LambdaMOO format.",
        default_value = "false"
    )]
    pub export_dirdump_format: bool,

    #[arg(
        long,
        value_name = "checkpoint-interval-seconds",
        help = "Interval in seconds between database checkpoints"
    )]
    pub checkpoint_interval_seconds: Option<u16>,

    #[arg(
        long,
        value_name = "textdump-output",
        help = "Path to directory to write the textdump file to on `dump_database()`, or on schedule, if any"
    )]
    pub textdump_out: Option<PathBuf>,

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

impl TextdumpArgs {
    pub fn merge_config(&self, config: &mut TextdumpConfig) {
        if let Some(args) = self.textdump.as_ref() {
            config.input_path = Some(args.clone());
        }
        if let Some(args) = self.textdump_out.as_ref() {
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
        config.import_dirdump = self.import_dirdump_format;
        config.export_dirdump = self.export_dirdump_format;
    }
}

#[allow(dead_code)]
#[derive(Parser, Debug)]
pub struct DatabaseArgs {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    pub db: PathBuf,

    #[arg(
        long,
        value_name = "cache-eviction-interval-seconds",
        help = "Rate to run cache eviction cycles at, in seconds"
    )]
    pub cache_eviction_interval: Option<u64>,

    #[arg(
        long,
        value_name = "default-eviction-threshold",
        help = "The default eviction threshold for each transaction-global cache. If a value is not specified \
          for a specific table, this value will be used. \
          Every `cache_eviction_interval` seconds, the total memory usage of the cache will be checked, \
          and if it exceeds this threshold, random entries will be put onto the eviction queue. \
          If they are still there, untouched, by the next eviction cycle, they will be removed."
    )]
    pub default_eviction_threshold: Option<usize>,
    // TODO: per table options
}

impl DatabaseArgs {
    #[allow(dead_code)]
    pub fn merge_config(&self, config: &mut DatabaseConfig) {
        if let Some(args) = self.cache_eviction_interval {
            config.cache_eviction_interval = Duration::from_secs(args);
        }
        if let Some(args) = self.default_eviction_threshold {
            config.default_eviction_threshold = args;
        }
    }
}

impl Args {
    #[allow(dead_code)]
    pub fn merge_config(&self, mut config: Config) -> Config {
        if let Some(args) = self.textdump_args.as_ref() {
            args.merge_config(&mut config.textdump_config);
        }
        if let Some(args) = self.feature_args.as_ref() {
            args.merge_config(&mut config.features_config);
        }
        self.db_args.merge_config(&mut config.database_config);

        config
    }
}
