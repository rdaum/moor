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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use moor_common::tasks::{NarrativeEvent, Presentation};
use moor_var::Obj;

use crate::event_log::{EventLogOps, LoggedNarrativeEvent};

/// Mock event log implementation for testing
pub struct MockEventLog {
    /// Stored narrative events by event ID
    narrative_events: Arc<Mutex<HashMap<Uuid, LoggedNarrativeEvent>>>,
    /// Current presentations by player
    presentations: Arc<Mutex<HashMap<Obj, HashMap<String, Presentation>>>>,
}

impl MockEventLog {
    pub fn new() -> Self {
        Self {
            narrative_events: Arc::new(Mutex::new(HashMap::new())),
            presentations: Arc::new(Mutex::new(HashMap::new())),
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
    pub fn get_all_presentations(&self) -> HashMap<Obj, HashMap<String, Presentation>> {
        self.presentations.lock().unwrap().clone()
    }

    /// Clear all stored data
    pub fn clear(&self) {
        self.narrative_events.lock().unwrap().clear();
        self.presentations.lock().unwrap().clear();
    }
}

impl Default for MockEventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLogOps for MockEventLog {
    fn append(&self, player: Obj, event: Box<NarrativeEvent>) -> Uuid {
        let event_id = event.event_id();

        match &event.event {
            moor_common::tasks::Event::Notify(_, _) | moor_common::tasks::Event::Traceback(_) => {
                // Store narrative events
                let logged_event = LoggedNarrativeEvent { player, event };
                self.narrative_events
                    .lock()
                    .unwrap()
                    .insert(event_id, logged_event);
            }
            moor_common::tasks::Event::Present(presentation) => {
                // Update current presentation state
                let mut presentations = self.presentations.lock().unwrap();
                let player_presentations = presentations.entry(player).or_default();
                player_presentations.insert(presentation.id.clone(), presentation.clone());
            }
            moor_common::tasks::Event::Unpresent(presentation_id) => {
                // Remove presentation from current state
                let mut presentations = self.presentations.lock().unwrap();
                if let Some(player_presentations) = presentations.get_mut(&player) {
                    player_presentations.remove(presentation_id);
                }
            }
        }

        event_id
    }

    fn current_presentations(&self, player: Obj) -> HashMap<String, Presentation> {
        let presentations = self.presentations.lock().unwrap();
        presentations.get(&player).cloned().unwrap_or_default()
    }

    fn dismiss_presentation(&self, player: Obj, presentation_id: String) {
        let mut presentations = self.presentations.lock().unwrap();
        if let Some(player_presentations) = presentations.get_mut(&player) {
            player_presentations.remove(&presentation_id);
        }
    }

    fn events_for_player_since(
        &self,
        player: Obj,
        since: Option<Uuid>,
    ) -> Vec<LoggedNarrativeEvent> {
        let events = self.narrative_events.lock().unwrap();
        let mut player_events: Vec<_> = events
            .values()
            .filter(|event| event.player == player)
            .cloned()
            .collect();

        // Sort by event ID (chronological for UUID v7)
        player_events.sort_by_key(|event| event.event.event_id());

        if let Some(since_id) = since {
            // Return events after the given ID
            player_events
                .into_iter()
                .filter(|event| event.event.event_id() > since_id)
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
        let events = self.narrative_events.lock().unwrap();
        let mut player_events: Vec<_> = events
            .values()
            .filter(|event| event.player == player)
            .cloned()
            .collect();

        // Sort by event ID (chronological for UUID v7)
        player_events.sort_by_key(|event| event.event.event_id());

        if let Some(until_id) = until {
            // Return events before the given ID
            player_events
                .into_iter()
                .filter(|event| event.event.event_id() < until_id)
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
        use std::time::{Duration, SystemTime};

        let cutoff_time = SystemTime::now()
            .checked_sub(Duration::from_secs(seconds_ago))
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let events = self.narrative_events.lock().unwrap();
        let mut player_events: Vec<_> = events
            .values()
            .filter(|event| event.player == player && event.event.timestamp() >= cutoff_time)
            .cloned()
            .collect();

        // Sort by event ID (chronological for UUID v7)
        player_events.sort_by_key(|event| event.event.event_id());
        player_events
    }
}
