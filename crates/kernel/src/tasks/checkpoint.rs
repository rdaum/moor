use std::{
    fs::{self, File},
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
use moor_textdump::{TextdumpWriter, make_textdump};
use tracing::{error, info, warn};

use crate::config::{Config, ImportExportFormat};

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
    version: &semver::Version,
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

    let Some(textdump_dir) = config.import_export.output_path.clone() else {
        checkpoint_flag.store(false, Ordering::SeqCst);
        error!("Cannot textdump as output directory not configured");
        return Err(SchedulerError::CouldNotStartTask);
    };

    if let Err(e) = fs::create_dir_all(&textdump_dir) {
        checkpoint_flag.store(false, Ordering::SeqCst);
        error!(?e, "Could not create textdump directory");
        return Err(SchedulerError::CouldNotStartTask);
    }

    let textdump_path = textdump_dir.join(format!(
        "textdump-{}.in-progress",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    ));

    let encoding_mode = config.import_export.output_encoding;
    let version_string = config
        .import_export
        .version_string(version, &config.features);
    let dirdump = config.import_export.export_format == ImportExportFormat::Objdef;

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
            Ok(loader_client) => perform_export(
                loader_client.as_ref(),
                &textdump_path,
                dirdump,
                &version_string,
                encoding_mode,
            ),
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
    textdump_path: &Path,
    dirdump: bool,
    version_string: &str,
    encoding_mode: moor_textdump::EncodingMode,
) -> Result<(), SchedulerError> {
    if dirdump {
        info!("Collecting objects for dump...");
        let objects = collect_object_definitions(loader_client).map_err(|e| {
            error!(?e, "Failed to collect objects for dump");
            SchedulerError::CouldNotStartTask
        })?;
        info!("Dumping objects to {textdump_path:?}");
        dump_object_definitions(&objects, textdump_path).map_err(|e| {
            error!(?e, "Failed to dump objects");
            SchedulerError::CouldNotStartTask
        })?;
        let final_path = textdump_path.with_extension("moo");
        fs::rename(textdump_path, &final_path).map_err(|e| {
            error!(?e, "Could not rename objdefdump to final path");
            SchedulerError::CouldNotStartTask
        })?;
        info!(?final_path, "Objdefdump written.");
    } else {
        let mut output = File::create(textdump_path).map_err(|e| {
            error!(?e, "Could not open textdump file for writing");
            SchedulerError::CouldNotStartTask
        })?;

        let textdump = make_textdump(loader_client, version_string.to_string());

        let mut writer = TextdumpWriter::new(&mut output, encoding_mode);
        writer.write_textdump(&textdump).map_err(|e| {
            error!(?e, "Could not write textdump");
            SchedulerError::CouldNotStartTask
        })?;

        let final_path = textdump_path.with_extension("moo-textdump");
        fs::rename(textdump_path, &final_path).map_err(|e| {
            error!(?e, "Could not rename textdump to final path");
            SchedulerError::CouldNotStartTask
        })?;
        info!(?final_path, "Textdump written.");
    }

    Ok(())
}
