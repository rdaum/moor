// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use moor_common::tasks::SchedulerError;
use moor_db::Database;
use moor_objdef::{collect_object_definitions, dump_object_definitions};
use tracing::{error, info, warn};

use crate::config::Config;

enum CheckpointCompletion {
    FireAndForget,
    Blocking(std::sync::mpsc::Sender<Result<(), SchedulerError>>),
}

/// Determine whether the checkpoint should block until the export has completed.
pub enum CheckpointMode {
    NonBlocking,
    Blocking,
}

/// Kick off a checkpoint operation, optionally waiting for completion.
pub fn start_checkpoint(
    database: &dyn Database,
    config: &Config,
    _version: &semver::Version,
    checkpoint_flag: Arc<AtomicBool>,
    mode: CheckpointMode,
) -> Result<(), SchedulerError> {
    if checkpoint_flag
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        warn!("Checkpoint already in progress, skipping duplicate request");
        return Ok(());
    }

    let Some(output_dir) = config.import_export.output_path.clone() else {
        checkpoint_flag.store(false, Ordering::SeqCst);
        error!("Cannot checkpoint as output directory not configured");
        return Err(SchedulerError::CouldNotStartTask);
    };

    if let Err(e) = fs::create_dir_all(&output_dir) {
        checkpoint_flag.store(false, Ordering::SeqCst);
        error!(?e, "Could not create checkpoint output directory");
        return Err(SchedulerError::CouldNotStartTask);
    }

    let checkpoint_path = output_dir.join(format!(
        "checkpoint-{}.in-progress",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    ));

    let (completion_handler, completion_receiver) = match mode {
        CheckpointMode::NonBlocking => (CheckpointCompletion::FireAndForget, None),
        CheckpointMode::Blocking => {
            let (tx, rx) = std::sync::mpsc::channel();
            (CheckpointCompletion::Blocking(tx), Some(rx))
        }
    };

    let checkpoint_flag_on_error = checkpoint_flag.clone();
    let result = database.create_snapshot_async(Box::new(move |snapshot_result| {
        let outcome = match snapshot_result {
            Ok(loader_client) => perform_export(loader_client.as_ref(), &checkpoint_path),
            Err(e) => {
                error!(?e, "Could not create snapshot for checkpoint");
                Err(SchedulerError::CouldNotStartTask)
            }
        };

        checkpoint_flag.store(false, Ordering::SeqCst);

        match completion_handler {
            CheckpointCompletion::FireAndForget => {
                if let Err(e) = &outcome {
                    error!(?e, "Checkpoint export failed");
                }
            }
            CheckpointCompletion::Blocking(ref sender) => {
                if sender.send(outcome).is_err() {
                    error!("Failed to send checkpoint completion result");
                }
            }
        }

        Ok(())
    }));

    if result.is_err() {
        checkpoint_flag_on_error.store(false, Ordering::SeqCst);
        return Err(SchedulerError::CouldNotStartTask);
    }

    if let Some(receiver) = completion_receiver {
        receiver.recv().unwrap_or_else(|_| {
            error!("Failed to receive checkpoint completion result");
            Err(SchedulerError::CouldNotStartTask)
        })
    } else {
        Ok(())
    }
}

fn perform_export(
    loader_client: &dyn moor_common::model::loader::SnapshotInterface,
    checkpoint_path: &Path,
) -> Result<(), SchedulerError> {
    info!("Collecting objects for checkpoint...");
    let objects = collect_object_definitions(loader_client).map_err(|e| {
        error!(?e, "Failed to collect objects for checkpoint");
        SchedulerError::CouldNotStartTask
    })?;
    info!("Dumping objects to {checkpoint_path:?}");
    dump_object_definitions(&objects, checkpoint_path).map_err(|e| {
        error!(?e, "Failed to dump objects");
        SchedulerError::CouldNotStartTask
    })?;
    let final_path = checkpoint_path.with_extension("moo");
    fs::rename(checkpoint_path, &final_path).map_err(|e| {
        error!(?e, "Could not rename checkpoint to final path");
        SchedulerError::CouldNotStartTask
    })?;
    info!(?final_path, "Checkpoint written.");

    Ok(())
}
