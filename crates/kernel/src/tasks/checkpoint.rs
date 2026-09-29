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

use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use moor_common::tasks::SchedulerError;
use moor_db::Database;
use moor_objdef::dump_snapshot_object_definitions;
use parking_lot::Mutex;
use semver::Version;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    config::Config,
    tasks::maintenance::{MaintenanceCoordinator, MaintenanceKind, MaintenanceTicket},
};

type CompletionCallback = Box<dyn FnOnce(u64, Result<(), SchedulerError>) + Send + 'static>;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CheckpointManifest {
    format_version: Version,
    snapshot_epoch: u64,
    export_completion_epoch: u64,
    scan_duration_millis: u64,
    write_duration_millis: u64,
    object_count: u64,
    verb_count: u64,
    property_count: u64,
    override_count: u64,
}

pub(crate) type CheckpointTicket = MaintenanceTicket;

/// An admitted checkpoint which has not yet been handed to the database.
///
/// Separating admission from launch lets a blocking MOO task enter the suspended
/// queue before an unusually fast snapshot callback can complete.
pub(crate) struct CheckpointJob {
    coordinator: MaintenanceCoordinator,
    ticket: CheckpointTicket,
    snapshot_epoch: u64,
    checkpoint_path: PathBuf,
    launched: bool,
}

impl CheckpointJob {
    #[inline]
    pub(crate) fn generation(&self) -> u64 {
        self.ticket.generation()
    }

    /// Start the snapshot and export. Completion is delivered exactly once.
    pub(crate) fn launch(
        mut self,
        database: &dyn Database,
        callback: CompletionCallback,
    ) -> Result<CheckpointTicket, SchedulerError> {
        let ticket = self.ticket.clone();
        let callback = Arc::new(Mutex::new(Some(callback)));
        let callback_for_snapshot = callback.clone();
        let coordinator = self.coordinator.clone();
        let coordinator_on_error = self.coordinator.clone();
        let ticket_for_snapshot = ticket.clone();
        let ticket_on_error = ticket.clone();
        let snapshot_epoch = self.snapshot_epoch;
        let checkpoint_path = self.checkpoint_path.clone();

        self.launched = true;
        let output_dir = checkpoint_path
            .parent()
            .expect("Checkpoint path should have an output directory");
        if let Err(e) = fs::create_dir_all(output_dir) {
            error!(?e, "Could not create checkpoint output directory");
            let outcome = Err(SchedulerError::CouldNotStartTask);
            finish_checkpoint(
                &coordinator_on_error,
                &ticket_on_error,
                outcome.clone(),
                &callback,
            );
            return outcome.map(|()| ticket);
        }

        let result = database.create_snapshot_async(Box::new(move |snapshot_result| {
            let outcome = match snapshot_result {
                Ok(loader_client) => {
                    perform_export(loader_client.as_ref(), &checkpoint_path, snapshot_epoch)
                }
                Err(e) => {
                    error!(?e, "Could not create snapshot for checkpoint");
                    Err(SchedulerError::CouldNotStartTask)
                }
            };

            finish_checkpoint(
                &coordinator,
                &ticket_for_snapshot,
                outcome,
                &callback_for_snapshot,
            );
            Ok(())
        }));

        if let Err(e) = result {
            error!(?e, "Could not start checkpoint snapshot");
            let outcome = Err(SchedulerError::CouldNotStartTask);
            finish_checkpoint(
                &coordinator_on_error,
                &ticket_on_error,
                outcome.clone(),
                &callback,
            );
            return outcome.map(|()| ticket);
        }

        Ok(ticket)
    }
}

impl Drop for CheckpointJob {
    fn drop(&mut self) {
        if self.launched {
            return;
        }
        self.coordinator
            .complete(&self.ticket, Err(SchedulerError::CouldNotStartTask));
    }
}

fn finish_checkpoint(
    coordinator: &MaintenanceCoordinator,
    ticket: &CheckpointTicket,
    outcome: Result<(), SchedulerError>,
    callback: &Mutex<Option<CompletionCallback>>,
) {
    coordinator.complete(ticket, outcome.clone());
    if let Some(callback) = callback.lock().take() {
        callback(ticket.generation(), outcome);
    }
}

/// Reserve a checkpoint generation and its output path.
pub(crate) fn prepare_checkpoint(
    config: &Config,
    coordinator: &MaintenanceCoordinator,
) -> Result<CheckpointJob, SchedulerError> {
    let ticket = coordinator.admit(MaintenanceKind::Checkpoint)?;

    let Some(output_dir) = config.import_export.output_path.clone() else {
        error!("Cannot checkpoint as output directory not configured");
        coordinator.complete(&ticket, Err(SchedulerError::CouldNotStartTask));
        return Err(SchedulerError::CouldNotStartTask);
    };

    let snapshot_epoch = unix_epoch_seconds();
    let checkpoint_path = output_dir.join(format!("checkpoint-{snapshot_epoch}.in-progress"));

    Ok(CheckpointJob {
        coordinator: coordinator.clone(),
        ticket,
        snapshot_epoch,
        checkpoint_path,
        launched: false,
    })
}

fn unix_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

fn duration_millis(duration: std::time::Duration) -> u64 {
    duration
        .as_millis()
        .try_into()
        .expect("Checkpoint phase duration exceeds u64 milliseconds")
}

fn write_manifest(
    checkpoint_path: &Path,
    manifest: &CheckpointManifest,
) -> Result<(), SchedulerError> {
    let manifest_path = checkpoint_path.join("manifest.json");
    let file = File::create(&manifest_path).map_err(|e| {
        error!(?e, ?manifest_path, "Could not create checkpoint manifest");
        SchedulerError::CouldNotStartTask
    })?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, manifest).map_err(|e| {
        error!(
            ?e,
            ?manifest_path,
            "Could not serialize checkpoint manifest"
        );
        SchedulerError::CouldNotStartTask
    })?;
    writeln!(writer).and_then(|()| writer.flush()).map_err(|e| {
        error!(?e, ?manifest_path, "Could not write checkpoint manifest");
        SchedulerError::CouldNotStartTask
    })
}

fn perform_export(
    loader_client: &dyn moor_common::model::loader::SnapshotInterface,
    checkpoint_path: &Path,
    snapshot_epoch: u64,
) -> Result<(), SchedulerError> {
    info!("Exporting snapshot to {checkpoint_path:?}");
    let stats = dump_snapshot_object_definitions(loader_client, checkpoint_path).map_err(|e| {
        error!(error = %e, "Failed to dump objects");
        SchedulerError::CouldNotStartTask
    })?;
    let manifest = CheckpointManifest {
        format_version: Version::new(2, 0, 0),
        snapshot_epoch,
        export_completion_epoch: unix_epoch_seconds(),
        scan_duration_millis: duration_millis(stats.metadata_elapsed),
        write_duration_millis: duration_millis(stats.write_elapsed),
        object_count: stats.objects as u64,
        verb_count: stats.verbs as u64,
        property_count: stats.properties as u64,
        override_count: stats.overrides as u64,
    };
    write_manifest(checkpoint_path, &manifest)?;
    let final_path = checkpoint_path.with_extension("moo");
    fs::rename(checkpoint_path, &final_path).map_err(|e| {
        error!(?e, "Could not rename checkpoint to final path");
        SchedulerError::CouldNotStartTask
    })?;
    info!(?final_path, ?stats, "Checkpoint written.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::{
        model::{ObjectKind, PropFlag, TaskPermissions, VerbArgsSpec, VerbFlag, WorldStateSource},
        util::BitEnum,
    };
    use moor_compiler::{CompileOptions, compile};
    use moor_db::{DatabaseConfig, TxDB};
    use moor_var::{Obj, SYSTEM_OBJECT, Symbol, program::ProgramType, v_int};

    fn database_with_export_data() -> TxDB {
        let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
        let permissions = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
        let mut world_state = database.new_world_state().unwrap();
        let system = world_state
            .create_object(
                &permissions,
                &Obj::mk_id(-1),
                &SYSTEM_OBJECT,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        assert_eq!(system, SYSTEM_OBJECT);
        let child = world_state
            .create_object(
                &permissions,
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        world_state
            .define_property(
                &permissions,
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                Symbol::mk("checkpoint_property"),
                &SYSTEM_OBJECT,
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                Some(v_int(1)),
            )
            .unwrap();
        world_state
            .update_property(
                &permissions,
                &child,
                Symbol::mk("checkpoint_property"),
                &v_int(2),
            )
            .unwrap();
        world_state
            .add_verb(
                &permissions,
                &SYSTEM_OBJECT,
                vec![Symbol::mk("checkpoint_verb")],
                &SYSTEM_OBJECT,
                BitEnum::new_with(VerbFlag::Exec),
                VerbArgsSpec::this_none_this(),
                ProgramType::MooR(compile("return 1;", CompileOptions::default()).unwrap()),
            )
            .unwrap();
        world_state.commit().unwrap();
        database
    }

    #[test]
    fn failed_export_keeps_only_the_in_progress_directory() {
        let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
        let permissions = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
        let mut world_state = database.new_world_state().unwrap();
        world_state
            .create_object(
                &permissions,
                &Obj::mk_id(-1),
                &SYSTEM_OBJECT,
                BitEnum::new(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        world_state.commit().unwrap();

        let snapshot = database.create_snapshot().unwrap();
        let output = tempfile::tempdir().unwrap();
        let in_progress = output.path().join("checkpoint-1.in-progress");
        std::fs::create_dir_all(in_progress.join("object_0.moo")).unwrap();

        assert!(perform_export(snapshot.as_ref(), &in_progress, 1).is_err());
        assert!(in_progress.is_dir());
        assert!(!output.path().join("checkpoint-1.moo").exists());
    }

    #[test]
    fn completed_checkpoint_contains_authoritative_manifest() {
        let database = database_with_export_data();
        let snapshot = database.create_snapshot().unwrap();
        let output = tempfile::tempdir().unwrap();
        let in_progress = output.path().join("checkpoint-1.in-progress");

        perform_export(snapshot.as_ref(), &in_progress, 1).unwrap();

        assert!(!in_progress.exists());
        let final_path = output.path().join("checkpoint-1.moo");
        let manifest: CheckpointManifest =
            serde_json::from_reader(File::open(final_path.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest.format_version, Version::new(2, 0, 0));
        assert_eq!(manifest.snapshot_epoch, 1);
        assert!(manifest.export_completion_epoch >= manifest.snapshot_epoch);
        assert_eq!(manifest.object_count, 2);
        assert_eq!(manifest.verb_count, 1);
        assert_eq!(manifest.property_count, 1);
        assert_eq!(manifest.override_count, 1);
    }

    #[test]
    fn manifest_failure_keeps_checkpoint_in_progress() {
        let database = database_with_export_data();
        let snapshot = database.create_snapshot().unwrap();
        let output = tempfile::tempdir().unwrap();
        let in_progress = output.path().join("checkpoint-2.in-progress");
        fs::create_dir_all(in_progress.join("manifest.json")).unwrap();

        assert!(perform_export(snapshot.as_ref(), &in_progress, 2).is_err());

        assert!(in_progress.is_dir());
        assert!(!output.path().join("checkpoint-2.moo").exists());
    }
}
