#![recursion_limit = "256"]
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

use std::sync::{Arc, atomic::AtomicBool};

use ::tracing::info;
use clap::Parser;
use eyre::{Report, bail, eyre};
use mimalloc::MiMalloc;
use moor_common::{build, tracing};
use moor_daemon::{
    DaemonEndpoints, DaemonKeys, DaemonPaths, DaemonRuntime, DaemonRuntimeConfig,
    VERSION_BANNER_MSG, ensure_enrollment_token, generate_keypair,
    load_or_generate_daemon_curve_keypair, rotate_enrollment_token,
};
use moor_runtime_api::load_keypair;

#[path = "../args.rs"]
mod args;
#[path = "../feature_args.rs"]
mod feature_args;

use crate::args::Args;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> Result<(), Report> {
    color_eyre::install()?;

    let args = Args::parse();
    args.db_args.storage_backend.check_available()?;
    let enrollment_token_path = args.resolved_enrollment_token_path();
    let version = semver::Version::parse(build::PKG_VERSION)
        .map_err(|e| eyre!("Invalid moor version '{}': {}", build::PKG_VERSION, e))?;

    eprintln!("Initializing...\n{VERSION_BANNER_MSG}");
    tracing::init_tracing(args.debug).map_err(|e| eyre!("Unable to configure logging: {e}"))?;

    if args.rotate_enrollment_token {
        rotate_enrollment_token(&enrollment_token_path)?;
        return Ok(());
    }

    let public_key_path = args.resolved_public_key_path();
    let private_key_path = args.resolved_private_key_path();
    prepare_config_dir()?;
    std::fs::create_dir_all(&args.data_dir)?;

    let (private_key, public_key) = if public_key_path.exists() && private_key_path.exists() {
        info!(
            "Loading existing keypair from {} / {}",
            public_key_path.display(),
            private_key_path.display()
        );
        load_keypair(&public_key_path, &private_key_path)
            .map_err(|e| eyre!("Unable to load keypair from public and private key files: {e}"))?
    } else if args.generate_keypair {
        generate_keypair(&public_key_path, &private_key_path)?;
        info!(
            "Generated keypair to {} / {}",
            public_key_path.display(),
            private_key_path.display()
        );
        load_keypair(&public_key_path, &private_key_path)
            .map_err(|e| eyre!("Unable to load generated keypair: {e}"))?
    } else {
        bail!(
            "Public ({:?}) and/or private ({:?}) key files must exist. Use --generate-keypair to create them.",
            public_key_path,
            private_key_path
        );
    };

    let daemon_curve_keypair = load_or_generate_daemon_curve_keypair(&args.data_dir)?;
    info!("Daemon CURVE keys are initialized");
    let _enrollment_token = ensure_enrollment_token(&enrollment_token_path)?;

    let config = args.load_config()?;
    let kill_switch = Arc::new(AtomicBool::new(false));
    let emergency_checkpoint = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, kill_switch.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, kill_switch.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGUSR1, emergency_checkpoint.clone())?;

    let runtime_config = DaemonRuntimeConfig {
        version,
        config,
        paths: DaemonPaths {
            data_dir: args.resolved_data_dir(),
            db_path: args.resolved_db_path(),
            connections_db_path: args.resolved_connections_db_path(),
            tasks_db_path: args.resolved_tasks_db_path(),
            events_db_path: args.resolved_events_db_path(),
        },
        endpoints: DaemonEndpoints {
            rpc_listen: args.rpc_listen.clone(),
            events_listen: args.events_listen.clone(),
            workers_request_listen: args.workers_request_listen.clone(),
            workers_response_listen: args.workers_response_listen.clone(),
            enrollment_listen: args.enrollment_listen.clone(),
        },
        keys: DaemonKeys {
            private_key,
            public_key,
            curve_keypair: daemon_curve_keypair,
            enrollment_token_path,
            allowed_hosts_dir: args.resolved_allowed_hosts_dir(),
        },
        num_io_threads: args.num_io_threads,
        workers_enabled: true,
        #[cfg(feature = "trace_events")]
        trace_output_path: args.resolved_trace_output_path(),
    };
    let runtime = DaemonRuntime {
        zmq_context: r0z::Context::new(),
        kill_switch,
        emergency_checkpoint: Some(emergency_checkpoint),
        ready_signal: None,
        local_transport: None,
        local_runtime_sender: None,
        local_worker_handler_sender: None,
    };

    moor_daemon::run(runtime_config, runtime)
}

fn prepare_config_dir() -> Result<(), Report> {
    let config_dir = moor_common::util::config_dir();
    std::fs::create_dir_all(&config_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&config_dir)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&config_dir, perms)?;
    }
    Ok(())
}
