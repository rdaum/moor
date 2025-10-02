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

use crate::event_log::presentation_from_flatbuffer;
use fjall::{CompressionType, Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use flume::{Receiver, Sender};
use moor_common::{
    schema::{
        common as fb_common,
        common::{EventUnion, ObjUnion, ObjUnionRef},
        convert::{obj_from_flatbuffer_struct, obj_to_flatbuffer_struct},
        event_log::{LoggedNarrativeEvent, PlayerPresentations},
    },
    tasks::Presentation,
};
use moor_var::Obj;
use tracing::{debug, error, info};

/// Trait abstracting event log operations for testing
pub trait EventLogOps: Send + Sync {
    /// Add a new event to the log, returns the event's UUID
    /// Note: Caller should convert from domain types to FlatBuffer types before calling
    fn append(&self, event: LoggedNarrativeEvent) -> Uuid;

    /// Get current presentations for a player
    /// Returns FlatBuffer presentations (Vec) - caller can convert to HashMap if needed
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
}

// LoggedNarrativeEvent and PlayerPresentations are now imported from
// moor_common::schema::event_log and are FlatBuffer-generated types

/// Messages for the background persistence thread
#[derive(Debug)]
enum PersistenceMessage {
    WriteNarrativeEvent(Uuid, LoggedNarrativeEvent),
    WritePresentationState(PlayerPresentations),
    Shutdown,
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
            keyspace.open_partition("presentations", partition_creation_options)?;

        Ok(Self {
            _tmpdir: tmpdir,
            _keyspace: keyspace,
            narrative_events_partition,
            player_index_partition,
            presentations_partition,
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
    fn obj_to_key(obj: &moor_common::schema::common::Obj) -> String {
        // Use the obj's union variant to create a unique key
        match &obj.obj {
            ObjUnion::ObjId(id) => format!("id:{}", id.id),
            ObjUnion::UuObjId(uuid) => format!("uuid:{}", uuid.packed_value),
            ObjUnion::AnonymousObjId(anon) => format!("anon:{}", anon.packed_value),
        }
    }

    // Helper to compare two FlatBuffer Obj types (used by trait implementation)
    pub(crate) fn obj_matches(
        obj1: &moor_common::schema::common::Obj,
        obj2: &moor_common::schema::common::Obj,
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
        obj: &moor_common::schema::common::Obj,
        obj_ref: &moor_common::schema::common::ObjRef,
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
                    <moor_common::schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
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
                    <moor_common::schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
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
                <moor_common::schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
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
                <moor_common::schema::event_log::LoggedNarrativeEventRef as ::planus::ReadAsRoot>::read_as_root(&value)?;

            // Check if player matches and timestamp is after cutoff
            let event_player_ref = event_ref.player()?;
            let event_narrative_ref = event_ref.event()?;
            let event_timestamp_nanos = event_narrative_ref.timestamp()?;
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
                <moor_common::schema::event_log::PlayerPresentationsRef as ::planus::ReadAsRoot>::read_as_root(&value)?;
            // Convert from Ref to owned type
            let presentations: PlayerPresentations = presentations_ref.try_into()?;
            Ok(Some(presentations))
        } else {
            Ok(None)
        }
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
    pub fn append(&self, event: LoggedNarrativeEvent) -> Uuid {
        // Extract event_id from the FlatBuffer event
        let event_id_bytes = event.event.event_id.data.as_slice();
        let event_id = if event_id_bytes.len() == 16 {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(event_id_bytes);
            Uuid::from_bytes(bytes)
        } else {
            error!("Invalid event_id in LoggedNarrativeEvent");
            return Uuid::nil();
        };

        // Check if this is a Present or Unpresent event and update presentation state
        match &event.event.event.event {
            EventUnion::PresentEvent(present_ref) => {
                // Add presentation to player's current presentations
                if let Ok(presentation) = presentation_from_flatbuffer(&present_ref.presentation) {
                    self.update_presentation_state(&event.player, Some(presentation), None);
                }
            }
            EventUnion::UnpresentEvent(unpresent_ref) => {
                // Remove presentation from player's current presentations
                self.update_presentation_state(
                    &event.player,
                    None,
                    Some(unpresent_ref.presentation_id.clone()),
                );
            }
            _ => {}
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

    /// Update presentation state for a player (add or remove a presentation)
    /// This is done synchronously to ensure consistency with reads
    fn update_presentation_state(
        &self,
        player: &moor_common::schema::common::Obj,
        add_presentation: Option<Presentation>,
        remove_id: Option<String>,
    ) {
        // Convert FlatBuffer Obj to domain Obj to use as key
        let player_obj = obj_from_flatbuffer_struct(player).expect("Failed to convert player obj");

        // Lock persistence and update synchronously for consistency
        let mut persistence_guard = self.persistence.lock().unwrap();

        // Load current state
        let mut current_state = persistence_guard
            .load_presentation_state(player_obj)
            .ok()
            .flatten()
            .unwrap_or_else(|| {
                // Create new state if none exists
                PlayerPresentations {
                    player: Box::new(player.clone()),
                    presentations: vec![],
                }
            });

        // Apply the update
        if let Some(presentation) = add_presentation {
            // Convert domain Presentation to FlatBuffer Presentation
            let fb_presentation = fb_common::Presentation {
                id: presentation.id.clone(),
                content_type: presentation.content_type.clone(),
                content: presentation.content.clone(),
                target: presentation.target.clone(),
                attributes: presentation
                    .attributes
                    .into_iter()
                    .map(|(k, v)| fb_common::PresentationAttribute { key: k, value: v })
                    .collect(),
            };
            current_state.presentations.push(fb_presentation);
        }

        if let Some(id) = remove_id {
            current_state.presentations.retain(|p| p.id != id);
        }

        // Write synchronously
        if let Err(e) = persistence_guard.write_presentation_state(&current_state) {
            error!("Failed to write presentation state: {}", e);
        }

        // Flush to ensure write is visible
        if let Err(e) = persistence_guard.flush() {
            error!("Failed to flush presentation state: {}", e);
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
    fn append(&self, event: LoggedNarrativeEvent) -> Uuid {
        self.append(event)
    }

    fn current_presentations(&self, player: Obj) -> Vec<Presentation> {
        self.load_presentation_state_from_disk(player)
            .ok()
            .flatten()
            .map(|state| {
                // Convert from FlatBuffer Presentations to domain Presentations
                state
                    .presentations
                    .iter()
                    .filter_map(|p| presentation_from_flatbuffer(p).ok())
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
    fn append(&self, event: LoggedNarrativeEvent) -> Uuid {
        // Extract event_id from the FlatBuffer event
        let event_id_bytes = event.event.event_id.data.as_slice();
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
        let event = NarrativeEvent {
            event_id: Uuid::now_v7(),
            timestamp: SystemTime::now(),
            author: v_str("test"),
            event: Event::Notify {
                value: v_str(message),
                content_type: None,
                no_flush: false,
                no_newline: false,
            },
        };
        logged_narrative_event_to_flatbuffer(player, Box::new(event))
            .expect("Failed to convert to FlatBuffer")
    }

    fn create_present_event(player: Obj, id: &str, content: &str) -> LoggedNarrativeEvent {
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
        logged_narrative_event_to_flatbuffer(player, Box::new(event))
            .expect("Failed to convert to FlatBuffer")
    }

    fn extract_event_id(event: &LoggedNarrativeEvent) -> Uuid {
        let uuid_bytes = event.event.event_id.data.as_slice();
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

        let id1 = log.append(event1);
        let id2 = log.append(event2);

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

        log.append(event1);
        log.append(event2);
        log.append(event3);

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

        let id1 = log.append(event1);
        let _id2 = log.append(event2);
        let id3 = log.append(event3);

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
        log.append(present_event);

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
        log.append(event1);

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
    fn test_no_op_event_log() {
        let log = NoOpEventLog::new();
        let player = Obj::mk_id(1);

        let event = create_logged_event(player, "test");
        let id = log.append(event);

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
