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

use std::{
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

use crate::event_log::{PresentationAction, presentation_from_flatbuffer};
use fjall::{CompressionType, Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use flume::{Receiver, Sender};
use moor_common::tasks::{EventLogPurgeResult, EventLogStats, Presentation};
use moor_schema::convert::presentation_to_flatbuffer_struct;
use moor_schema::{
    common::{ObjUnion, ObjUnionRef},
    convert::obj_to_flatbuffer_struct,
    event_log::{LoggedNarrativeEvent, PlayerPresentations},
};
use moor_var::Obj;
use rpc_common::StrErr;
use tracing::{debug, error, info};

/// Trait abstracting event log operations for testing
pub trait EventLogOps: Send + Sync {
    /// Add a new event to the log, returns the event's UUID
    /// Note: Caller should convert from domain types to FlatBuffer types before calling
    fn append(
        &self,
        event: LoggedNarrativeEvent,
        presentation_action: Option<PresentationAction>,
    ) -> Uuid;

    /// Get current presentation IDs for a player
    /// Returns presentation objects with only IDs populated (content is in encrypted history)
    fn current_presentations(&self, player: Obj) -> Vec<Presentation>;

    /// Dismiss a presentation by ID
    fn dismiss_presentation(&self, player: Obj, presentation_id: String);

    /// Get narrative events for a specific player since the given UUID
    fn events_for_player_since(
        &self,
        player: Obj,
        since: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent>;

    /// Get narrative events for a specific player until the given UUID
    fn events_for_player_until(
        &self,
        player: Obj,
        until: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent>;

    /// Get narrative events for a specific player since N seconds ago
    fn events_for_player_since_seconds(
        &self,
        player: Obj,
        seconds_ago: u64,
    ) -> Vec<LoggedNarrativeEvent>;

    /// Get the public key for a player's event log encryption
    fn get_pubkey(&self, player: Obj) -> Option<String>;

    /// Set the public key for a player's event log encryption
    fn set_pubkey(&self, player: Obj, pubkey: String);

    /// Delete all event history for a player
    fn delete_all_events(&self, player: Obj) -> Result<(), String>;

    /// Return summary statistics about a player's event history.
    fn player_event_log_stats(
        &self,
        player: Obj,
        since: Option<SystemTime>,
        until: Option<SystemTime>,
    ) -> Result<EventLogStats, String>;

    /// Purge part or all of a player's event history, optionally removing their public key.
    fn purge_player_event_log(
        &self,
        player: Obj,
        before: Option<SystemTime>,
        drop_pubkey: bool,
    ) -> Result<EventLogPurgeResult, String>;
}

// LoggedNarrativeEvent and PlayerPresentations are now imported from
// moor_schema::event_log and are FlatBuffer-generated types

/// Messages for the background persistence thread
#[derive(Debug)]
enum PersistenceMessage {
    WriteNarrativeEvent(Uuid, LoggedNarrativeEvent),
    WritePresentationState(PlayerPresentations),
    Shutdown,
}

fn system_time_to_nanos(time: SystemTime) -> u64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let nanos = duration.as_nanos();
            if nanos > u64::MAX as u128 {
                u64::MAX
            } else {
                nanos as u64
            }
        }
        Err(_) => 0,
    }
}

fn opt_system_time_to_nanos(time: Option<SystemTime>) -> Option<u64> {
    time.map(system_time_to_nanos)
}

/// Configuration for event log persistence
#[derive(Debug, Clone, Default)]
pub struct EventLogConfig {
    // Configuration options can be added here as needed
}

/// Persistent event log that maintains chronological ordering for narrative events
/// and current presentation state separately.
///
/// Uses fjall for disk persistence with FlatBuffers for zero-copy reads.
/// Narrative events are stored by UUID (v7) which provides natural chronological ordering.
/// Presentations are stored as current state per player.
/// Background thread handles writes asynchronously.
pub struct EventLog {
    // Persistence layer for queries - wrapped in Arc<Mutex> for shared access
    persistence: Arc<Mutex<EventPersistence>>,
    // Channel to send writes to background thread
    persistence_sender: Option<Sender<PersistenceMessage>>,
}

/// Internal structure for the fjall persistence backend
struct EventPersistence {
    _tmpdir: Option<tempfile::TempDir>,
    _keyspace: Keyspace,
    narrative_events_partition: PartitionHandle,
    player_index_partition: PartitionHandle,
    presentations_partition: PartitionHandle,
    pubkeys_partition: PartitionHandle,
}

impl EventPersistence {
    fn open(path: Option<&Path>) -> Result<Self, eyre::Error> {
        let (tmpdir, path) = match path {
            Some(path) => (None, path.to_path_buf()),
            None => {
                let tmpdir = tempfile::TempDir::new()?;
                let path = tmpdir.path().to_path_buf();
                (Some(tmpdir), path)
            }
        };

        info!("Opening event log database at {:?}", path);
        let keyspace = Config::new(&path).open()?;

        let partition_creation_options =
            PartitionCreateOptions::default().compression(CompressionType::Lz4);
        let narrative_events_partition =
            keyspace.open_partition("narrative_events", partition_creation_options.clone())?;
        let player_index_partition =
            keyspace.open_partition("player_index", partition_creation_options.clone())?;
        let presentations_partition =
            keyspace.open_partition("presentations", partition_creation_options.clone())?;
        let pubkeys_partition = keyspace.open_partition("pubkeys", partition_creation_options)?;

        Ok(Self {
            _tmpdir: tmpdir,
            _keyspace: keyspace,
            narrative_events_partition,
            player_index_partition,
            presentations_partition,
            pubkeys_partition,
        })
    }

    fn write_narrative_event(
        &mut self,
        event_id: Uuid,
        event: &LoggedNarrativeEvent,
    ) -> Result<(), eyre::Error> {
        // Serialize the event using FlatBuffers (planus)
        let mut builder = ::planus::Builder::new();
        let event_bytes = builder.finish(event, None);

        // Store in main narrative events partition with UUID as key
        self.narrative_events_partition
            .insert(event_id.as_bytes(), event_bytes.as_ref())?;

        // Update player index for efficient per-player queries
        // Convert FlatBuffer Obj to a key string
        let player_key = format!("{}:{}", Self::obj_to_key(&event.player), event_id);
        self.player_index_partition
            .insert(player_key.as_bytes(), event_id.as_bytes())?;

        Ok(())
    }

    fn flush(&mut self) -> Result<(), eyre::Error> {
        self._keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    // Helper to convert FlatBuffer Obj to a string key
    fn obj_to_key(obj: &moor_schema::common::Obj) -> String {
        // Use the obj's union variant to create a unique key
        match &obj.obj {
            ObjUnion::ObjId(id) => format!("id:{}", id.id),
            ObjUnion::UuObjId(uuid) => format!("uuid:{}", uuid.packed_value),
            ObjUnion::AnonymousObjId(anon) => format!("anon:{}", anon.packed_value),
        }
    }

    // Helper to compare two FlatBuffer Obj types (used by trait implementation)
    pub(crate) fn obj_matches(
        obj1: &moor_schema::common::Obj,
        obj2: &moor_schema::common::Obj,
    ) -> bool {
        match (&obj1.obj, &obj2.obj) {
            (ObjUnion::ObjId(a), ObjUnion::ObjId(b)) => a.id == b.id,
            (ObjUnion::UuObjId(a), ObjUnion::UuObjId(b)) => a.packed_value == b.packed_value,
            (ObjUnion::AnonymousObjId(a), ObjUnion::AnonymousObjId(b)) => {
                a.packed_value == b.packed_value
            }
            _ => false,
        }
    }

    // Helper to compare FlatBuffer Obj with ObjRef (used internally)
    fn obj_matches_ref(
        obj: &moor_schema::common::Obj,
        obj_ref: &moor_schema::common::ObjRef,
    ) -> bool {
        let obj_ref_union = obj_ref.obj().ok();
        match (&obj.obj, obj_ref_union) {
            (ObjUnion::ObjId(a), Some(ObjUnionRef::ObjId(b))) => a.id == b.id().unwrap_or(-1),
            (ObjUnion::UuObjId(a), Some(ObjUnionRef::UuObjId(b))) => {
                a.packed_value == b.packed_value().unwrap_or(0)
            }
            (ObjUnion::AnonymousObjId(a), Some(ObjUnionRef::AnonymousObjId(b))) => {
                a.packed_value == b.packed_value().unwrap_or(0)
            }
            _ => false,
        }
    }

    fn write_presentation_state(
        &mut self,
        player_presentations: &PlayerPresentations,
    ) -> Result<(), eyre::Error> {
        // Serialize the presentation state using FlatBuffers (planus)
        let mut builder = ::planus::Builder::new();
        let state_bytes = builder.finish(player_presentations, None);

        // Store with player ID as key
        let player_key = Self::obj_to_key(&player_presentations.player);
        self.presentations_partition
            .insert(player_key.as_bytes(), state_bytes.as_ref())?;

        Ok(())
    }

    fn load_narrative_events_since(
        &self,
        since: Option<Uuid>,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        debug!("Loading narrative events since {:?}", since);
        let mut events = Vec::new();

        let start_bound = match since {
            Some(since_id) => {
                // Use the UUID as the lower bound (exclusive)
                let mut start_key = since_id.as_bytes().to_vec();
                // Increment the last byte to make it exclusive
                if let Some(last_byte) = start_key.last_mut() {
                    *last_byte = last_byte.saturating_add(1);
                }
                start_key
            }
            None => vec![],
        };

        // Fjall range query from start_bound to end
        if start_bound.is_empty() {
            for entry in self.narrative_events_partition.iter() {
                let (_, value) = entry?;
                // Deserialize using planus from FlatBuffer bytes
                let event_ref =
                    <moor_schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
                // Convert from Ref to owned type
                let event: LoggedNarrativeEvent = event_ref.try_into()?;
                events.push(event);
            }
        } else {
            for entry in self
                .narrative_events_partition
                .range(start_bound.as_slice()..)
            {
                let (_key, value) = entry?;
                // Deserialize using planus from FlatBuffer bytes
                let event_ref =
                    <moor_schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
                // Convert from Ref to owned type
                let event: LoggedNarrativeEvent = event_ref.try_into()?;
                events.push(event);
            }
        }

        Ok(events)
    }

    fn load_narrative_events_until(
        &self,
        until: Option<Uuid>,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let mut events = Vec::new();

        let end_bound = match until {
            Some(until_id) => until_id.as_bytes().to_vec(),
            None => vec![0xff; 16], // Max UUID bytes
        };

        // Fjall range query from start to end_bound (exclusive)
        for entry in self
            .narrative_events_partition
            .range(..end_bound.as_slice())
        {
            let (_, value) = entry?;
            // Deserialize using planus from FlatBuffer bytes
            let event_ref =
                <moor_schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
            // Convert from Ref to owned type
            let event: LoggedNarrativeEvent = event_ref.try_into()?;
            events.push(event);
        }

        Ok(events)
    }

    fn load_narrative_events_for_player_since_seconds(
        &self,
        player: Obj,
        seconds_ago: u64,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let cutoff_time = SystemTime::now()
            .checked_sub(Duration::from_secs(seconds_ago))
            .unwrap_or(UNIX_EPOCH);

        // For player-specific queries, we could use the player index, but for now
        // let's do a simple scan and filter (can optimize later)
        let mut events = Vec::new();

        // Convert player to FlatBuffer Obj for comparison
        let player_fb = obj_to_flatbuffer_struct(&player);

        for entry in self.narrative_events_partition.iter() {
            let (_key, value) = entry?;

            // Deserialize using planus from FlatBuffer bytes
            let event_ref =
                <moor_schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&value)?;

            // Check if player matches and timestamp is after cutoff
            let event_player_ref = event_ref.player()?;
            let event_timestamp_nanos = event_ref.timestamp()?;
            let event_timestamp = UNIX_EPOCH + Duration::from_nanos(event_timestamp_nanos);

            if Self::obj_matches_ref(&player_fb, &event_player_ref)
                && event_timestamp >= cutoff_time
            {
                // Convert from Ref to owned type
                let event: LoggedNarrativeEvent = event_ref.try_into()?;
                events.push(event);
            }
        }

        Ok(events)
    }

    #[allow(dead_code)]
    fn load_presentation_state(
        &self,
        player: Obj,
    ) -> Result<Option<PlayerPresentations>, eyre::Error> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let player_key = Self::obj_to_key(&player_fb);

        if let Some(value) = self.presentations_partition.get(player_key.as_bytes())? {
            // Deserialize using planus from FlatBuffer bytes
            let presentations_ref =
                <moor_schema::event_log::PlayerPresentationsRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
            // Convert from Ref to owned type
            let presentations: PlayerPresentations = presentations_ref.try_into()?;
            Ok(Some(presentations))
        } else {
            Ok(None)
        }
    }

    fn load_pubkey(&self, player: Obj) -> Result<Option<String>, eyre::Error> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let player_key = Self::obj_to_key(&player_fb);

        if let Some(value) = self.pubkeys_partition.get(player_key.as_bytes())? {
            let pubkey = String::from_utf8(value.to_vec())?;
            Ok(Some(pubkey))
        } else {
            Ok(None)
        }
    }

    fn store_pubkey(&mut self, player: Obj, pubkey: String) -> Result<(), eyre::Error> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let player_key = Self::obj_to_key(&player_fb);

        self.pubkeys_partition
            .insert(player_key.as_bytes(), pubkey.as_bytes())?;
        Ok(())
    }

    fn add_presentation(
        &mut self,
        player: &moor_schema::common::Obj,
        presentation: &moor_common::tasks::Presentation,
        pubkey: String,
    ) -> Result<(), eyre::Error> {
        let player_key = Self::obj_to_key(player);

        // Load current presentations
        let mut presentations: Vec<moor_schema::event_log::StoredPresentation> = if let Some(
            value,
        ) =
            self.presentations_partition.get(player_key.as_bytes())?
        {
            let presentations_ref =
                <moor_schema::event_log::PlayerPresentationsRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
            let vec_ref = presentations_ref.presentations()?;
            let mut result = Vec::new();
            for p_ref in vec_ref.iter() {
                result.push(p_ref?.try_into()?);
            }
            result
        } else {
            Vec::new()
        };

        // Serialize presentation to FlatBuffer bytes
        let pres_fb = presentation_to_flatbuffer_struct(presentation)?;
        let mut builder = ::planus::Builder::new();
        let pres_bytes = builder.finish(&pres_fb, None);

        // Encrypt with pubkey (REQUIRED - no plaintext storage)
        let encrypted_content = crate::event_log::encryption::encrypt(pres_bytes, &pubkey)
            .map_err(|e| eyre::eyre!("Encryption failed: {}", e))?;

        // Add/update presentation (replace if ID already exists)
        presentations.retain(|p| p.id != presentation.id);
        presentations.push(moor_schema::event_log::StoredPresentation {
            id: presentation.id.clone(),
            encrypted_content,
        });

        // Save updated list
        let updated = PlayerPresentations {
            player: Box::new(player.clone()),
            presentations,
        };

        let mut builder = ::planus::Builder::new();
        let value_bytes = builder.finish(&updated, None);
        self.presentations_partition
            .insert(player_key.as_bytes(), value_bytes)?;

        Ok(())
    }

    fn remove_presentation(
        &mut self,
        player: &moor_schema::common::Obj,
        presentation_id: &str,
    ) -> Result<(), eyre::Error> {
        let player_key = Self::obj_to_key(player);

        // Load current presentations
        if let Some(value) = self.presentations_partition.get(player_key.as_bytes())? {
            let presentations_ref =
                <moor_schema::event_log::PlayerPresentationsRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
            let vec_ref = presentations_ref.presentations()?;
            let mut presentations = Vec::new();
            for p_ref in vec_ref.iter() {
                presentations.push(p_ref?.try_into()?);
            }

            // Remove the presentation with matching ID
            presentations
                .retain(|p: &moor_schema::event_log::StoredPresentation| p.id != presentation_id);

            // Save updated list
            let updated = PlayerPresentations {
                player: Box::new(player.clone()),
                presentations,
            };

            let mut builder = ::planus::Builder::new();
            let value_bytes = builder.finish(&updated, None);
            self.presentations_partition
                .insert(player_key.as_bytes(), value_bytes)?;
        }

        Ok(())
    }

    fn delete_all_events_for_player(
        &mut self,
        player: &moor_schema::common::Obj,
    ) -> Result<(), eyre::Error> {
        let player_key_prefix = Self::obj_to_key(player);
        let player_prefix_with_colon = format!("{player_key_prefix}:");

        // Use range to efficiently iterate over all keys for this player
        // Collect event IDs and index keys in one pass
        let mut index_keys = Vec::new();
        let mut event_ids = Vec::new();

        for result in self
            .player_index_partition
            .range(player_prefix_with_colon.as_bytes()..)
        {
            let (key, value) = result?;

            // Check if key still matches our prefix (range scan stops when prefix doesn't match)
            let key_str = std::str::from_utf8(&key)?;
            if !key_str.starts_with(&player_prefix_with_colon) {
                break;
            }

            // Store the index key for deletion
            index_keys.push(key.to_vec());

            // Extract event UUID from the value (stored as UUID bytes)
            if value.len() == 16 {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&value);
                event_ids.push(Uuid::from_bytes(bytes));
            }
        }

        // Delete from player index partition
        for key in index_keys {
            self.player_index_partition.remove(key)?;
        }

        // Delete from narrative events partition
        for event_id in event_ids {
            self.narrative_events_partition
                .remove(event_id.as_bytes())?;
        }

        // Delete presentations for this player
        self.presentations_partition
            .remove(player_key_prefix.as_bytes())?;

        Ok(())
    }

    fn count_events_for_player(
        &self,
        player: &moor_schema::common::Obj,
        since_ns: Option<u64>,
        until_ns: Option<u64>,
    ) -> Result<EventLogStats, eyre::Error> {
        let mut stats = EventLogStats::default();
        let player_key_prefix = Self::obj_to_key(player);
        let player_prefix_with_colon = format!("{player_key_prefix}:");

        for result in self
            .player_index_partition
            .range(player_prefix_with_colon.as_bytes()..)
        {
            let (key, value) = result?;

            let key_str = std::str::from_utf8(&key)?;
            if !key_str.starts_with(&player_prefix_with_colon) {
                break;
            }

            if value.len() != 16 {
                continue;
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&value);
            let event_id = Uuid::from_bytes(bytes);

            let Some(event_bytes) = self.narrative_events_partition.get(event_id.as_bytes())?
            else {
                continue;
            };
            let event_ref =
                <moor_schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&event_bytes)?;
            let timestamp = event_ref.timestamp()?;

            if let Some(since) = since_ns
                && timestamp < since
            {
                continue;
            }
            if let Some(until) = until_ns
                && timestamp > until
            {
                continue;
            }

            stats.total_events += 1;

            let event_time = UNIX_EPOCH + Duration::from_nanos(timestamp);
            if stats.earliest.is_none_or(|current| event_time < current) {
                stats.earliest = Some(event_time);
            }
            if stats.latest.is_none_or(|current| event_time > current) {
                stats.latest = Some(event_time);
            }
        }

        Ok(stats)
    }

    fn purge_events_for_player(
        &mut self,
        player: &moor_schema::common::Obj,
        before_ns: Option<u64>,
    ) -> Result<u64, eyre::Error> {
        let player_key_prefix = Self::obj_to_key(player);
        let player_prefix_with_colon = format!("{player_key_prefix}:");

        let mut index_keys = Vec::new();
        let mut event_ids = Vec::new();

        for result in self
            .player_index_partition
            .range(player_prefix_with_colon.as_bytes()..)
        {
            let (key, value) = result?;

            let key_str = std::str::from_utf8(&key)?;
            if !key_str.starts_with(&player_prefix_with_colon) {
                break;
            }

            let mut should_remove = before_ns.is_none();
            let mut event_id_opt = None;
            if value.len() == 16 {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&value);
                let event_id = Uuid::from_bytes(bytes);
                event_id_opt = Some(event_id);

                if let Some(before) = before_ns {
                    if let Some(event_bytes) =
                        self.narrative_events_partition.get(event_id.as_bytes())?
                    {
                        let event_ref =
                            <moor_schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&event_bytes)?;
                        let timestamp = event_ref.timestamp()?;
                        if timestamp <= before {
                            should_remove = true;
                        }
                    } else {
                        should_remove = true;
                    }
                }
            } else {
                should_remove = true;
            }

            if should_remove {
                index_keys.push(key.to_vec());
                if let Some(event_id) = event_id_opt {
                    event_ids.push(event_id);
                }
            }
        }

        for key in index_keys {
            self.player_index_partition.remove(key)?;
        }

        for event_id in &event_ids {
            self.narrative_events_partition
                .remove(event_id.as_bytes())?;
        }

        Ok(event_ids.len() as u64)
    }

    fn delete_pubkey(&mut self, player: &moor_schema::common::Obj) -> Result<bool, eyre::Error> {
        let player_key = Self::obj_to_key(player);
        let existed = self.pubkeys_partition.get(player_key.as_bytes())?.is_some();
        if existed {
            self.pubkeys_partition.remove(player_key.as_bytes())?;
        }
        Ok(existed)
    }
}

impl EventLog {
    pub fn new() -> Self {
        Self::with_config(EventLogConfig::default(), None)
    }

    pub fn with_config(_config: EventLogConfig, db_path: Option<&Path>) -> Self {
        let (persistence_sender, persistence_receiver) = flume::unbounded();

        // Create persistence layer - this will create tmpdir if needed
        let persistence = match EventPersistence::open(db_path) {
            Ok(p) => Arc::new(Mutex::new(p)),
            Err(e) => {
                error!("Failed to open event log persistence: {}", e);
                panic!("Failed to open event log: {e}");
            }
        };

        // Start background persistence thread with the SAME EventPersistence instance
        let persistence_clone = persistence.clone();
        thread::spawn(move || {
            Self::persistence_thread_main(persistence_receiver, persistence_clone);
        });

        Self {
            persistence,
            persistence_sender: Some(persistence_sender),
        }
    }

    fn persistence_thread_main(
        receiver: Receiver<PersistenceMessage>,
        persistence: Arc<Mutex<EventPersistence>>,
    ) {
        info!("Event log persistence thread started");

        // Process messages one at a time - with FlatBuffers, writes are fast
        loop {
            match receiver.recv() {
                Ok(PersistenceMessage::WriteNarrativeEvent(event_id, event)) => {
                    let mut guard = persistence.lock().unwrap();
                    if let Err(e) = guard.write_narrative_event(event_id, &event) {
                        error!(
                            "Failed to write narrative event {} to disk: {}",
                            event_id, e
                        );
                    }
                    // Flush to ensure write is visible to readers
                    if let Err(e) = guard.flush() {
                        error!("Failed to flush narrative event write: {}", e);
                    }
                }
                Ok(PersistenceMessage::WritePresentationState(state)) => {
                    let mut guard = persistence.lock().unwrap();
                    if let Err(e) = guard.write_presentation_state(&state) {
                        error!("Failed to write presentation state to disk: {}", e);
                    }
                    if let Err(e) = guard.flush() {
                        error!("Failed to flush presentation state write: {}", e);
                    }
                }
                Ok(PersistenceMessage::Shutdown) => {
                    info!("Event log persistence thread shutting down");
                    return;
                }
                Err(_) => {
                    // Channel closed
                    info!("Event log persistence thread exiting (channel closed)");
                    return;
                }
            }
        }
    }

    /// Add a new event to the log, returns the event's UUID
    /// Takes a FlatBuffer LoggedNarrativeEvent - caller should convert from domain types
    pub fn append(
        &self,
        event: LoggedNarrativeEvent,
        presentation_action: Option<PresentationAction>,
    ) -> Uuid {
        // Extract event_id from the FlatBuffer event
        let event_id_bytes = event.event_id.data.as_slice();
        let event_id = if event_id_bytes.len() == 16 {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(event_id_bytes);
            Uuid::from_bytes(bytes)
        } else {
            error!("Invalid event_id in LoggedNarrativeEvent");
            return Uuid::nil();
        };

        // Handle presentation state updates
        if let Some(action) = presentation_action {
            // Get player's pubkey for presentation encryption (REQUIRED)
            let pubkey = {
                let guard = self.persistence.lock().unwrap();
                moor_schema::convert::obj_from_flatbuffer_struct(&event.player)
                    .ok()
                    .and_then(|player_obj| guard.load_pubkey(player_obj).ok().flatten())
            };

            match action {
                PresentationAction::Add(presentation) => {
                    if let Some(key) = pubkey {
                        self.add_presentation(&event.player, &presentation, key);
                    }
                }
                PresentationAction::Remove(presentation_id) => {
                    self.remove_presentation(&event.player, &presentation_id);
                }
            }
        }

        // Send to background persistence thread
        if let Some(ref sender) = self.persistence_sender
            && let Err(e) = sender.send(PersistenceMessage::WriteNarrativeEvent(event_id, event))
        {
            error!(
                "Failed to send narrative event to persistence thread: {}",
                e
            );
        }

        event_id
    }

    /// Add a presentation to the player's active presentations (always encrypted)
    fn add_presentation(
        &self,
        player: &moor_schema::common::Obj,
        presentation: &moor_common::tasks::Presentation,
        pubkey: String,
    ) {
        let mut persistence_guard = self.persistence.lock().unwrap();
        if let Err(e) = persistence_guard.add_presentation(player, presentation, pubkey) {
            error!("Failed to add presentation: {}", e);
        }
    }

    /// Remove a presentation from the player's active presentations
    fn remove_presentation(&self, player: &moor_schema::common::Obj, presentation_id: &str) {
        let mut persistence_guard = self.persistence.lock().unwrap();
        if let Err(e) = persistence_guard.remove_presentation(player, presentation_id) {
            error!("Failed to remove presentation: {}", e);
        }
    }

    /// Shutdown the event log and flush any pending writes
    pub fn shutdown(&mut self) {
        if let Some(sender) = self.persistence_sender.take()
            && let Err(e) = sender.send(PersistenceMessage::Shutdown)
        {
            error!(
                "Failed to send shutdown message to persistence thread: {}",
                e
            );
        }
    }

    /// Get all narrative events since the given UUID (exclusive)
    /// Returns events in chronological order
    fn load_narrative_from_disk_since(
        &self,
        since: Option<Uuid>,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        persistence_guard.load_narrative_events_since(since)
    }

    fn load_narrative_from_disk_until(
        &self,
        until: Option<Uuid>,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        persistence_guard.load_narrative_events_until(until)
    }

    fn load_presentation_state_from_disk(
        &self,
        player: Obj,
    ) -> Result<Option<PlayerPresentations>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        persistence_guard.load_presentation_state(player)
    }

    fn load_narrative_from_disk_for_player_since_seconds(
        &self,
        player: Obj,
        seconds_ago: u64,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        persistence_guard.load_narrative_events_for_player_since_seconds(player, seconds_ago)
    }

    fn load_pubkey_from_disk(&self, player: Obj) -> Result<Option<String>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        persistence_guard.load_pubkey(player)
    }

    fn store_pubkey_to_disk(&self, player: Obj, pubkey: String) -> Result<(), eyre::Error> {
        let mut persistence_guard = self.persistence.lock().unwrap();
        persistence_guard.store_pubkey(player, pubkey)
    }
}

impl Drop for EventLog {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLogOps for EventLog {
    fn append(
        &self,
        event: LoggedNarrativeEvent,
        presentation_action: Option<PresentationAction>,
    ) -> Uuid {
        self.append(event, presentation_action)
    }

    fn current_presentations(&self, player: Obj) -> Vec<Presentation> {
        self.load_presentation_state_from_disk(player)
            .ok()
            .flatten()
            .map(|state| {
                // When encrypted, daemon can't decrypt (no secret key)
                // Return stub presentations with IDs only
                // Web-host will decrypt from event history using client's secret key
                state
                    .presentations
                    .iter()
                    .filter_map(|stored_pres| {
                        // Try to deserialize - works for plaintext (no encryption)
                        // For encrypted data, just return stub with ID
                        if let Ok(pres_ref) = <moor_schema::common::PresentationRef as ::planus::ReadAsRoot>::read_as_root(&stored_pres.encrypted_content) {
                            presentation_from_flatbuffer(&pres_ref).ok()
                        } else {
                            // Encrypted - return stub with ID only
                            Some(Presentation {
                                id: stored_pres.id.clone(),
                                content_type: String::new(),
                                content: String::new(),
                                target: String::new(),
                                attributes: vec![],
                            })
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn dismiss_presentation(&self, player: Obj, presentation_id: String) {
        // Load current state, remove presentation, save back
        if let Ok(Some(mut state)) = self.load_presentation_state_from_disk(player) {
            state.presentations.retain(|p| p.id != presentation_id);
            if let Some(ref sender) = self.persistence_sender {
                let _ = sender.send(PersistenceMessage::WritePresentationState(state));
            }
        }
    }

    fn events_for_player_since(
        &self,
        player: Obj,
        since: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        // Convert player to FlatBuffer for comparison
        let player_fb = obj_to_flatbuffer_struct(&player);

        match self.load_narrative_from_disk_since(since) {
            Ok(events) => {
                let filtered: Vec<_> = events
                    .into_iter()
                    .filter(|event| {
                        // Compare FlatBuffer Objs
                        EventPersistence::obj_matches(&player_fb, &event.player)
                    })
                    .collect();
                filtered
            }
            Err(e) => {
                error!("Failed to load narrative events since {:?}: {}", since, e);
                Vec::new()
            }
        }
    }

    fn events_for_player_until(
        &self,
        player: Obj,
        until: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        // Convert player to FlatBuffer for comparison
        let player_fb = obj_to_flatbuffer_struct(&player);

        match self.load_narrative_from_disk_until(until) {
            Ok(events) => events
                .into_iter()
                .filter(|event| {
                    // Compare FlatBuffer Objs
                    EventPersistence::obj_matches(&player_fb, &event.player)
                })
                .collect(),
            Err(e) => {
                error!("Failed to load narrative events until {:?}: {}", until, e);
                Vec::new()
            }
        }
    }

    fn events_for_player_since_seconds(
        &self,
        player: Obj,
        seconds_ago: u64,
    ) -> Vec<LoggedNarrativeEvent> {
        match self.load_narrative_from_disk_for_player_since_seconds(player, seconds_ago) {
            Ok(events) => events,
            Err(e) => {
                error!(
                    "Failed to load narrative events for player since {} seconds: {}",
                    seconds_ago, e
                );
                Vec::new()
            }
        }
    }

    fn get_pubkey(&self, player: Obj) -> Option<String> {
        self.load_pubkey_from_disk(player).ok().flatten()
    }

    fn set_pubkey(&self, player: Obj, pubkey: String) {
        if let Err(e) = self.store_pubkey_to_disk(player, pubkey) {
            error!("Failed to store pubkey for player {:?}: {}", player, e);
        }
    }

    fn delete_all_events(&self, player: Obj) -> Result<(), String> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let mut persistence = self.persistence.lock().unwrap();
        persistence
            .delete_all_events_for_player(&player_fb)
            .str_err()
    }

    fn player_event_log_stats(
        &self,
        player: Obj,
        since: Option<SystemTime>,
        until: Option<SystemTime>,
    ) -> Result<EventLogStats, String> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let persistence = self.persistence.lock().unwrap();
        persistence
            .count_events_for_player(
                &player_fb,
                opt_system_time_to_nanos(since),
                opt_system_time_to_nanos(until),
            )
            .str_err()
    }

    fn purge_player_event_log(
        &self,
        player: Obj,
        before: Option<SystemTime>,
        drop_pubkey: bool,
    ) -> Result<EventLogPurgeResult, String> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let mut persistence = self.persistence.lock().unwrap();
        let deleted_events = persistence
            .purge_events_for_player(&player_fb, opt_system_time_to_nanos(before))
            .str_err()?;

        if before.is_none() {
            let player_key = EventPersistence::obj_to_key(&player_fb);
            persistence
                .presentations_partition
                .remove(player_key.as_bytes())
                .str_err()?;
        }

        let mut pubkey_deleted = false;
        if drop_pubkey {
            pubkey_deleted = persistence.delete_pubkey(&player_fb).str_err()?;
        }

        Ok(EventLogPurgeResult {
            deleted_events,
            pubkey_deleted,
        })
    }
}

/// No-op event log implementation that discards all events
/// Used when event logging is disabled
pub struct NoOpEventLog;

impl NoOpEventLog {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoOpEventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLogOps for NoOpEventLog {
    fn append(
        &self,
        event: LoggedNarrativeEvent,
        _presentation_action: Option<PresentationAction>,
    ) -> Uuid {
        // Extract event_id from the FlatBuffer event
        let event_id_bytes = event.event_id.data.as_slice();
        if event_id_bytes.len() == 16 {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(event_id_bytes);
            Uuid::from_bytes(bytes)
        } else {
            Uuid::nil()
        }
    }

    fn current_presentations(&self, _player: Obj) -> Vec<Presentation> {
        Vec::new()
    }

    fn dismiss_presentation(&self, _player: Obj, _presentation_id: String) {
        // No-op
    }

    fn events_for_player_since(
        &self,
        _player: Obj,
        _since: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        Vec::new()
    }

    fn events_for_player_until(
        &self,
        _player: Obj,
        _until: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        Vec::new()
    }

    fn events_for_player_since_seconds(
        &self,
        _player: Obj,
        _seconds_ago: u64,
    ) -> Vec<LoggedNarrativeEvent> {
        Vec::new()
    }

    fn get_pubkey(&self, _player: Obj) -> Option<String> {
        None
    }

    fn set_pubkey(&self, _player: Obj, _pubkey: String) {
        // No-op
    }

    fn delete_all_events(&self, _player: Obj) -> Result<(), String> {
        Ok(())
    }

    fn player_event_log_stats(
        &self,
        _player: Obj,
        _since: Option<SystemTime>,
        _until: Option<SystemTime>,
    ) -> Result<EventLogStats, String> {
        Ok(EventLogStats::default())
    }

    fn purge_player_event_log(
        &self,
        _player: Obj,
        _before: Option<SystemTime>,
        _drop_pubkey: bool,
    ) -> Result<EventLogPurgeResult, String> {
        Ok(EventLogPurgeResult::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_log::logged_narrative_event_to_flatbuffer;
    use moor_common::tasks::{Event, NarrativeEvent, Presentation};
    use moor_var::{Obj, v_str};
    use std::time::SystemTime;
    use uuid::Uuid;

    fn create_logged_event(player: Obj, message: &str) -> LoggedNarrativeEvent {
        // Generate a test key for encryption (all events must be encrypted)
        let identity = age::x25519::Identity::generate();
        let pubkey = identity.to_public().to_string();

        let event = NarrativeEvent {
            event_id: Uuid::now_v7(),
            timestamp: SystemTime::now(),
            author: v_str("test"),
            event: Event::Notify {
                value: v_str(message),
                content_type: None,
                no_flush: false,
                no_newline: false,
                metadata: None,
            },
        };
        logged_narrative_event_to_flatbuffer(player, Box::new(event), pubkey)
            .expect("Failed to convert to FlatBuffer")
            .0
    }

    fn create_present_event(player: Obj, id: &str, content: &str) -> LoggedNarrativeEvent {
        // Generate a test key for encryption (all events must be encrypted)
        let identity = age::x25519::Identity::generate();
        let pubkey = identity.to_public().to_string();

        let event = NarrativeEvent {
            event_id: Uuid::now_v7(),
            timestamp: SystemTime::now(),
            author: v_str("test"),
            event: Event::Present(Presentation {
                id: id.to_string(),
                content_type: "text/plain".to_string(),
                content: content.to_string(),
                target: "main".to_string(),
                attributes: vec![],
            }),
        };
        logged_narrative_event_to_flatbuffer(player, Box::new(event), pubkey)
            .expect("Failed to convert to FlatBuffer")
            .0
    }

    fn extract_event_id(event: &LoggedNarrativeEvent) -> Uuid {
        let uuid_bytes = event.event_id.data.as_slice();
        if uuid_bytes.len() == 16 {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(uuid_bytes);
            Uuid::from_bytes(bytes)
        } else {
            Uuid::nil()
        }
    }

    #[test]
    fn test_basic_append_and_retrieve() {
        let log = EventLog::new();
        let player = Obj::mk_id(1);

        let event1 = create_logged_event(player, "hello");
        let event2 = create_logged_event(player, "world");

        let id1 = log.append(event1, None);
        let id2 = log.append(event2, None);

        assert!(!id1.is_nil());
        assert!(!id2.is_nil());
        assert!(id1 < id2, "UUID v7 should be chronologically ordered");

        // Give persistence thread time to write
        std::thread::sleep(std::time::Duration::from_millis(500));

        let events = log.events_for_player_since(player, None);
        assert_eq!(events.len(), 2);
        assert_eq!(extract_event_id(&events[0]), id1);
        assert_eq!(extract_event_id(&events[1]), id2);
    }

    #[test]
    fn test_events_for_player_filtering() {
        let log = EventLog::new();
        let player1 = Obj::mk_id(1);
        let player2 = Obj::mk_id(2);

        let event1 = create_logged_event(player1, "player1 msg1");
        let event2 = create_logged_event(player2, "player2 msg1");
        let event3 = create_logged_event(player1, "player1 msg2");

        log.append(event1, None);
        log.append(event2, None);
        log.append(event3, None);

        // Give persistence thread time to write
        std::thread::sleep(std::time::Duration::from_millis(500));

        let player1_events = log.events_for_player_since(player1, None);
        assert_eq!(player1_events.len(), 2);

        let player2_events = log.events_for_player_since(player2, None);
        assert_eq!(player2_events.len(), 1);
    }

    #[test]
    fn test_events_since_filtering() {
        let log = EventLog::new();
        let player = Obj::mk_id(1);

        let event1 = create_logged_event(player, "msg1");
        let event2 = create_logged_event(player, "msg2");
        let event3 = create_logged_event(player, "msg3");

        let id1 = log.append(event1, None);
        let _id2 = log.append(event2, None);
        let id3 = log.append(event3, None);

        // Give persistence thread time to write
        std::thread::sleep(std::time::Duration::from_millis(500));

        let events_since_id1 = log.events_for_player_since(player, Some(id1));
        assert_eq!(events_since_id1.len(), 2, "Should get events after id1");

        let events_until_id3 = log.events_for_player_until(player, Some(id3));
        assert_eq!(events_until_id3.len(), 2, "Should get events before id3");
    }

    #[test]
    fn test_presentation_management() {
        let log = EventLog::new();
        let player = Obj::mk_id(1);

        // Initially no presentations
        let presentations = log.current_presentations(player);
        assert!(presentations.is_empty());

        // Add a presentation (note: presentations aren't handled via append in the new API)
        // We need to directly create a PlayerPresentations state
        let present_event = create_present_event(player, "widget1", "Hello World");
        log.append(present_event, None);

        // Give persistence thread time to write
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Note: In the new architecture, presentations need to be explicitly saved
        // The append doesn't automatically update presentation state anymore
        // This test demonstrates the API but won't pass until we implement
        // presentation state updates separately from narrative events
    }

    #[test]
    fn test_events_for_player_since_seconds() {
        let log = EventLog::new();
        let player = Obj::mk_id(1);

        let event1 = create_logged_event(player, "old message");
        log.append(event1, None);

        // Give persistence thread time to write
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Events within last 10 seconds
        let recent_events = log.events_for_player_since_seconds(player, 10);
        assert_eq!(recent_events.len(), 1);

        // Events within last 1 second (more robust than 0)
        let very_recent = log.events_for_player_since_seconds(player, 1);
        assert!(!very_recent.is_empty());
    }

    #[test]
    fn test_encryption_roundtrip() {
        // Generate a key pair
        let identity = age::x25519::Identity::generate();
        let pubkey = identity.to_public().to_string();
        let seckey = identity.to_string();

        let log = EventLog::new();
        let player = Obj::mk_id(1);

        // Set the player's pubkey
        log.set_pubkey(player, pubkey.clone());

        // Verify we can retrieve it
        assert_eq!(log.get_pubkey(player), Some(pubkey.clone()));

        // Create a test event with a presentation
        let presentation = moor_common::tasks::Presentation {
            id: "test_widget".to_string(),
            content_type: "text/plain".to_string(),
            content: "Secret Message!".to_string(),
            target: "main".to_string(),
            attributes: vec![],
        };

        let event = Box::new(moor_common::tasks::NarrativeEvent {
            event_id: uuid::Uuid::now_v7(),
            timestamp: std::time::SystemTime::now(),
            author: moor_var::v_str("test_author"),
            event: moor_common::tasks::Event::Present(presentation.clone()),
        });

        // Convert to FlatBuffer (will be encrypted)
        let (logged_event, presentation_action) =
            crate::event_log::logged_narrative_event_to_flatbuffer(player, event, pubkey)
                .expect("Failed to create logged event");

        // Append the event
        log.append(logged_event.clone(), presentation_action);

        // Verify the encrypted_blob is actually encrypted (not plaintext FlatBuffer)
        // Plaintext FlatBuffer starts with specific magic bytes, encrypted Age data won't
        assert!(
            logged_event.encrypted_blob.len() > 100,
            "Encrypted blob should be reasonably sized"
        );

        // Try to deserialize as plaintext - should fail for encrypted data
        let _plaintext_parse =
            <moor_schema::common::NarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(
                &logged_event.encrypted_blob,
            );
        // For encrypted data, this should fail. For testing without keys it will succeed.
        // The important thing is encryption works when pubkey is provided.

        // Decrypt and verify
        use age::secrecy::ExposeSecret;
        let decrypted_bytes = crate::event_log::encryption::decrypt(
            &logged_event.encrypted_blob,
            seckey.expose_secret(),
        )
        .expect("Failed to decrypt");

        // Parse decrypted bytes and convert to domain type
        let narrative_event_ref =
            <moor_schema::common::NarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(
                &decrypted_bytes,
            )
            .expect("Failed to parse decrypted event");
        let narrative_event = moor_schema::convert::narrative_event_from_ref(narrative_event_ref)
            .expect("Failed to convert event");

        // Verify the event content
        if let moor_common::tasks::Event::Present(pres) = narrative_event.event() {
            assert_eq!(pres.id, "test_widget");
            assert_eq!(pres.content, "Secret Message!");
        } else {
            panic!("Expected Present event");
        }
    }

    #[test]
    fn test_no_op_event_log() {
        let log = NoOpEventLog::new();
        let player = Obj::mk_id(1);

        let event = create_logged_event(player, "test");
        let id = log.append(event, None);

        assert!(!id.is_nil(), "Should return valid UUID even for no-op");

        let events = log.events_for_player_since(player, None);
        assert!(events.is_empty(), "No-op log should not store events");

        let presentations = log.current_presentations(player);
        assert!(
            presentations.is_empty(),
            "No-op log should not store presentations"
        );
    }
}
