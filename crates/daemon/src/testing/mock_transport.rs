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

//! Mock transport layer for testing RPC message handling without ZMQ

use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::rpc::{MessageHandler, Transport};
use moor_common::{schema::rpc as moor_rpc, tasks::NarrativeEvent};
use moor_kernel::SchedulerClient;
use moor_var::Obj;
use planus::ReadAsRoot;
use rpc_common::{HostToken, RpcMessageError};

/// Type alias for captured host reply tuples
type HostReply = (
    HostToken,
    Vec<u8>,
    Result<moor_rpc::DaemonToHostReply, RpcMessageError>,
);

/// Type alias for captured client reply tuples
type ClientReply = (
    Uuid,
    Vec<u8>,
    Result<moor_rpc::DaemonToClientReply, RpcMessageError>,
);

/// Mock transport that captures events for testing instead of sending over ZMQ
pub struct MockTransport {
    /// Captured narrative events
    pub narrative_events: Arc<Mutex<Vec<(Obj, NarrativeEvent)>>>,
    /// Captured host broadcast events
    pub host_events: Arc<Mutex<Vec<moor_rpc::HostBroadcastEvent>>>,
    /// Captured client events
    pub client_events: Arc<Mutex<Vec<(Uuid, moor_rpc::ClientEvent)>>>,
    /// Captured client broadcast events
    pub client_broadcast_events: Arc<Mutex<Vec<moor_rpc::ClientsBroadcastEvent>>>,
    /// Captured host replies (message bytes and reply bytes)
    pub host_replies: Arc<Mutex<Vec<HostReply>>>,
    /// Captured client replies (message bytes and reply bytes)
    pub client_replies: Arc<Mutex<Vec<ClientReply>>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            narrative_events: Arc::new(Mutex::new(Vec::new())),
            host_events: Arc::new(Mutex::new(Vec::new())),
            client_events: Arc::new(Mutex::new(Vec::new())),
            client_broadcast_events: Arc::new(Mutex::new(Vec::new())),
            host_replies: Arc::new(Mutex::new(Vec::new())),
            client_replies: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Simulate processing a host message through the message handler
    pub fn process_host_message(
        &self,
        message_handler: &dyn MessageHandler,
        host_token: HostToken,
        message: moor_rpc::HostToDaemonMessage,
    ) -> Result<moor_rpc::DaemonToHostReply, RpcMessageError> {
        // Serialize to bytes then parse as Ref for handler
        let message_bytes = planus::Builder::new().finish(&message, None).to_vec();

        let message_ref =
            moor_rpc::HostToDaemonMessageRef::read_as_root(&message_bytes).map_err(|e| {
                RpcMessageError::InvalidRequest(format!("Failed to parse message: {e}"))
            })?;

        let result = message_handler.handle_host_message(host_token.clone(), message_ref);

        // Capture the reply for verification
        let mut replies = self.host_replies.lock().unwrap();
        replies.push((host_token, message_bytes, result.clone()));

        result
    }

    /// Simulate processing a client message through the message handler
    pub fn process_client_message(
        &self,
        message_handler: &dyn MessageHandler,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        message: moor_rpc::HostClientToDaemonMessage,
    ) -> Result<moor_rpc::DaemonToClientReply, RpcMessageError> {
        // Serialize to bytes then parse as Ref for handler
        let message_bytes = planus::Builder::new().finish(&message, None).to_vec();

        let message_ref = moor_rpc::HostClientToDaemonMessageRef::read_as_root(&message_bytes)
            .map_err(|e| {
                RpcMessageError::InvalidRequest(format!("Failed to parse message: {e}"))
            })?;

        let result =
            message_handler.handle_client_message(scheduler_client, client_id, message_ref);

        // Capture the reply for verification
        let mut replies = self.client_replies.lock().unwrap();
        replies.push((client_id, message_bytes, result.clone()));

        result
    }

    /// Get captured narrative events
    pub fn get_narrative_events(&self) -> Vec<(Obj, NarrativeEvent)> {
        self.narrative_events.lock().unwrap().clone()
    }

    /// Get captured host events
    pub fn get_host_events(&self) -> Vec<moor_rpc::HostBroadcastEvent> {
        self.host_events.lock().unwrap().clone()
    }

    /// Get captured client events
    pub fn get_client_events(&self) -> Vec<(Uuid, moor_rpc::ClientEvent)> {
        self.client_events.lock().unwrap().clone()
    }

    /// Get captured client broadcast events
    pub fn get_client_broadcast_events(&self) -> Vec<moor_rpc::ClientsBroadcastEvent> {
        self.client_broadcast_events.lock().unwrap().clone()
    }

    /// Clear all captured events
    pub fn clear_events(&self) {
        self.narrative_events.lock().unwrap().clear();
        self.host_events.lock().unwrap().clear();
        self.client_events.lock().unwrap().clear();
        self.client_broadcast_events.lock().unwrap().clear();
        self.host_replies.lock().unwrap().clear();
        self.client_replies.lock().unwrap().clear();
    }

    /// Check if any narrative events were captured
    pub fn has_narrative_events(&self) -> bool {
        !self.narrative_events.lock().unwrap().is_empty()
    }

    /// Check if any host events were captured
    pub fn has_host_events(&self) -> bool {
        !self.host_events.lock().unwrap().is_empty()
    }

    /// Check if any client events were captured
    pub fn has_client_events(&self) -> bool {
        !self.client_events.lock().unwrap().is_empty()
    }

    /// Check if any client broadcast events were captured
    pub fn has_client_broadcast_events(&self) -> bool {
        !self.client_broadcast_events.lock().unwrap().is_empty()
    }

    /// Get count of narrative events
    pub fn narrative_event_count(&self) -> usize {
        self.narrative_events.lock().unwrap().len()
    }

    /// Get count of host events
    pub fn host_event_count(&self) -> usize {
        self.host_events.lock().unwrap().len()
    }

    /// Get count of client events
    pub fn client_event_count(&self) -> usize {
        self.client_events.lock().unwrap().len()
    }

    /// Get count of client broadcast events
    pub fn client_broadcast_event_count(&self) -> usize {
        self.client_broadcast_events.lock().unwrap().len()
    }

    /// Get captured host replies
    pub fn get_host_replies(&self) -> Vec<HostReply> {
        self.host_replies.lock().unwrap().clone()
    }

    /// Get captured client replies
    pub fn get_client_replies(&self) -> Vec<ClientReply> {
        self.client_replies.lock().unwrap().clone()
    }

    /// Get the last host reply
    pub fn get_last_host_reply(
        &self,
    ) -> Option<Result<moor_rpc::DaemonToHostReply, RpcMessageError>> {
        self.host_replies
            .lock()
            .unwrap()
            .last()
            .map(|(_, _, result)| result.clone())
    }

    /// Get the last client reply
    pub fn get_last_client_reply(
        &self,
    ) -> Option<Result<moor_rpc::DaemonToClientReply, RpcMessageError>> {
        self.client_replies
            .lock()
            .unwrap()
            .last()
            .map(|(_, _, result)| result.clone())
    }

    /// Clear all captured replies
    #[allow(dead_code)]
    pub fn clear_replies(&self) {
        self.host_replies.lock().unwrap().clear();
        self.client_replies.lock().unwrap().clear();
    }

    /// Manually capture a client event (for testing scenarios)
    pub fn capture_client_event(&self, client_id: Uuid, event: moor_rpc::ClientEvent) {
        self.client_events.lock().unwrap().push((client_id, event));
    }

    /// Convenience method to send a narrative event (for testing)
    pub fn send_narrative_event(&self, player: Obj, event: NarrativeEvent) {
        self.narrative_events.lock().unwrap().push((player, event));
    }

    /// Convenience method to send a host event (for testing)
    pub fn send_host_event(&self, event: moor_rpc::HostBroadcastEvent) {
        self.host_events.lock().unwrap().push(event);
    }

    /// Convenience method to send a client broadcast event (for testing)
    pub fn send_client_broadcast_event(&self, event: moor_rpc::ClientsBroadcastEvent) {
        self.client_broadcast_events.lock().unwrap().push(event);
    }

    /// Wait for at least the specified number of narrative events to be captured
    /// Returns true if the condition is met within the timeout, false otherwise
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

    /// Wait for at least the specified number of client events to be captured
    /// Returns true if the condition is met within the timeout, false otherwise
    #[allow(dead_code)]
    pub fn wait_for_client_events(&self, min_count: usize, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            if self.client_event_count() >= min_count {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    /// Wait for at least the specified number of client replies to be captured
    /// Returns true if the condition is met within the timeout, false otherwise
    #[allow(dead_code)]
    pub fn wait_for_client_replies(&self, min_count: usize, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            if self.client_replies.lock().unwrap().len() >= min_count {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    /// Wait for a specific condition to be met with a custom predicate
    /// Returns true if the condition is met within the timeout, false otherwise
    pub fn wait_for_condition<F>(&self, predicate: F, timeout_ms: u64) -> bool
    where
        F: Fn(&MockTransport) -> bool,
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

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockTransport {
    fn start_request_loop(
        &self,
        _rpc_endpoint: String,
        _scheduler_client: SchedulerClient,
        _message_handler: Arc<dyn MessageHandler>,
    ) -> eyre::Result<()> {
        // Mock implementation - doesn't actually start a loop
        // In tests, messages are processed via process_host_message/process_client_message
        Ok(())
    }

    fn publish_narrative_events(
        &self,
        events: &[(Obj, Box<NarrativeEvent>)],
        _connections: &dyn crate::connections::ConnectionRegistry,
    ) -> Result<(), eyre::Error> {
        let mut captured = self.narrative_events.lock().unwrap();
        for (player, event) in events {
            captured.push((*player, (**event).clone()));
        }
        Ok(())
    }

    fn broadcast_host_event(&self, event: moor_rpc::HostBroadcastEvent) -> Result<(), eyre::Error> {
        let mut captured = self.host_events.lock().unwrap();
        captured.push(event);
        Ok(())
    }

    fn publish_client_event(
        &self,
        client_id: Uuid,
        event: moor_rpc::ClientEvent,
    ) -> Result<(), eyre::Error> {
        let mut captured = self.client_events.lock().unwrap();
        captured.push((client_id, event));
        Ok(())
    }

    fn broadcast_client_event(
        &self,
        event: moor_rpc::ClientsBroadcastEvent,
    ) -> Result<(), eyre::Error> {
        let mut captured = self.client_broadcast_events.lock().unwrap();
        captured.push(event);
        Ok(())
    }
}
