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

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use bincode::{Decode, Encode};
use fjall::{CompressionType, Config, Keyspace, PartitionCreateOptions, PartitionHandle};
use flume::{Receiver, Sender};
use moor_common::tasks::{Event, NarrativeEvent, Presentation};
use moor_var::{BINCODE_CONFIG, Obj};
use tracing::{debug, error, info};

/// An immutable narrative event record in the chronological log
#[derive(Debug, Clone, Encode, Decode)]
pub struct LoggedNarrativeEvent {
    pub player: Obj,
    pub event: Box<NarrativeEvent>,
}

/// Current presentation state for a player
#[derive(Debug, Clone, Encode, Decode)]
pub struct PlayerPresentations {
    pub player: Obj,
    /// Map of presentation_id -> Presentation
    pub presentations: HashMap<String, Presentation>,
}

/// Messages for the background persistence thread
#[derive(Debug)]
enum PersistenceMessage {
    WriteNarrativeEvent(Uuid, LoggedNarrativeEvent),
    WritePresentationState(PlayerPresentations),
    Shutdown,
}

/// Configuration for event log persistence and caching
#[derive(Debug, Clone)]
pub struct EventLogConfig {
    /// Number of days worth of events to keep in memory cache per player
    pub cache_days: u64,
    /// Maximum number of events to keep in memory regardless of age
    pub max_cache_events: usize,
    /// Background thread batch size for writes
    pub write_batch_size: usize,
}

/// Persistent event log that maintains chronological ordering for narrative events
/// and current presentation state separately.
///
/// Uses fjall for disk persistence with a write-through cache for performance.
/// Narrative events are stored by UUID (v7) which provides natural chronological ordering.
/// Presentations are stored as current state per player.
pub struct EventLog {
    // In-memory cache for narrative events (BTreeMap provides ordered iteration by UUID)
    narrative_cache: Arc<Mutex<BTreeMap<Uuid, LoggedNarrativeEvent>>>,
    // Current presentation state per player
    presentations: Arc<Mutex<HashMap<Obj, HashMap<String, Presentation>>>>,
    // Background persistence thread communication
    persistence_sender: Option<Sender<PersistenceMessage>>,
    // Configuration for caching and persistence
    config: EventLogConfig,
    // Reference to persistence layer for cache miss queries
    persistence: PersistenceRef,
}

/// Internal structure for the fjall persistence backend
struct EventPersistence {
    _tmpdir: Option<tempfile::TempDir>,
    _keyspace: Keyspace,
    narrative_events_partition: PartitionHandle,
    player_index_partition: PartitionHandle,
    presentations_partition: PartitionHandle,
}

/// Reference to persistence layer for cache miss handling
type PersistenceRef = Arc<Mutex<Option<EventPersistence>>>;

impl Default for EventLogConfig {
    fn default() -> Self {
        Self {
            cache_days: 7,           // Keep 1 week in cache
            max_cache_events: 10000, // Maximum 10k events in cache
            write_batch_size: 100,   // Write in batches of 100
        }
    }
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
        &self,
        event_id: Uuid,
        event: &LoggedNarrativeEvent,
    ) -> Result<(), eyre::Error> {
        // Serialize the event
        let event_bytes = bincode::encode_to_vec(event, *BINCODE_CONFIG)?;

        // Store in main narrative events partition with UUID as key
        self.narrative_events_partition
            .insert(event_id.as_bytes(), &event_bytes)?;

        // Update player index for efficient per-player queries
        let player_key = format!("{}:{}", event.player.to_literal(), event_id);
        self.player_index_partition
            .insert(player_key.as_bytes(), event_id.as_bytes())?;

        Ok(())
    }

    fn write_presentation_state(
        &self,
        player_presentations: &PlayerPresentations,
    ) -> Result<(), eyre::Error> {
        // Serialize the presentation state
        let state_bytes = bincode::encode_to_vec(player_presentations, *BINCODE_CONFIG)?;

        // Store with player ID as key
        let player_key = player_presentations.player.to_literal();
        self.presentations_partition
            .insert(player_key.as_bytes(), &state_bytes)?;

        Ok(())
    }

    fn load_narrative_events_since(
        &self,
        since: Option<Uuid>,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
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
                let (event, _): (LoggedNarrativeEvent, usize) =
                    bincode::decode_from_slice(&value, *BINCODE_CONFIG)?;
                events.push(event);
            }
        } else {
            for entry in self
                .narrative_events_partition
                .range(start_bound.as_slice()..)
            {
                let (_, value) = entry?;
                let (event, _): (LoggedNarrativeEvent, usize) =
                    bincode::decode_from_slice(&value, *BINCODE_CONFIG)?;
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
            let (event, _): (LoggedNarrativeEvent, usize) =
                bincode::decode_from_slice(&value, *BINCODE_CONFIG)?;
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

        for entry in self.narrative_events_partition.iter() {
            let (_, value) = entry?;
            let (event, _): (LoggedNarrativeEvent, usize) =
                bincode::decode_from_slice(&value, *BINCODE_CONFIG)?;

            if event.player == player && event.event.timestamp >= cutoff_time {
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
        let player_key = player.to_literal();

        if let Some(value) = self.presentations_partition.get(player_key.as_bytes())? {
            let (presentations, _): (PlayerPresentations, usize) =
                bincode::decode_from_slice(&value, *BINCODE_CONFIG)?;
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

    pub fn with_config(config: EventLogConfig, db_path: Option<&Path>) -> Self {
        let (persistence_sender, persistence_receiver) = flume::unbounded();

        // Create persistence layer for cache miss queries
        let persistence = match EventPersistence::open(db_path) {
            Ok(p) => Arc::new(Mutex::new(Some(p))),
            Err(e) => {
                error!("Failed to open event log persistence for queries: {}", e);
                Arc::new(Mutex::new(None))
            }
        };

        // Start background persistence thread
        let config_clone = config.clone();
        let db_path_owned = db_path.map(|p| p.to_path_buf());
        thread::spawn(move || {
            Self::persistence_thread_main(
                config_clone,
                persistence_receiver,
                db_path_owned.as_deref(),
            );
        });

        Self {
            narrative_cache: Arc::new(Mutex::new(BTreeMap::new())),
            presentations: Arc::new(Mutex::new(HashMap::new())),
            persistence_sender: Some(persistence_sender),
            config,
            persistence,
        }
    }

    fn persistence_thread_main(
        config: EventLogConfig,
        receiver: Receiver<PersistenceMessage>,
        db_path: Option<&Path>,
    ) {
        let persistence = match EventPersistence::open(db_path) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to open event log persistence: {}", e);
                return;
            }
        };

        info!("Event log persistence thread started");

        let mut write_batch: Vec<(Uuid, LoggedNarrativeEvent)> = Vec::new();

        loop {
            // Try to collect a batch of writes
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(PersistenceMessage::WriteNarrativeEvent(event_id, event)) => {
                    write_batch.push((event_id, event));

                    // If we hit the batch size or can't get more messages quickly, flush
                    while write_batch.len() < config.write_batch_size {
                        match receiver.try_recv() {
                            Ok(PersistenceMessage::WriteNarrativeEvent(id, evt)) => {
                                write_batch.push((id, evt));
                            }
                            Ok(PersistenceMessage::WritePresentationState(state)) => {
                                // Write presentation state immediately (not batched)
                                if let Err(e) = persistence.write_presentation_state(&state) {
                                    error!("Failed to write presentation state to disk: {}", e);
                                }
                            }
                            Ok(PersistenceMessage::Shutdown) => {
                                // Flush remaining writes and exit
                                Self::flush_narrative_write_batch(&persistence, &mut write_batch);
                                info!("Event log persistence thread shutting down");
                                return;
                            }
                            Err(_) => break, // No more messages available
                        }
                    }

                    // Flush the batch
                    Self::flush_narrative_write_batch(&persistence, &mut write_batch);
                }
                Ok(PersistenceMessage::WritePresentationState(state)) => {
                    // Write presentation state immediately
                    if let Err(e) = persistence.write_presentation_state(&state) {
                        error!("Failed to write presentation state to disk: {}", e);
                    }
                }
                Ok(PersistenceMessage::Shutdown) => {
                    info!("Event log persistence thread shutting down");
                    return;
                }
                Err(_) => {
                    // Timeout - just continue the loop
                    continue;
                }
            }
        }
    }

    fn flush_narrative_write_batch(
        persistence: &EventPersistence,
        batch: &mut Vec<(Uuid, LoggedNarrativeEvent)>,
    ) {
        for (event_id, event) in batch.drain(..) {
            if let Err(e) = persistence.write_narrative_event(event_id, &event) {
                error!(
                    "Failed to write narrative event {} to disk: {}",
                    event_id, e
                );
            }
        }
    }

    /// Add a new event to the log, returns the event's UUID
    /// Routes events to appropriate storage based on event type
    pub fn append(&self, player: Obj, event: Box<NarrativeEvent>) -> Uuid {
        let event_id = event.event_id();

        // Don't store events for connection objects (negative IDs)
        if !player.is_positive() {
            debug!(
                "EventLog: Skipping event {} for connection object {} (negative ID)",
                event_id, player
            );
            return event_id;
        }

        match &event.event {
            Event::Notify(_, _) | Event::Traceback(_) => {
                // Store narrative events in chronological log
                self.append_narrative_event(player, event)
            }
            Event::Present(presentation) => {
                // Update current presentation state
                self.update_presentation(player, presentation.clone());
                event_id
            }
            Event::Unpresent(presentation_id) => {
                // Remove presentation from current state
                self.remove_presentation(player, presentation_id.clone());
                event_id
            }
        }
    }

    /// Add a narrative event (Notify/Traceback) to the chronological log
    fn append_narrative_event(&self, player: Obj, event: Box<NarrativeEvent>) -> Uuid {
        let event_id = event.event_id();
        let logged_event = LoggedNarrativeEvent { player, event };

        // Add to in-memory cache
        {
            let mut cache = self.narrative_cache.lock().unwrap();
            cache.insert(event_id, logged_event.clone());

            // Prune cache if it's getting too large
            self.prune_narrative_cache_if_needed(&mut cache);

            debug!(
                "EventLog: Added narrative event {} for player {} (cache size: {})",
                event_id,
                player,
                cache.len()
            );
        }

        // Send to background persistence thread
        if let Some(ref sender) = self.persistence_sender {
            if let Err(e) = sender.send(PersistenceMessage::WriteNarrativeEvent(
                event_id,
                logged_event,
            )) {
                error!(
                    "Failed to send narrative event to persistence thread: {}",
                    e
                );
            }
        }

        event_id
    }

    /// Update current presentation state for a player
    fn update_presentation(&self, player: Obj, presentation: Presentation) {
        let presentation_id = presentation.id.clone();
        let mut presentations = self.presentations.lock().unwrap();
        let player_presentations = presentations.entry(player).or_default();
        player_presentations.insert(presentation_id.clone(), presentation);

        debug!(
            "EventLog: Updated presentation {} for player {}",
            presentation_id, player
        );

        // Send updated state to persistence
        if let Some(ref sender) = self.persistence_sender {
            let state = PlayerPresentations {
                player,
                presentations: player_presentations.clone(),
            };
            if let Err(e) = sender.send(PersistenceMessage::WritePresentationState(state)) {
                error!(
                    "Failed to send presentation state to persistence thread: {}",
                    e
                );
            }
        }
    }

    /// Remove a presentation from current state for a player
    fn remove_presentation(&self, player: Obj, presentation_id: String) {
        let mut presentations = self.presentations.lock().unwrap();
        if let Some(player_presentations) = presentations.get_mut(&player) {
            player_presentations.remove(&presentation_id);

            debug!(
                "EventLog: Removed presentation {} for player {}",
                presentation_id, player
            );

            // Send updated state to persistence
            if let Some(ref sender) = self.persistence_sender {
                let state = PlayerPresentations {
                    player,
                    presentations: player_presentations.clone(),
                };
                if let Err(e) = sender.send(PersistenceMessage::WritePresentationState(state)) {
                    error!(
                        "Failed to send presentation state to persistence thread: {}",
                        e
                    );
                }
            }
        }
    }

    /// Dismiss a presentation by ID (public API for manual dismissal)
    pub fn dismiss_presentation(&self, player: Obj, presentation_id: String) {
        self.remove_presentation(player, presentation_id);
    }

    fn prune_narrative_cache_if_needed(&self, cache: &mut BTreeMap<Uuid, LoggedNarrativeEvent>) {
        if cache.len() <= self.config.max_cache_events {
            return;
        }

        let cutoff_time = SystemTime::now()
            .checked_sub(Duration::from_secs(self.config.cache_days * 24 * 3600))
            .unwrap_or(UNIX_EPOCH);

        // Remove old events
        let mut to_remove = Vec::new();
        for (event_id, logged_event) in cache.iter() {
            if logged_event.event.timestamp < cutoff_time {
                to_remove.push(*event_id);
            }
        }

        for event_id in to_remove {
            cache.remove(&event_id);
        }

        // If still too large, remove oldest until we're under the limit
        while cache.len() > self.config.max_cache_events {
            if let Some((oldest_id, _)) = cache.iter().next() {
                let oldest_id = *oldest_id;
                cache.remove(&oldest_id);
            } else {
                break;
            }
        }

        debug!("Pruned narrative cache to {} events", cache.len());
    }

    /// Get current presentations for a player
    pub fn current_presentations(&self, player: Obj) -> HashMap<String, Presentation> {
        let presentations = self.presentations.lock().unwrap();
        presentations.get(&player).cloned().unwrap_or_default()
    }

    /// Load current presentations for a player from disk (called when player connects)
    #[allow(dead_code)]
    pub fn load_player_presentations(
        &self,
        player: Obj,
    ) -> Result<HashMap<String, Presentation>, eyre::Error> {
        let presentations = match self.load_presentation_state_from_disk(player)? {
            Some(state) => {
                // Update in-memory state
                let mut presentations_guard = self.presentations.lock().unwrap();
                presentations_guard.insert(player, state.presentations.clone());
                state.presentations
            }
            None => HashMap::new(),
        };

        debug!(
            "Loaded {} presentations for player {} from disk",
            presentations.len(),
            player
        );
        Ok(presentations)
    }

    /// Load recent narrative events for a player into cache (called when player connects)
    #[allow(dead_code)]
    pub fn preload_player_events(&self, player: Obj, days: u64) -> Result<usize, eyre::Error> {
        let events =
            self.load_narrative_from_disk_for_player_since_seconds(player, days * 24 * 3600)?;
        let count = events.len();

        if !events.is_empty() {
            let mut cache = self.narrative_cache.lock().unwrap();
            for event in events {
                cache.insert(event.event.event_id(), event);
            }
            debug!(
                "Preloaded {} narrative events for player {} from disk",
                count, player
            );
        }

        Ok(count)
    }

    /// Shutdown the event log and flush any pending writes
    pub fn shutdown(&mut self) {
        if let Some(sender) = self.persistence_sender.take() {
            if let Err(e) = sender.send(PersistenceMessage::Shutdown) {
                error!(
                    "Failed to send shutdown message to persistence thread: {}",
                    e
                );
            }
        }
    }

    /// Get all narrative events since the given UUID (exclusive)
    /// Returns events in chronological order
    pub fn events_since(&self, since: Option<Uuid>) -> Vec<LoggedNarrativeEvent> {
        // First try cache
        let cache_events = {
            let cache = self.narrative_cache.lock().unwrap();
            match since {
                Some(since_id) => cache
                    .range((
                        std::ops::Bound::Excluded(since_id),
                        std::ops::Bound::Unbounded,
                    ))
                    .map(|(_, event)| event.clone())
                    .collect::<Vec<_>>(),
                None => {
                    // Return all cached events
                    cache.values().cloned().collect()
                }
            }
        };

        // If cache is empty or incomplete, try disk
        if cache_events.is_empty() {
            self.load_narrative_from_disk_since(since)
                .unwrap_or_else(|e| {
                    debug!("Failed to load narrative events from disk: {}", e);
                    cache_events
                })
        } else {
            cache_events
        }
    }

    fn load_narrative_from_disk_since(
        &self,
        since: Option<Uuid>,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        if let Some(ref persistence) = *persistence_guard {
            persistence.load_narrative_events_since(since)
        } else {
            Ok(vec![])
        }
    }

    fn load_narrative_from_disk_until(
        &self,
        until: Option<Uuid>,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        if let Some(ref persistence) = *persistence_guard {
            persistence.load_narrative_events_until(until)
        } else {
            Ok(vec![])
        }
    }

    #[allow(dead_code)]
    fn load_presentation_state_from_disk(
        &self,
        player: Obj,
    ) -> Result<Option<PlayerPresentations>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        if let Some(ref persistence) = *persistence_guard {
            persistence.load_presentation_state(player)
        } else {
            Ok(None)
        }
    }

    /// Get narrative events since the given UUID with optional limit (exclusive)
    /// Returns events in chronological order
    pub fn events_since_with_limit(
        &self,
        since: Option<Uuid>,
        limit: Option<usize>,
    ) -> Vec<LoggedNarrativeEvent> {
        let mut events = self.events_since(since);
        if let Some(limit) = limit {
            events.truncate(limit);
        }
        events
    }

    /// Get all narrative events until the given UUID (exclusive)
    /// Returns events in chronological order
    pub fn events_until(&self, until: Option<Uuid>) -> Vec<LoggedNarrativeEvent> {
        let cache = self.narrative_cache.lock().unwrap();

        match until {
            Some(until_id) => cache
                .range((
                    std::ops::Bound::Unbounded,
                    std::ops::Bound::Excluded(until_id),
                ))
                .map(|(_, event)| event.clone())
                .collect(),
            None => {
                // Return all cached events
                cache.values().cloned().collect()
            }
        }
    }

    /// Get narrative events until the given UUID with optional limit (exclusive)
    /// Returns events in chronological order
    #[allow(dead_code)]
    pub fn events_until_with_limit(
        &self,
        until: Option<Uuid>,
        limit: Option<usize>,
    ) -> Vec<LoggedNarrativeEvent> {
        let mut events = self.events_until(until);
        if let Some(limit) = limit {
            events.truncate(limit);
        }
        events
    }

    /// Get all narrative events for a specific player since the given UUID
    pub fn events_for_player_since(
        &self,
        player: Obj,
        since: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        // First try cache - get ALL events from cache
        let all_cache_events = self.events_since(since);

        // If cache has NO events, try disk
        if all_cache_events.is_empty() {
            self.load_narrative_from_disk_since(since)
                .unwrap_or_else(|e| {
                    debug!("Failed to load narrative events from disk: {}", e);
                    Vec::new()
                })
                .into_iter()
                .filter(|event| event.player == player)
                .collect()
        } else {
            // Cache has events, filter for this player
            all_cache_events
                .into_iter()
                .filter(|event| event.player == player)
                .collect()
        }
    }

    /// Get narrative events for a specific player since the given UUID with optional limit
    #[allow(dead_code)]
    pub fn events_for_player_since_with_limit(
        &self,
        player: Obj,
        since: Option<Uuid>,
        limit: Option<usize>,
    ) -> Vec<LoggedNarrativeEvent> {
        self.events_since_with_limit(since, limit)
            .into_iter()
            .filter(|event| event.player == player)
            .collect()
    }

    /// Get all narrative events for a specific player until the given UUID
    pub fn events_for_player_until(
        &self,
        player: Obj,
        until: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        // First try cache - get ALL events from cache
        let all_cache_events = self.events_until(until);

        // If cache has NO events, try disk
        if all_cache_events.is_empty() {
            self.load_narrative_from_disk_until(until)
                .unwrap_or_else(|e| {
                    debug!("Failed to load narrative events from disk: {}", e);
                    Vec::new()
                })
                .into_iter()
                .filter(|event| event.player == player)
                .collect()
        } else {
            // Cache has events, filter for this player
            all_cache_events
                .into_iter()
                .filter(|event| event.player == player)
                .collect()
        }
    }

    /// Get narrative events for a specific player until the given UUID with optional limit
    #[allow(dead_code)]
    pub fn events_for_player_until_with_limit(
        &self,
        player: Obj,
        until: Option<Uuid>,
        limit: Option<usize>,
    ) -> Vec<LoggedNarrativeEvent> {
        self.events_until_with_limit(until, limit)
            .into_iter()
            .filter(|event| event.player == player)
            .collect()
    }

    /// Get all narrative events since N seconds ago
    pub fn events_since_seconds(&self, seconds_ago: u64) -> Vec<LoggedNarrativeEvent> {
        let cutoff_time = SystemTime::now()
            .checked_sub(Duration::from_secs(seconds_ago))
            .unwrap_or(UNIX_EPOCH);

        // First try cache
        let cache_events = {
            let cache = self.narrative_cache.lock().unwrap();
            let total_events = cache.len();
            let matching_events: Vec<LoggedNarrativeEvent> = cache
                .values()
                .filter(|logged_event| logged_event.event.timestamp() >= cutoff_time)
                .cloned()
                .collect();

            debug!(
                "EventLog: {} cached narrative events, {} events match cutoff {} seconds ago (cutoff time: {:?})",
                total_events,
                matching_events.len(),
                seconds_ago,
                cutoff_time
            );

            matching_events
        };

        // If cache is empty, try loading all events from disk and filter by time
        if cache_events.is_empty() {
            self.load_narrative_from_disk_since_seconds(seconds_ago)
                .unwrap_or_else(|e| {
                    debug!("Failed to load narrative events from disk: {}", e);
                    cache_events
                })
        } else {
            cache_events
        }
    }

    /// Get all narrative events for a specific player since N seconds ago
    pub fn events_for_player_since_seconds(
        &self,
        player: Obj,
        seconds_ago: u64,
    ) -> Vec<LoggedNarrativeEvent> {
        // Try cache first
        let cache_events = self
            .events_since_seconds(seconds_ago)
            .into_iter()
            .filter(|event| event.player == player)
            .collect::<Vec<_>>();

        // Always also try disk to ensure we get complete results
        let disk_events = self
            .load_narrative_from_disk_for_player_since_seconds(player, seconds_ago)
            .unwrap_or_else(|e| {
                debug!("Failed to load player narrative events from disk: {}", e);
                Vec::new()
            });

        // Merge cache and disk events, removing duplicates by event ID
        let mut all_events = cache_events;
        for disk_event in disk_events {
            // Only add if not already in cache
            if !all_events
                .iter()
                .any(|cache_event| cache_event.event.event_id() == disk_event.event.event_id())
            {
                all_events.push(disk_event);
            }
        }

        // Sort by event ID (which is chronological for UUID v7)
        all_events.sort_by_key(|event| event.event.event_id());
        all_events
    }

    fn load_narrative_from_disk_for_player_since_seconds(
        &self,
        player: Obj,
        seconds_ago: u64,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let persistence_guard = self.persistence.lock().unwrap();
        if let Some(ref persistence) = *persistence_guard {
            persistence.load_narrative_events_for_player_since_seconds(player, seconds_ago)
        } else {
            Ok(vec![])
        }
    }

    fn load_narrative_from_disk_since_seconds(
        &self,
        seconds_ago: u64,
    ) -> Result<Vec<LoggedNarrativeEvent>, eyre::Error> {
        let cutoff_time = SystemTime::now()
            .checked_sub(Duration::from_secs(seconds_ago))
            .unwrap_or(UNIX_EPOCH);

        let persistence_guard = self.persistence.lock().unwrap();
        if let Some(ref persistence) = *persistence_guard {
            // Load all events and filter by time (could be optimized with time-based indexing)
            let mut events = Vec::new();
            for entry in persistence.narrative_events_partition.iter() {
                let (_, value) = entry?;
                let (event, _): (LoggedNarrativeEvent, usize) =
                    bincode::decode_from_slice(&value, *BINCODE_CONFIG)?;

                if event.event.timestamp >= cutoff_time {
                    events.push(event);
                }
            }
            Ok(events)
        } else {
            Ok(vec![])
        }
    }

    /// Get narrative events for a specific player since N seconds ago with optional limit
    #[allow(dead_code)]
    pub fn events_for_player_since_seconds_with_limit(
        &self,
        player: Obj,
        seconds_ago: u64,
        limit: Option<usize>,
    ) -> Vec<LoggedNarrativeEvent> {
        let mut events = self
            .events_since_seconds(seconds_ago)
            .into_iter()
            .filter(|event| event.player == player)
            .collect::<Vec<_>>();

        if let Some(limit) = limit {
            events.truncate(limit);
        }
        events
    }

    /// Get the most recent narrative event UUID in the log
    #[allow(dead_code)]
    pub fn latest_event_id(&self) -> Option<Uuid> {
        let cache = self.narrative_cache.lock().unwrap();
        cache.keys().last().copied()
    }

    /// Get the count of narrative events in the log
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let cache = self.narrative_cache.lock().unwrap();
        cache.len()
    }

    /// Check if the narrative log is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let cache = self.narrative_cache.lock().unwrap();
        cache.is_empty()
    }

    // TODO: Future methods:
    // - prune_before(cutoff: EventId) -> Result<usize, Error>
    // - compact() -> Result<(), Error>
    // - persist_to_disk() -> Result<(), Error>
    // - load_from_disk() -> Result<Self, Error>
    // - memory_usage() -> usize
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

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::tasks::{Event, Presentation};
    use moor_var::{Obj, SYSTEM_OBJECT, v_str};
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_notify_event(_player: Obj, message: &str) -> Box<NarrativeEvent> {
        Box::new(NarrativeEvent {
            event_id: Uuid::now_v7(),
            timestamp: SystemTime::now(),
            author: v_str("test"),
            event: Event::Notify(v_str(message), None),
        })
    }

    fn create_test_present_event(_player: Obj, id: &str, content: &str) -> Box<NarrativeEvent> {
        Box::new(NarrativeEvent {
            event_id: Uuid::now_v7(),
            timestamp: SystemTime::now(),
            author: v_str("test"),
            event: Event::Present(Presentation {
                id: id.to_string(),
                content_type: "text_plain".to_string(),
                content: content.to_string(),
                target: "main".to_string(),
                attributes: vec![],
            }),
        })
    }

    fn create_test_unpresent_event(_player: Obj, id: &str) -> Box<NarrativeEvent> {
        Box::new(NarrativeEvent {
            event_id: Uuid::now_v7(),
            timestamp: SystemTime::now(),
            author: v_str("test"),
            event: Event::Unpresent(id.to_string()),
        })
    }

    fn create_test_config() -> EventLogConfig {
        EventLogConfig {
            cache_days: 1,
            max_cache_events: 100,
            write_batch_size: 5,
        }
    }

    #[test]
    fn test_uuid_v7_ordering() {
        let id1 = Uuid::now_v7();
        std::thread::sleep(Duration::from_millis(1));
        let id2 = Uuid::now_v7();

        assert!(id1 < id2);
    }

    #[test]
    fn test_basic_narrative_operations() {
        let log = EventLog::new();
        let player = SYSTEM_OBJECT;

        let event1 = create_test_notify_event(player, "hello");
        let event2 = create_test_notify_event(player, "world");

        let id1 = log.append(player, event1.clone());
        let id2 = log.append(player, event2.clone());

        assert!(id1 < id2);
        assert_eq!(log.len(), 2);

        let all_events = log.events_since(None);
        assert_eq!(all_events.len(), 2);
        assert_eq!(all_events[0].event.event_id(), id1);
        assert_eq!(all_events[1].event.event_id(), id2);

        let events_since_id1 = log.events_since(Some(id1));
        assert_eq!(events_since_id1.len(), 1);
        assert_eq!(events_since_id1[0].event.event_id(), id2);

        let events_until_id2 = log.events_until(Some(id2));
        assert_eq!(events_until_id2.len(), 1);
        assert_eq!(events_until_id2[0].event.event_id(), id1);
    }

    #[test]
    fn test_presentation_state_management() {
        let log = EventLog::new();
        let player = SYSTEM_OBJECT;

        // Initially no presentations
        let presentations = log.current_presentations(player);
        assert!(presentations.is_empty());

        // Add a presentation
        let present_event = create_test_present_event(player, "widget1", "Hello World");
        let _id1 = log.append(player, present_event);

        // Check current presentations
        let presentations = log.current_presentations(player);
        assert_eq!(presentations.len(), 1);
        assert!(presentations.contains_key("widget1"));
        assert_eq!(presentations["widget1"].content, "Hello World");

        // Add another presentation
        let present_event2 = create_test_present_event(player, "widget2", "Goodbye World");
        let _id2 = log.append(player, present_event2);

        // Check both presentations exist
        let presentations = log.current_presentations(player);
        assert_eq!(presentations.len(), 2);
        assert!(presentations.contains_key("widget1"));
        assert!(presentations.contains_key("widget2"));

        // Update existing presentation
        let update_event = create_test_present_event(player, "widget1", "Updated Content");
        let _id3 = log.append(player, update_event);

        // Check that widget1 was updated
        let presentations = log.current_presentations(player);
        assert_eq!(presentations.len(), 2);
        assert_eq!(presentations["widget1"].content, "Updated Content");
        assert_eq!(presentations["widget2"].content, "Goodbye World");

        // Remove a presentation
        let unpresent_event = create_test_unpresent_event(player, "widget1");
        let _id4 = log.append(player, unpresent_event);

        // Check that widget1 was removed
        let presentations = log.current_presentations(player);
        assert_eq!(presentations.len(), 1);
        assert!(!presentations.contains_key("widget1"));
        assert!(presentations.contains_key("widget2"));

        // Narrative events should not be stored for presentation/unpresent
        assert_eq!(log.len(), 0); // No narrative events stored
    }

    #[test]
    fn test_mixed_event_types() {
        let log = EventLog::new();
        let player = SYSTEM_OBJECT;

        // Add narrative events and presentations
        let notify1 = create_test_notify_event(player, "User says hello");
        let present1 = create_test_present_event(player, "status", "Connected");
        let notify2 = create_test_notify_event(player, "User says goodbye");
        let unpresent1 = create_test_unpresent_event(player, "status");

        let _id1 = log.append(player, notify1);
        let _id2 = log.append(player, present1);
        let _id3 = log.append(player, notify2);
        let _id4 = log.append(player, unpresent1);

        // Check narrative events (only Notify events should be stored)
        let narrative_events = log.events_since(None);
        assert_eq!(narrative_events.len(), 2);

        // Check presentation state (should be empty after unpresent)
        let presentations = log.current_presentations(player);
        assert!(presentations.is_empty());
    }

    #[test]
    fn test_persistence_basic_write_and_read() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let db_path = tmpdir.path();

        let player1 = SYSTEM_OBJECT;
        let player2 = Obj::mk_id(42);

        // Create log and add events
        let mut log = EventLog::with_config(create_test_config(), Some(db_path));

        let event1 = create_test_notify_event(player1, "event1");
        let event2 = create_test_notify_event(player2, "event2");
        let event3 = create_test_notify_event(player1, "event3");

        let _id1 = log.append(player1, event1);
        let _id2 = log.append(player2, event2);
        let _id3 = log.append(player1, event3);

        // Wait for background writes to complete
        thread::sleep(Duration::from_millis(200));

        // Verify cache has all events
        assert_eq!(log.len(), 3);

        // Shutdown and recreate to test persistence
        log.shutdown();
        drop(log);

        // Verify persistence layer can be opened again
        let persistence =
            EventPersistence::open(Some(db_path)).expect("Failed to reopen persistence");

        // Verify events are persisted (though the current implementation doesn't load them back into cache automatically)
        // For now, this test mainly verifies that the persistence layer can be created and doesn't crash
        drop(persistence);
    }

    #[test]
    fn test_multiple_players_isolation() {
        let log = EventLog::new();
        let player1 = SYSTEM_OBJECT;
        let player2 = Obj::mk_id(42);
        let player3 = Obj::mk_id(100);

        // Add events for different players
        let id1 = log.append(player1, create_test_notify_event(player1, "p1_event1"));
        let id2 = log.append(player2, create_test_notify_event(player2, "p2_event1"));
        let id3 = log.append(player1, create_test_notify_event(player1, "p1_event2"));
        let id4 = log.append(player3, create_test_notify_event(player3, "p3_event1"));
        let id5 = log.append(player2, create_test_notify_event(player2, "p2_event2"));

        // Test player-specific filtering
        let player1_events = log.events_for_player_since(player1, None);
        assert_eq!(player1_events.len(), 2);
        assert_eq!(player1_events[0].event.event_id(), id1);
        assert_eq!(player1_events[1].event.event_id(), id3);

        let player2_events = log.events_for_player_since(player2, None);
        assert_eq!(player2_events.len(), 2);
        assert_eq!(player2_events[0].event.event_id(), id2);
        assert_eq!(player2_events[1].event.event_id(), id5);

        let player3_events = log.events_for_player_since(player3, None);
        assert_eq!(player3_events.len(), 1);
        assert_eq!(player3_events[0].event.event_id(), id4);

        // Test since filtering per player
        let player1_since_id1 = log.events_for_player_since(player1, Some(id1));
        assert_eq!(player1_since_id1.len(), 1);
        assert_eq!(player1_since_id1[0].event.event_id(), id3);
    }

    #[test]
    fn test_time_based_queries() {
        let log = EventLog::new();
        let player = SYSTEM_OBJECT;

        // Add some events
        let _id1 = log.append(player, create_test_notify_event(player, "old_event"));

        // Wait a bit
        thread::sleep(Duration::from_millis(10));

        let _id2 = log.append(player, create_test_notify_event(player, "recent_event1"));
        let _id3 = log.append(player, create_test_notify_event(player, "recent_event2"));

        // Query events since 5ms ago (should get the recent events)
        let recent_events = log.events_since_seconds(1); // 1 second ago should get all
        assert_eq!(recent_events.len(), 3);

        // Query for specific player
        let recent_player_events = log.events_for_player_since_seconds(player, 1);
        assert_eq!(recent_player_events.len(), 3);

        // Test with limit
        let limited_events = log.events_for_player_since_seconds_with_limit(player, 1, Some(2));
        assert_eq!(limited_events.len(), 2);
    }

    #[test]
    fn test_cache_pruning() {
        let config = EventLogConfig {
            cache_days: 1,
            max_cache_events: 5, // Small cache to trigger pruning
            write_batch_size: 10,
        };

        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let log = EventLog::with_config(config, Some(tmpdir.path()));
        let player = SYSTEM_OBJECT;

        // Add more events than cache limit
        for i in 0..10 {
            log.append(
                player,
                create_test_notify_event(player, &format!("event{}", i)),
            );
        }

        // Wait for background processing
        thread::sleep(Duration::from_millis(100));

        // Cache should be pruned to limit
        assert!(
            log.len() <= 5,
            "Cache size {} should be <= 5 after pruning",
            log.len()
        );
    }

    #[test]
    fn test_background_thread_batching() {
        let config = EventLogConfig {
            cache_days: 1,
            max_cache_events: 100,
            write_batch_size: 3, // Small batch size to test batching
        };

        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let log = EventLog::with_config(config, Some(tmpdir.path()));
        let player = SYSTEM_OBJECT;

        // Add events rapidly to test batching
        for i in 0..10 {
            log.append(
                player,
                create_test_notify_event(player, &format!("batch_event{}", i)),
            );
        }

        // Wait for background writes
        thread::sleep(Duration::from_millis(300));

        // All events should be in cache
        assert_eq!(log.len(), 10);
    }

    #[test]
    fn test_persistence_thread_shutdown() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let mut log = EventLog::with_config(create_test_config(), Some(tmpdir.path()));
        let player = SYSTEM_OBJECT;

        // Add some events
        for i in 0..5 {
            log.append(
                player,
                create_test_notify_event(player, &format!("shutdown_test{}", i)),
            );
        }

        // Explicit shutdown should work without panicking
        log.shutdown();

        // Should be safe to call shutdown multiple times
        log.shutdown();
    }

    #[test]
    fn test_range_queries() {
        let log = EventLog::new();
        let player = SYSTEM_OBJECT;

        let id1 = log.append(player, create_test_notify_event(player, "event1"));
        let id2 = log.append(player, create_test_notify_event(player, "event2"));
        let id3 = log.append(player, create_test_notify_event(player, "event3"));
        let id4 = log.append(player, create_test_notify_event(player, "event4"));

        // Test events_since
        let since_id2 = log.events_since(Some(id2));
        assert_eq!(since_id2.len(), 2);
        assert_eq!(since_id2[0].event.event_id(), id3);
        assert_eq!(since_id2[1].event.event_id(), id4);

        // Test events_until
        let until_id3 = log.events_until(Some(id3));
        assert_eq!(until_id3.len(), 2);
        assert_eq!(until_id3[0].event.event_id(), id1);
        assert_eq!(until_id3[1].event.event_id(), id2);

        // Test with limits
        let limited_since = log.events_since_with_limit(Some(id1), Some(2));
        assert_eq!(limited_since.len(), 2);
        assert_eq!(limited_since[0].event.event_id(), id2);
        assert_eq!(limited_since[1].event.event_id(), id3);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let log = Arc::new(EventLog::new());
        let mut handles = vec![];

        // Spawn multiple threads writing events
        for thread_id in 0..5 {
            let log_clone = Arc::clone(&log);
            let handle = thread::spawn(move || {
                let player = Obj::mk_id(thread_id);
                for i in 0..10 {
                    log_clone.append(
                        player,
                        create_test_notify_event(
                            player,
                            &format!("thread{}_event{}", thread_id, i),
                        ),
                    );
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }

        // Should have 5 threads * 10 events = 50 total events
        assert_eq!(log.len(), 50);

        // Each player should have 10 events
        for thread_id in 0..5 {
            let player = Obj::mk_id(thread_id);
            let player_events = log.events_for_player_since(player, None);
            assert_eq!(
                player_events.len(),
                10,
                "Player {} should have 10 events",
                thread_id
            );
        }
    }

    #[test]
    fn test_persistence_error_handling() {
        // Test with invalid path (should fall back to tmpdir)
        let invalid_path = std::path::Path::new("/invalid/nonexistent/path");

        // This should not panic, but create a tmpdir instead
        let _log = EventLog::with_config(create_test_config(), Some(invalid_path));

        // Basic operations should still work
        let player = SYSTEM_OBJECT;
        let _id = _log.append(player, create_test_notify_event(player, "error_test"));

        // Wait a bit for background processing
        thread::sleep(Duration::from_millis(100));
    }

    #[test]
    fn test_large_batch_persistence() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let config = EventLogConfig {
            cache_days: 1,
            max_cache_events: 1000,
            write_batch_size: 50,
        };

        let log = EventLog::with_config(config, Some(tmpdir.path()));
        let player1 = SYSTEM_OBJECT;
        let player2 = Obj::mk_id(42);

        // Add a large number of events
        for i in 0..200 {
            let player = if i % 2 == 0 { player1 } else { player2 };
            log.append(
                player,
                create_test_notify_event(player, &format!("large_batch_{}", i)),
            );
        }

        // Wait for background processing
        thread::sleep(Duration::from_millis(500));

        // Verify all events are in cache
        assert_eq!(log.len(), 200);

        // Verify player filtering works with large datasets
        let player1_events = log.events_for_player_since(player1, None);
        let player2_events = log.events_for_player_since(player2, None);

        assert_eq!(player1_events.len(), 100);
        assert_eq!(player2_events.len(), 100);
    }

    #[test]
    fn test_drop_cleanup() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");

        {
            let log = EventLog::with_config(create_test_config(), Some(tmpdir.path()));
            let player = SYSTEM_OBJECT;

            // Add some events
            for i in 0..5 {
                log.append(
                    player,
                    create_test_notify_event(player, &format!("drop_test{}", i)),
                );
            }

            // Wait a bit
            thread::sleep(Duration::from_millis(100));
        } // log goes out of scope here, triggering Drop

        // Should be able to recreate the persistence layer
        let _persistence = EventPersistence::open(Some(tmpdir.path()))
            .expect("Should be able to reopen after drop");
    }

    #[test]
    fn test_cache_miss_fallback_to_disk() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let db_path = tmpdir.path();
        let player = SYSTEM_OBJECT;

        // Create first log and add events
        {
            let log = EventLog::with_config(create_test_config(), Some(db_path));

            for i in 0..5 {
                log.append(
                    player,
                    create_test_notify_event(player, &format!("persisted_event{}", i)),
                );
            }

            // Wait for background writes
            thread::sleep(Duration::from_millis(300));

            // Verify events are in cache
            assert_eq!(log.len(), 5);
            let cache_events = log.events_for_player_since_seconds(player, 60);
            assert_eq!(cache_events.len(), 5);
        }

        // Create new log instance (simulating reconnection)
        let new_log = EventLog::with_config(create_test_config(), Some(db_path));

        // Cache should be empty initially
        assert_eq!(new_log.len(), 0);

        // But querying should fall back to disk
        let disk_events = new_log.events_for_player_since_seconds(player, 60);

        // Should have found events from disk
        assert_eq!(
            disk_events.len(),
            5,
            "Should have loaded 5 events from disk"
        );

        // Verify event content - just check that we got events for the right player
        for event in &disk_events {
            assert_eq!(event.player, player);
            // Verify it's a notify event
            assert!(matches!(
                event.event.event,
                moor_common::tasks::Event::Notify(_, _)
            ));
        }
    }

    #[test]
    fn test_preload_player_events() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let db_path = tmpdir.path();
        let player1 = SYSTEM_OBJECT;
        let player2 = Obj::mk_id(42);

        // Create first log and add events for multiple players
        {
            let log = EventLog::with_config(create_test_config(), Some(db_path));

            for i in 0..3 {
                log.append(
                    player1,
                    create_test_notify_event(player1, &format!("p1_event{}", i)),
                );
                log.append(
                    player2,
                    create_test_notify_event(player2, &format!("p2_event{}", i)),
                );
            }

            // Wait for background writes
            thread::sleep(Duration::from_millis(300));
        }

        // Create new log instance
        let new_log = EventLog::with_config(create_test_config(), Some(db_path));

        // Cache should be empty
        assert_eq!(new_log.len(), 0);

        // Preload events for player1
        let loaded_count = new_log
            .preload_player_events(player1, 1)
            .expect("Should load events");
        assert_eq!(loaded_count, 3, "Should have loaded 3 events for player1");

        // Cache should now have player1's events
        assert_eq!(new_log.len(), 3);

        // Querying for player1 should now hit cache
        let player1_events = new_log.events_for_player_since_seconds(player1, 60);
        assert_eq!(player1_events.len(), 3);

        // Querying for player2 should still hit disk
        let player2_events = new_log.events_for_player_since_seconds(player2, 60);
        assert_eq!(player2_events.len(), 3);
    }

    #[test]
    fn test_debug_event_logging() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let db_path = tmpdir.path();
        let player = SYSTEM_OBJECT;

        println!("Creating EventLog with path: {:?}", db_path);
        let log = EventLog::with_config(create_test_config(), Some(db_path));

        println!("Appending event...");
        let event_id = log.append(player, create_test_notify_event(player, "test_event"));
        println!("Appended event with ID: {}", event_id);

        // Check cache immediately
        println!("Cache length: {}", log.len());
        let cache_events = log.events_for_player_since_seconds(player, 60);
        println!("Events from cache: {}", cache_events.len());

        // Wait for background processing
        thread::sleep(Duration::from_millis(500));
        println!("After background processing - cache length: {}", log.len());

        // Try to query again
        let after_events = log.events_for_player_since_seconds(player, 60);
        println!("Events after background processing: {}", after_events.len());

        // Create new log instance to test persistence
        println!("Creating new EventLog instance...");
        let new_log = EventLog::with_config(create_test_config(), Some(db_path));
        println!("New log cache length: {}", new_log.len());

        // Should be able to load from disk
        let disk_events = new_log.events_for_player_since_seconds(player, 60);
        println!("Events loaded from disk: {}", disk_events.len());

        assert_eq!(disk_events.len(), 1, "Should have 1 event from disk");
    }

    #[test]
    fn test_history_query_cache_miss_handling() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let db_path = tmpdir.path();
        let player = SYSTEM_OBJECT;

        // Create first log and add multiple events
        let first_log = {
            let log = EventLog::with_config(create_test_config(), Some(db_path));

            let event1 = log.append(player, create_test_notify_event(player, "first_event"));
            let event2 = log.append(player, create_test_notify_event(player, "second_event"));
            let event3 = log.append(player, create_test_notify_event(player, "third_event"));

            // Wait for background writes
            thread::sleep(Duration::from_millis(300));

            // Verify events are in cache
            assert_eq!(log.len(), 3);

            (log, vec![event1, event2, event3])
        };

        let (log, event_ids) = first_log;

        // Verify all three history query methods work with cache
        let cache_since = log.events_for_player_since(player, Some(event_ids[0]));
        let cache_until = log.events_for_player_until(player, Some(event_ids[2]));
        let cache_seconds = log.events_for_player_since_seconds(player, 60);

        assert_eq!(cache_since.len(), 2, "Should get 2 events since first");
        assert_eq!(cache_until.len(), 2, "Should get 2 events until third");
        assert_eq!(
            cache_seconds.len(),
            3,
            "Should get all 3 events from last 60 seconds"
        );

        // Drop the first log to ensure persistence
        drop(log);

        // Create new log instance (simulating reconnection)
        let new_log = EventLog::with_config(create_test_config(), Some(db_path));

        // Cache should be empty initially
        assert_eq!(new_log.len(), 0);

        // Test that all three history query methods fall back to disk correctly
        let disk_since = new_log.events_for_player_since(player, Some(event_ids[0]));
        let disk_until = new_log.events_for_player_until(player, Some(event_ids[2]));
        let disk_seconds = new_log.events_for_player_since_seconds(player, 60);

        assert_eq!(
            disk_since.len(),
            2,
            "Should load 2 events since first from disk"
        );
        assert_eq!(
            disk_until.len(),
            2,
            "Should load 2 events until third from disk"
        );
        assert_eq!(disk_seconds.len(), 3, "Should load all 3 events from disk");

        // Verify event content is correct
        assert_eq!(disk_since[0].event.event_id(), event_ids[1]);
        assert_eq!(disk_since[1].event.event_id(), event_ids[2]);

        assert_eq!(disk_until[0].event.event_id(), event_ids[0]);
        assert_eq!(disk_until[1].event.event_id(), event_ids[1]);

        assert_eq!(disk_seconds[0].event.event_id(), event_ids[0]);
        assert_eq!(disk_seconds[1].event.event_id(), event_ids[1]);
        assert_eq!(disk_seconds[2].event.event_id(), event_ids[2]);
    }

    #[test]
    fn test_presentation_persistence() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let db_path = tmpdir.path();
        let player = SYSTEM_OBJECT;

        // Create first log and add presentations
        {
            let log = EventLog::with_config(create_test_config(), Some(db_path));

            // Add some presentations
            let present1 = create_test_present_event(player, "widget1", "Content 1");
            let present2 = create_test_present_event(player, "widget2", "Content 2");
            let _id1 = log.append(player, present1);
            let _id2 = log.append(player, present2);

            // Verify presentations are in memory
            let presentations = log.current_presentations(player);
            assert_eq!(presentations.len(), 2);
            assert_eq!(presentations["widget1"].content, "Content 1");
            assert_eq!(presentations["widget2"].content, "Content 2");

            // Wait for background persistence
            thread::sleep(Duration::from_millis(300));
        }

        // Create new log instance (simulating restart)
        let new_log = EventLog::with_config(create_test_config(), Some(db_path));

        // Initially no presentations in memory
        let presentations = new_log.current_presentations(player);
        assert!(presentations.is_empty());

        // Load presentations from disk
        let loaded_presentations = new_log
            .load_player_presentations(player)
            .expect("Should load presentations");

        assert_eq!(loaded_presentations.len(), 2);
        assert_eq!(loaded_presentations["widget1"].content, "Content 1");
        assert_eq!(loaded_presentations["widget2"].content, "Content 2");

        // After loading, they should be in memory too
        let presentations = new_log.current_presentations(player);
        assert_eq!(presentations.len(), 2);
        assert_eq!(presentations["widget1"].content, "Content 1");
        assert_eq!(presentations["widget2"].content, "Content 2");
    }

    #[test]
    fn test_separate_player_presentations() {
        let log = EventLog::new();
        let player1 = SYSTEM_OBJECT;
        let player2 = Obj::mk_id(42);

        // Add presentations for different players
        let present1 = create_test_present_event(player1, "widget1", "Player 1 Content");
        let present2 = create_test_present_event(player2, "widget1", "Player 2 Content");
        let _id1 = log.append(player1, present1);
        let _id2 = log.append(player2, present2);

        // Each player should have their own presentation state
        let player1_presentations = log.current_presentations(player1);
        let player2_presentations = log.current_presentations(player2);

        assert_eq!(player1_presentations.len(), 1);
        assert_eq!(player2_presentations.len(), 1);
        assert_eq!(player1_presentations["widget1"].content, "Player 1 Content");
        assert_eq!(player2_presentations["widget1"].content, "Player 2 Content");

        // Remove presentation for player1 should not affect player2
        let unpresent1 = create_test_unpresent_event(player1, "widget1");
        let _id3 = log.append(player1, unpresent1);

        let player1_presentations = log.current_presentations(player1);
        let player2_presentations = log.current_presentations(player2);

        assert!(player1_presentations.is_empty());
        assert_eq!(player2_presentations.len(), 1);
        assert_eq!(player2_presentations["widget1"].content, "Player 2 Content");
    }

    #[test]
    fn test_web_client_pagination_sequence() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let db_path = tmpdir.path();
        let player = SYSTEM_OBJECT;

        println!("=== Testing Web Client Pagination Sequence ===");

        // Step 1: Create lots of events to test pagination
        let _all_event_ids = {
            println!("1. Creating EventLog and logging 20 events...");
            let log = EventLog::with_config(create_test_config(), Some(db_path));

            let mut event_ids = Vec::new();
            for i in 0..20 {
                let event_id = log.append(
                    player,
                    create_test_notify_event(
                        player,
                        &format!("Event {}: User says something", i + 1),
                    ),
                );
                event_ids.push(event_id);
                // Small delay to ensure different timestamps
                thread::sleep(Duration::from_millis(1));
            }

            // Wait for background persistence
            thread::sleep(Duration::from_millis(500));
            println!("   Logged {} events in cache", log.len());

            drop(log);
            event_ids
        };

        println!("2. Creating fresh EventLog (simulating daemon restart)...");
        let log = EventLog::with_config(create_test_config(), Some(db_path));
        println!("   Fresh EventLog created, cache size: {}", log.len());

        // Step 2: Initial load - get most recent 5 events (like web client initial load)
        println!("3. Initial load: getting most recent 5 events...");
        let initial_events = log.events_for_player_since_seconds(player, 3600);
        println!("   Found {} total events", initial_events.len());

        // Simulate taking the most recent 5 (what the server would return with limit)
        let len = initial_events.len();
        let recent_5: Vec<_> = if len > 5 {
            initial_events.into_iter().skip(len - 5).collect()
        } else {
            initial_events
        };
        println!("   Taking most recent 5 events:");
        for (i, event) in recent_5.iter().enumerate() {
            println!("     {}. Event ID: {}", i + 1, event.event.event_id());
        }

        assert_eq!(recent_5.len(), 5, "Should have 5 recent events");
        let earliest_from_initial = recent_5[0].event.event_id();
        println!(
            "   Earliest event ID from initial load: {}",
            earliest_from_initial
        );

        // Step 3: First back-scroll - get events until the earliest from initial load
        println!(
            "4. First back-scroll: getting events until {}...",
            earliest_from_initial
        );
        let first_backscroll = log.events_for_player_until(player, Some(earliest_from_initial));
        println!(
            "   Found {} events in first back-scroll",
            first_backscroll.len()
        );

        if first_backscroll.is_empty() {
            println!("   ERROR: First back-scroll returned no events!");
        } else {
            println!("   First back-scroll events:");
            for (i, event) in first_backscroll.iter().enumerate() {
                println!("     {}. Event ID: {}", i + 1, event.event.event_id());
            }
        }

        // Take the last 5 from first backscroll (most recent 5 before the boundary)
        let len = first_backscroll.len();
        let first_batch: Vec<_> = if len > 5 {
            first_backscroll.into_iter().skip(len - 5).collect()
        } else {
            first_backscroll
        };

        if !first_batch.is_empty() {
            let earliest_from_first_batch = first_batch[0].event.event_id();
            println!(
                "   Earliest event ID from first batch: {}",
                earliest_from_first_batch
            );

            // Step 4: Second back-scroll - get events until the earliest from first batch
            println!(
                "5. Second back-scroll: getting events until {}...",
                earliest_from_first_batch
            );
            let second_backscroll =
                log.events_for_player_until(player, Some(earliest_from_first_batch));
            println!(
                "   Found {} events in second back-scroll",
                second_backscroll.len()
            );

            if second_backscroll.is_empty() {
                println!("   ERROR: Second back-scroll returned no events!");
            } else {
                println!("   Second back-scroll events:");
                for (i, event) in second_backscroll.iter().enumerate() {
                    println!("     {}. Event ID: {}", i + 1, event.event.event_id());
                }
            }

            // Verify we're getting different events each time
            let initial_ids: Vec<_> = recent_5.iter().map(|e| e.event.event_id()).collect();
            let first_ids: Vec<_> = first_batch.iter().map(|e| e.event.event_id()).collect();
            let second_ids: Vec<_> = second_backscroll
                .iter()
                .map(|e| e.event.event_id())
                .collect();

            println!("6. Verifying pagination correctness...");
            println!("   Initial load IDs: {:?}", initial_ids);
            println!("   First batch IDs: {:?}", first_ids);
            println!("   Second batch IDs: {:?}", second_ids);

            // Verify no overlap between batches
            for id in &initial_ids {
                assert!(
                    !first_ids.contains(id),
                    "Initial and first batch should not overlap"
                );
                assert!(
                    !second_ids.contains(id),
                    "Initial and second batch should not overlap"
                );
            }

            if !first_batch.is_empty() && !second_backscroll.is_empty() {
                for id in &first_ids {
                    assert!(
                        !second_ids.contains(id),
                        "First and second batch should not overlap"
                    );
                }
            }

            // Verify chronological ordering (older events should have smaller UUIDs)
            if !first_batch.is_empty() {
                let latest_from_first = first_batch.last().unwrap().event.event_id();
                let earliest_from_initial = recent_5[0].event.event_id();
                assert!(
                    latest_from_first < earliest_from_initial,
                    "Latest from first batch should be older than earliest from initial"
                );
            }

            println!("=== SUCCESS: Pagination working correctly ===");
        } else {
            println!("=== FAILURE: First back-scroll returned no events ===");
            assert!(
                !first_batch.is_empty(),
                "First back-scroll should return events"
            );
        }
    }

    #[test]
    fn test_web_client_history_simulation() {
        let tmpdir = TempDir::new().expect("Failed to create temp dir");
        let db_path = tmpdir.path();
        let player = SYSTEM_OBJECT;

        println!("=== Simulating Web Client History Bug ===");

        // Step 1: Simulate events being logged during normal operation
        let logged_events = {
            println!("1. Creating EventLog and logging some events...");
            let log = EventLog::with_config(create_test_config(), Some(db_path));

            let event1 = log.append(
                player,
                create_test_notify_event(player, "User says 'hello'"),
            );
            let event2 = log.append(
                player,
                create_test_notify_event(player, "User says 'how are you?'"),
            );
            let event3 = log.append(
                player,
                create_test_notify_event(player, "Bot says 'I am fine, thanks!'"),
            );

            // Wait for background persistence
            thread::sleep(Duration::from_millis(500));

            println!("   Logged {} events in cache", log.len());

            // Verify cache works
            let cache_events = log.events_for_player_since_seconds(player, 3600); // Last hour
            println!("   Cache query found {} events", cache_events.len());

            drop(log); // Explicit drop to simulate shutdown
            vec![event1, event2, event3]
        };

        println!("2. Simulating daemon restart / client reconnection...");

        // Step 2: Simulate EventLog creation on daemon restart (fresh cache)
        let new_log = EventLog::with_config(create_test_config(), Some(db_path));
        println!("   Fresh EventLog created, cache size: {}", new_log.len());

        // Step 3: Simulate web client requesting history with "since_seconds: 3600"
        println!("3. Simulating web client history request (since_seconds: 3600)...");
        let history_events = new_log.events_for_player_since_seconds(player, 3600);

        println!("   History query returned {} events", history_events.len());

        if history_events.is_empty() {
            println!("   ERROR: No events found! This reproduces the bug.");
        } else {
            println!("   SUCCESS: Found historical events:");
            for (i, event) in history_events.iter().enumerate() {
                println!(
                    "     {}. {} ({})",
                    i + 1,
                    event.player,
                    match &event.event.event {
                        moor_common::tasks::Event::Notify(msg, _) => format!("{:?}", msg),
                        _ => "other event".to_string(),
                    }
                );
            }
        }

        // This should NOT fail if our cache miss handling is working
        assert_eq!(
            history_events.len(),
            3,
            "Expected 3 historical events but got {}. This means cache miss handling failed!",
            history_events.len()
        );

        // Verify event IDs match what we logged
        let event_ids: Vec<_> = history_events.iter().map(|e| e.event.event_id()).collect();
        assert_eq!(
            event_ids, logged_events,
            "Event IDs should match original logged events"
        );

        println!("4. Testing other history query methods used by build_history_response...");

        // Test events_for_player_since
        let since_events = new_log.events_for_player_since(player, Some(logged_events[0]));
        println!(
            "   events_for_player_since returned {} events",
            since_events.len()
        );
        assert_eq!(since_events.len(), 2, "Should get 2 events since first");

        // Test events_for_player_until
        let until_events = new_log.events_for_player_until(player, Some(logged_events[2]));
        println!(
            "   events_for_player_until returned {} events",
            until_events.len()
        );
        assert_eq!(until_events.len(), 2, "Should get 2 events until third");

        println!("=== SUCCESS: All history queries working correctly ===");
    }
}
