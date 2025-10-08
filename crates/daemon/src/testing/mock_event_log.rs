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

//! Mock event log for testing

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

use moor_common::tasks::Presentation;
use moor_schema::{
    common::ObjUnion,
    convert::{obj_from_flatbuffer_struct, obj_to_flatbuffer_struct},
    event_log::LoggedNarrativeEvent,
};
use moor_var::Obj;

use crate::event_log::{EventLogOps, PresentationAction};

/// Mock event log implementation for testing
pub struct MockEventLog {
    /// Stored narrative events by event ID
    narrative_events: Arc<Mutex<HashMap<Uuid, LoggedNarrativeEvent>>>,
    /// Current presentations by player (Vec instead of HashMap to match new API)
    presentations: Arc<Mutex<HashMap<Obj, Vec<Presentation>>>>,
}

impl MockEventLog {
    pub fn new() -> Self {
        Self {
            narrative_events: Arc::new(Mutex::new(HashMap::new())),
            presentations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Helper to extract UUID from FlatBuffer event
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

    /// Helper to compare FlatBuffer Obj with domain Obj
    fn obj_matches(
        player_fb: &moor_schema::common::Obj,
        event_player: &moor_schema::common::Obj,
    ) -> bool {
        match (&player_fb.obj, &event_player.obj) {
            (ObjUnion::ObjId(a), ObjUnion::ObjId(b)) => a.id == b.id,
            (ObjUnion::UuObjId(a), ObjUnion::UuObjId(b)) => a.packed_value == b.packed_value,
            (ObjUnion::AnonymousObjId(a), ObjUnion::AnonymousObjId(b)) => {
                a.packed_value == b.packed_value
            }
            _ => false,
        }
    }

    /// Get all stored events (for testing)
    pub fn get_all_events(&self) -> Vec<LoggedNarrativeEvent> {
        self.narrative_events
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    /// Get all presentations for all players (for testing)
    #[allow(dead_code)]
    pub fn get_all_presentations(&self) -> HashMap<Obj, Vec<Presentation>> {
        self.presentations.lock().unwrap().clone()
    }

    /// Clear all stored data
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.narrative_events.lock().unwrap().clear();
        self.presentations.lock().unwrap().clear();
    }

    /// Get count of narrative events
    #[allow(dead_code)]
    pub fn narrative_event_count(&self) -> usize {
        self.narrative_events.lock().unwrap().len()
    }

    /// Get count of events for a specific player
    #[allow(dead_code)]
    pub fn event_count_for_player(&self, player: Obj) -> usize {
        let player_fb = obj_to_flatbuffer_struct(&player);
        self.narrative_events
            .lock()
            .unwrap()
            .values()
            .filter(|event| Self::obj_matches(&player_fb, &event.player))
            .count()
    }

    /// Wait for at least the specified number of narrative events to be logged
    /// Returns true if the condition is met within the timeout, false otherwise
    #[allow(dead_code)]
    pub fn wait_for_narrative_events(&self, min_count: usize, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            if self.narrative_event_count() >= min_count {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    /// Wait for at least the specified number of events for a specific player
    /// Returns true if the condition is met within the timeout, false otherwise
    #[allow(dead_code)]
    pub fn wait_for_player_events(&self, player: Obj, min_count: usize, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            if self.event_count_for_player(player) >= min_count {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    /// Wait for a specific condition to be met with a custom predicate
    /// Returns true if the condition is met within the timeout, false otherwise
    #[allow(dead_code)]
    pub fn wait_for_condition<F>(&self, predicate: F, timeout_ms: u64) -> bool
    where
        F: Fn(&MockEventLog) -> bool,
    {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            if predicate(self) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }
}

impl Default for MockEventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLogOps for MockEventLog {
    fn append(
        &self,
        event: LoggedNarrativeEvent,
        presentation_action: Option<PresentationAction>,
    ) -> Uuid {
        // Extract event_id from FlatBuffer event
        let event_id_bytes = event.event_id.data.as_slice();
        let event_id = if event_id_bytes.len() == 16 {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(event_id_bytes);
            Uuid::from_bytes(bytes)
        } else {
            Uuid::nil()
        };

        // Handle presentation state updates (same as real EventLog)
        if let Some(action) = presentation_action {
            let player_obj =
                obj_from_flatbuffer_struct(&event.player).expect("Failed to convert player obj");

            match action {
                PresentationAction::Add(presentation) => {
                    let mut presentations = self.presentations.lock().unwrap();
                    presentations
                        .entry(player_obj)
                        .or_default()
                        .push(presentation);
                }
                PresentationAction::Remove(presentation_id) => {
                    let mut presentations = self.presentations.lock().unwrap();
                    if let Some(player_presentations) = presentations.get_mut(&player_obj) {
                        player_presentations.retain(|p| p.id != presentation_id);
                    }
                }
            }
        }

        // Store the event
        self.narrative_events
            .lock()
            .unwrap()
            .insert(event_id, event);

        event_id
    }

    fn current_presentations(&self, player: Obj) -> Vec<Presentation> {
        let presentations = self.presentations.lock().unwrap();
        presentations.get(&player).cloned().unwrap_or_default()
    }

    fn dismiss_presentation(&self, player: Obj, presentation_id: String) {
        let mut presentations = self.presentations.lock().unwrap();
        if let Some(player_presentations) = presentations.get_mut(&player) {
            player_presentations.retain(|p| p.id != presentation_id);
        }
    }

    fn get_pubkey(&self, _player: Obj) -> Option<String> {
        // Mock implementation - no encryption in tests
        None
    }

    fn set_pubkey(&self, _player: Obj, _pubkey: String) {
        // Mock implementation - no encryption in tests
    }

    fn delete_all_events(&self, player: Obj) -> Result<(), String> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let mut events = self.narrative_events.lock().unwrap();

        // Remove all events for this player
        events.retain(|_, event| !Self::obj_matches(&player_fb, &event.player));

        Ok(())
    }

    fn events_for_player_since(
        &self,
        player: Obj,
        since: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let events = self.narrative_events.lock().unwrap();
        let mut player_events: Vec<_> = events
            .values()
            .filter(|event| Self::obj_matches(&player_fb, &event.player))
            .cloned()
            .collect();

        // Sort by event ID (chronological for UUID v7)
        player_events.sort_by_key(Self::extract_event_id);

        if let Some(since_id) = since {
            // Return events after the given ID
            player_events
                .into_iter()
                .filter(|event| Self::extract_event_id(event) > since_id)
                .collect()
        } else {
            player_events
        }
    }

    fn events_for_player_until(
        &self,
        player: Obj,
        until: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        let player_fb = obj_to_flatbuffer_struct(&player);
        let events = self.narrative_events.lock().unwrap();
        let mut player_events: Vec<_> = events
            .values()
            .filter(|event| Self::obj_matches(&player_fb, &event.player))
            .cloned()
            .collect();

        // Sort by event ID (chronological for UUID v7)
        player_events.sort_by_key(Self::extract_event_id);

        if let Some(until_id) = until {
            // Return events before the given ID
            player_events
                .into_iter()
                .filter(|event| Self::extract_event_id(event) < until_id)
                .collect()
        } else {
            player_events
        }
    }

    fn events_for_player_since_seconds(
        &self,
        player: Obj,
        seconds_ago: u64,
    ) -> Vec<LoggedNarrativeEvent> {
        let cutoff_time = SystemTime::now()
            .checked_sub(Duration::from_secs(seconds_ago))
            .unwrap_or(UNIX_EPOCH);

        let player_fb = obj_to_flatbuffer_struct(&player);
        let events = self.narrative_events.lock().unwrap();
        let mut player_events: Vec<_> = events
            .values()
            .filter(|event| {
                if !Self::obj_matches(&player_fb, &event.player) {
                    return false;
                }
                // Check timestamp (FlatBuffer timestamp is in nanoseconds)
                let event_time = UNIX_EPOCH + Duration::from_nanos(event.timestamp);
                event_time >= cutoff_time
            })
            .cloned()
            .collect();

        // Sort by event ID (chronological for UUID v7)
        player_events.sort_by_key(Self::extract_event_id);
        player_events
    }
}
