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

//! Test sessions with transaction-local output and one scenario-wide delivery/presence hub.
//! Synthetic connections can be attached, detached, and reassigned. Attributes and elapsed time are not simulated.

use moor_common::tasks::{ConnectionDetails, Event, NarrativeEvent, Session, SessionError};
use moor_var::{Obj, Symbol, Var};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    },
};
use uuid::Uuid;

pub type InputRequest = (Obj, Uuid, Option<Vec<(Symbol, Var)>>);

#[derive(Default)]
pub struct SessionHub {
    delivered: Mutex<Vec<(Obj, NarrativeEvent)>>,
    logged: Mutex<Vec<(Obj, NarrativeEvent)>>,
    input: Mutex<Vec<InputRequest>>,
    connected: Mutex<HashMap<Obj, Obj>>,
    next_connection: AtomicI32,
    system: Mutex<Vec<String>>,
}

impl SessionHub {
    pub fn attach(&self, player: Obj) -> Obj {
        let connection = Obj::mk_id(-1000 - self.next_connection.fetch_add(1, Ordering::Relaxed));
        self.connected.lock().unwrap().insert(connection, player);
        connection
    }
    pub fn detach(&self, connection: Obj) -> bool {
        self.connected.lock().unwrap().remove(&connection).is_some()
    }
    pub fn reassign(&self, connection: Obj, player: Obj) -> bool {
        let mut connected = self.connected.lock().unwrap();
        let Some(owner) = connected.get_mut(&connection) else {
            return false;
        };
        *owner = player;
        true
    }
    pub fn for_player(&self, player: Obj) -> Vec<Obj> {
        let mut connections: Vec<_> = self
            .connected
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(connection, owner)| (*owner == player).then_some(*connection))
            .collect();
        connections.sort();
        connections
    }
    pub fn set_connected(&self, player: Obj, connected: bool) {
        if connected {
            if self.for_player(player).is_empty() {
                self.attach(player);
            }
        } else {
            self.connected
                .lock()
                .unwrap()
                .retain(|_, owner| *owner != player);
        }
    }
    pub fn owner_for(&self, connection: Obj) -> Option<Obj> {
        self.connected.lock().unwrap().get(&connection).copied()
    }
    pub fn recipient_player(&self, target: Obj) -> Obj {
        self.connected
            .lock()
            .unwrap()
            .get(&target)
            .copied()
            .unwrap_or(target)
    }
    /// Consume a committed history record for an exact player, type and string value.
    pub fn consume_logged(&self, player: Obj, content_type: &str, text: &str) -> bool {
        let mut logged = self.logged.lock().unwrap();
        let Some(index) = logged.iter().position(|(recipient, event)| {
            *recipient == player
                && matches!(&event.event,
                Event::Notify { value, content_type: Some(actual), .. }
                    if actual.as_str() == content_type && value.as_string() == Some(text))
        }) else {
            return false;
        };
        logged.remove(index);
        true
    }
    pub fn take_input_requests(&self) -> Vec<InputRequest> {
        std::mem::take(&mut *self.input.lock().unwrap())
    }
    pub fn take_committed_events(&self) -> Vec<(Obj, NarrativeEvent)> {
        std::mem::take(&mut *self.delivered.lock().unwrap())
    }
}

pub struct TestSession {
    pending: Mutex<Vec<(Obj, NarrativeEvent)>>,
    pending_log: Mutex<Vec<(Obj, NarrativeEvent)>>,
    hub: Arc<SessionHub>,
    connection: Option<Obj>,
}

impl TestSession {
    pub fn new(hub: Arc<SessionHub>) -> Self {
        Self {
            pending: Mutex::default(),
            pending_log: Mutex::default(),
            hub,
            connection: None,
        }
    }

    pub fn for_player(hub: Arc<SessionHub>, player: Obj) -> Self {
        let connection = hub.for_player(player).first().copied();
        Self {
            connection,
            ..Self::new(hub)
        }
    }
}

impl TestSession {
    pub fn for_connection(hub: Arc<SessionHub>, connection: Obj) -> Self {
        Self {
            connection: Some(connection),
            ..Self::new(hub)
        }
    }
    pub fn connection_id(&self) -> Option<Obj> {
        self.connection
    }
}

impl Session for TestSession {
    fn commit(&self) -> Result<(), SessionError> {
        let pending = std::mem::take(&mut *self.pending.lock().unwrap());
        self.hub.delivered.lock().unwrap().extend(pending);
        let logged = std::mem::take(&mut *self.pending_log.lock().unwrap());
        self.hub.logged.lock().unwrap().extend(logged);
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        self.pending.lock().unwrap().clear();
        self.pending_log.lock().unwrap().clear();
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(Self {
            connection: self.connection,
            ..Self::new(self.hub.clone())
        }))
    }

    fn fork_retry(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(self)
    }

    fn request_input(
        &self,
        player: Obj,
        input_request_id: Uuid,
        metadata: Option<Vec<(Symbol, Var)>>,
    ) -> Result<(), SessionError> {
        self.hub
            .input
            .lock()
            .unwrap()
            .push((player, input_request_id, metadata));
        Ok(())
    }

    fn send_event(&self, player: Obj, msg: Box<NarrativeEvent>) -> Result<(), SessionError> {
        let connections = self.hub.for_player(player);
        let mut pending = self.pending.lock().unwrap();
        if connections.is_empty() {
            pending.push((player, *msg));
        } else {
            pending.extend(
                connections
                    .into_iter()
                    .map(|connection| (connection, (*msg).clone())),
            );
        }
        Ok(())
    }

    fn log_event(&self, player: Obj, event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        self.pending_log.lock().unwrap().push((player, *event));
        Ok(())
    }

    fn send_system_msg(&self, player: Obj, msg: &str) -> Result<(), SessionError> {
        self.hub
            .system
            .lock()
            .unwrap()
            .push(format!("{player}: {msg}"));
        Ok(())
    }

    fn notify_shutdown(&self, msg: Option<String>) -> Result<(), SessionError> {
        let mut system = self.hub.system.lock().unwrap();
        if let Some(msg) = msg {
            system.push(format!("shutdown: {msg}"));
        } else {
            system.push(String::from("shutdown"));
        }
        Ok(())
    }

    fn connection_name(&self, player: Obj) -> Result<String, SessionError> {
        Ok(format!("player-{player}"))
    }

    fn disconnect(&self, player: Obj) -> Result<(), SessionError> {
        if player.is_positive() {
            self.hub.set_connected(player, false);
        } else {
            self.hub.detach(player);
        }
        Ok(())
    }

    fn connected_players(&self, include_all: bool) -> Result<Vec<Obj>, SessionError> {
        let connected = self.hub.connected.lock().unwrap();
        let mut players: Vec<_> = connected.values().copied().collect();
        players.sort();
        players.dedup();
        if include_all {
            players.extend(connected.keys().copied());
        }
        players.sort();
        Ok(players)
    }

    fn connected_seconds(&self, player: Obj) -> Result<f64, SessionError> {
        let player = self.hub.recipient_player(player);
        self.hub
            .connected
            .lock()
            .unwrap()
            .values()
            .any(|owner| *owner == player)
            .then_some(0.0)
            .ok_or(SessionError::NoConnectionForPlayer(player))
    }

    fn idle_seconds(&self, player: Obj) -> Result<f64, SessionError> {
        self.connected_seconds(player)
    }

    fn connections(&self, player: Option<Obj>) -> Result<Vec<Obj>, SessionError> {
        let connected = self.hub.connected.lock().unwrap();
        if let Some(player) = player {
            let mut connections: Vec<_> = connected
                .iter()
                .filter_map(|(connection, owner)| (*owner == player).then_some(*connection))
                .collect();
            connections.sort();
            return Ok(connections);
        }
        let Some(current) = self
            .connection
            .filter(|connection| connected.contains_key(connection))
        else {
            return Ok(Vec::new());
        };
        let owner = connected[&current];
        let mut others: Vec<_> = connected
            .iter()
            .filter_map(|(connection, candidate)| {
                (*candidate == owner && *connection != current).then_some(*connection)
            })
            .collect();
        others.sort();
        // The host places the initiating client first, followed by its other connections.
        Ok(std::iter::once(current).chain(others).collect())
    }

    fn connection_details(
        &self,
        player: Option<Obj>,
    ) -> Result<Vec<ConnectionDetails>, SessionError> {
        Ok(self
            .connections(player)?
            .into_iter()
            .map(|connection_obj| ConnectionDetails {
                connection_obj,
                peer_addr: "session-test".into(),
                idle_seconds: 0.0,
                acceptable_content_types: vec![Symbol::mk("text/plain")],
            })
            .collect())
    }

    fn connection_attributes(&self, _obj: Obj) -> Result<Var, SessionError> {
        use moor_var::v_list;
        Ok(v_list(&[]))
    }

    fn set_connection_attribute(
        &self,
        _connection_obj: Obj,
        _key: Symbol,
        _value: Var,
    ) -> Result<(), SessionError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::tasks::Event;
    use moor_var::{SYSTEM_OBJECT, v_obj, v_str};

    fn event(text: &str) -> Box<NarrativeEvent> {
        Box::new(NarrativeEvent::notify(
            v_obj(SYSTEM_OBJECT),
            v_str(text),
            None,
            false,
            false,
            None,
        ))
    }
    #[test]
    fn history_requires_commit_and_exact_principal_type_and_value() {
        let hub = Arc::new(SessionHub::default());
        let session = Arc::new(TestSession::new(hub.clone()));
        let player = Obj::mk_id(7);
        let record = || {
            Box::new(NarrativeEvent::notify(
                v_obj(SYSTEM_OBJECT),
                v_str("history"),
                Some(Symbol::mk("text_plain")),
                false,
                false,
                None,
            ))
        };
        session.log_event(player, record()).unwrap();
        assert!(!hub.consume_logged(player, "text_plain", "history"));
        session.rollback().unwrap();
        session.commit().unwrap();
        assert!(!hub.consume_logged(player, "text_plain", "history"));
        let child = session.clone().fork().unwrap();
        child.log_event(player, record()).unwrap();
        session.rollback().unwrap();
        child.commit().unwrap();
        assert!(!hub.consume_logged(Obj::mk_id(8), "text_plain", "history"));
        assert!(!hub.consume_logged(player, "text/html", "history"));
        assert!(!hub.consume_logged(player, "text_plain", "other"));
        assert!(hub.consume_logged(player, "text_plain", "history"));
        assert!(!hub.consume_logged(player, "text_plain", "history"));
    }
    fn delivered(hub: &SessionHub) -> Vec<(Obj, String)> {
        hub.take_committed_events()
            .into_iter()
            .map(|(recipient, event)| {
                let Event::Notify { value, .. } = event.event else {
                    panic!("not a notification")
                };
                (recipient, value.as_string().unwrap().to_string())
            })
            .collect()
    }

    #[test]
    fn transaction_buffers_and_shared_delivery_preserve_payloads() {
        let hub = Arc::new(SessionHub::default());
        let parent = Arc::new(TestSession::new(hub.clone()));
        let child = parent.clone().fork().unwrap();
        let a = Obj::mk_id(1);
        let b = Obj::mk_id(2);
        parent.send_event(a, event("parent pending")).unwrap();
        child.send_event(b, event("discard child")).unwrap();
        child.rollback().unwrap();
        assert!(delivered(&hub).is_empty());
        parent.commit().unwrap();
        assert_eq!(delivered(&hub), vec![(a, "parent pending".into())]);
        parent.send_event(a, event("discard parent")).unwrap();
        child.send_event(b, event("child committed")).unwrap();
        child.commit().unwrap();
        parent.rollback().unwrap();
        assert_eq!(delivered(&hub), vec![(b, "child committed".into())]);
        assert!(delivered(&hub).is_empty());
        parent.send_event(a, event("first")).unwrap();
        parent.commit().unwrap();
        child.commit().unwrap();
        parent.send_event(b, event("second")).unwrap();
        parent.commit().unwrap();
        assert_eq!(
            delivered(&hub),
            vec![(a, "first".into()), (b, "second".into())]
        );
        parent.send_event(a, event("retry discarded")).unwrap();
        parent.rollback().unwrap();
        let retry = parent.fork_retry().unwrap();
        retry.send_event(b, event("retry committed")).unwrap();
        retry.commit().unwrap();
        assert_eq!(delivered(&hub), vec![(b, "retry committed".into())]);
    }

    #[test]
    fn presence_and_disconnect_are_shared_across_sessions_and_forks() {
        let hub = Arc::new(SessionHub::default());
        let parent = Arc::new(TestSession::new(hub.clone()));
        let sibling = TestSession::new(hub.clone());
        let child = parent.clone().fork().unwrap();
        let player = Obj::mk_id(12);
        assert!(parent.connected_players(false).unwrap().is_empty());
        assert!(
            matches!(parent.connected_seconds(player), Err(SessionError::NoConnectionForPlayer(p)) if p == player)
        );
        assert!(parent.idle_seconds(player).is_err());
        hub.set_connected(player, true);
        assert_eq!(child.connected_players(false).unwrap(), vec![player]);
        assert_eq!(sibling.connected_seconds(player).unwrap(), 0.0);
        child.disconnect(player).unwrap();
        assert!(parent.connected_players(false).unwrap().is_empty());
        assert!(sibling.idle_seconds(player).is_err());
        hub.set_connected(player, true);
        assert_eq!(parent.connected_players(false).unwrap(), vec![player]);
    }

    #[test]
    fn connection_targets_are_scoped_and_survive_forks() {
        let hub = Arc::new(SessionHub::default());
        let a = Obj::mk_id(11);
        let b = Obj::mk_id(12);
        hub.set_connected(a, true);
        hub.set_connected(b, true);
        let session = Arc::new(TestSession::for_player(hub.clone(), a));
        let ac = session.connections(None).unwrap()[0];
        let bc = session.connections(Some(b)).unwrap()[0];
        assert_ne!(ac, bc);
        assert!(!ac.is_positive());
        let child = session.clone().fork().unwrap();
        assert_eq!(child.connections(None).unwrap(), vec![ac]);
        child.send_event(ac, event("local")).unwrap();
        assert!(delivered(&hub).is_empty());
        child.commit().unwrap();
        assert_eq!(delivered(&hub), vec![(ac, "local".into())]);
        assert_eq!(hub.recipient_player(ac), a);
        assert!(
            TestSession::new(hub.clone())
                .connections(None)
                .unwrap()
                .is_empty()
        );
        session.disconnect(ac).unwrap();
        assert!(child.connections(None).unwrap().is_empty());
        assert_eq!(session.connections(Some(b)).unwrap(), vec![bc]);
        hub.set_connected(a, true);
        assert!(child.connections(None).unwrap().is_empty());
        let replacement = TestSession::for_player(hub, a);
        assert_ne!(replacement.connections(None).unwrap(), vec![ac]);
    }

    #[test]
    fn multiple_connections_preserve_targets_after_reassignment_and_detach() {
        let hub = Arc::new(SessionHub::default());
        let a = Obj::mk_id(11);
        let b = Obj::mk_id(12);
        let first = hub.attach(a);
        let second = hub.attach(a);
        let session = TestSession::for_connection(hub.clone(), second);
        assert_eq!(session.connections(None).unwrap(), vec![second, first]);
        assert_eq!(session.connections(Some(a)).unwrap().len(), 2);
        session.send_event(a, event("broadcast")).unwrap();
        session.commit().unwrap();
        let output = delivered(&hub);
        assert_eq!(output.len(), 2);
        assert!(output.iter().any(|(target, _)| *target == first));
        assert!(output.iter().any(|(target, _)| *target == second));
        assert!(hub.reassign(second, b));
        assert_eq!(hub.recipient_player(second), b);
        assert_eq!(session.connections(Some(a)).unwrap(), vec![first]);
        assert_eq!(session.connections(Some(b)).unwrap(), vec![second]);
        assert!(hub.detach(second));
        assert!(session.connections(None).unwrap().is_empty());
        assert!(!hub.reassign(second, a));
        assert_eq!(session.connections(Some(a)).unwrap(), vec![first]);
    }

    #[test]
    fn input_requests_keep_metadata_and_drain_once() {
        let hub = Arc::new(SessionHub::default());
        let parent = Arc::new(TestSession::new(hub.clone()));
        let child = parent.fork().unwrap();
        let player = Obj::mk_id(7);
        let id = Uuid::nil();
        let metadata = Some(vec![(Symbol::mk("prompt"), v_str("Subject:"))]);
        child.request_input(player, id, metadata.clone()).unwrap();
        assert_eq!(hub.take_input_requests(), vec![(player, id, metadata)]);
        assert!(hub.take_input_requests().is_empty());
    }
}
