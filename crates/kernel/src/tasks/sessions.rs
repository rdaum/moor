// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::sync::{Arc, RwLock};

use thiserror::Error;
use uuid::Uuid;

use moor_values::model::NarrativeEvent;
use moor_values::var::Objid;

/// The interface for managing the user I/O connection side of state, exposed by the scheduler to
/// the VM during execution and by the host server to the scheduler.
///
/// Because the execution path within the server is transactional, with the scheduler committing
/// and rolling back 'world state' on task commit/rollback, the general expectation is that this
/// entity should *also* perform transactionally, buffering output until the task commits, and
/// throwing it out on rollback. This may or may not be practical for large amounts of output.
///
/// It is up to the implementation to decide how to buffer output. Options could include a
/// memory mapped file, a full database, or a simple in-memory buffer.
///
/// Implementations would live in the 'server' host (e.g. websocket connections or repl loop)
///
// TODO: Fix up connected/reconnected/discconnected handling.
//  Will probably deprecate MOO's concept of 'disconnected' and 'connected' players in the long
//  run and emulate slack, discord, skype, etc which have a concept of 'presence' (online, offline,
//  away, etc) but keep a persistent virtual history. Challenge would be around making this work
//  nicely with existing MOO code.
//  Right now the same user can connect multiple times and we output and input on all connections,
//  which is different from MOO's "reconnected" handling, but probably preferable.

pub trait Session: Send + Sync {
    /// Commit for current activity, called by the scheduler when a task commits and *after* the world
    /// state has successfully been committed. This is the point at which the session should send
    /// its buffered output.
    /// The session should not be usable after this point.
    /// Note: there is no "two phase" process, so if I/O output fails, the world state will not be
    ///  rolled back. I/O output is not considered "critical" to the transaction's success, and
    ///  the world state's integrity and performance in that path is considered more important.
    ///  If this leads to weird symptoms, we can revisit this.
    fn commit(&self) -> Result<(), SessionError>;

    /// Rollback for this session, called by the scheduler when a task rolls back and *after* the
    /// world state has successfully been rolled back.
    /// Should result in the session throwing away all buffered output.
    /// The session should not be usable after this point.
    fn rollback(&self) -> Result<(), SessionError>;

    /// "Fork" this session; create a new session which attaches to the same connection, but
    /// maintains its own buffer and state and can be committed/rolled back independently.
    /// Is used for forked tasks which end up running in their own transaction.
    /// Note: `disconnect` on one must also disconnect on all the other forks of the same lineage.
    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError>;

    /// Request that the client send input to the server.
    /// The task is committed and suspended until the client sends input to `submit_requested_input`
    /// with the given `input_request_id` argument, at which time the task is resumed in a new
    /// transaction.
    fn request_input(&self, player: Objid, input_request_id: Uuid) -> Result<(), SessionError>;

    /// Spool output to the given player's connection.
    /// The actual output will not be sent until the task commits, and will be thrown out on
    /// rollback.
    fn send_event(&self, player: Objid, event: NarrativeEvent) -> Result<(), SessionError>;

    /// Send non-spooled output to the given player's connection
    /// Examples of the kinds of messages that would be sent here are state-independent messages
    /// like login/logout messages, system error messages ("task aborted") or messages that are not
    /// generally co-incident with the mutable state of the world and that need not be logged
    /// across multiple connections, etc.
    fn send_system_msg(&self, player: Objid, msg: &str) -> Result<(), SessionError>;

    /// Process a (wizard) request for system shutdown, with an optional shutdown message.
    fn shutdown(&self, msg: Option<String>) -> Result<(), SessionError>;

    /// The 'name' of the *most recent* connection associated with the player.
    /// In a networked environment this is the hostname.
    /// LambdaMOO cores tend to expect this to be a resolved DNS hostname.
    fn connection_name(&self, player: Objid) -> Result<String, SessionError>;

    /// Disconnect the given player's connection.
    fn disconnect(&self, player: Objid) -> Result<(), SessionError>;

    /// Return the list of other currently-connected players.
    fn connected_players(&self) -> Result<Vec<Objid>, SessionError>;

    /// Return how many seconds the given player has been connected.
    fn connected_seconds(&self, player: Objid) -> Result<f64, SessionError>;

    /// Return how many seconds the given player has been idle (no tasks submitted).
    fn idle_seconds(&self, player: Objid) -> Result<f64, SessionError>;
}

/// A factory for creating background sessions, usually on task resumption on server restart.
pub trait SessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        player: Objid,
    ) -> Result<Arc<dyn Session>, SessionError>;
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("No connection for player {0}")]
    NoConnectionForPlayer(Objid),
    #[error("Could not deliver session message")]
    DeliveryError,
    #[error("Could not commit session: {0}")]
    CommitError(String),
    #[error("Invalid authorization token")]
    InvalidToken,
}

/// A simple no-op implementation of the Sessions trait, for use in unit tests.
/// No output, and pretends no players are connected.
pub struct NoopClientSession {}
impl NoopClientSession {
    pub fn new() -> Self {
        NoopClientSession {}
    }
}

impl Default for NoopClientSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Session for NoopClientSession {
    fn commit(&self) -> Result<(), SessionError> {
        Ok(())
    }
    fn rollback(&self) -> Result<(), SessionError> {
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(self.clone())
    }

    fn request_input(&self, player: Objid, _input_request_id: Uuid) -> Result<(), SessionError> {
        panic!(
            "NoopClientSession::request_input called for player {}",
            player.0
        )
    }

    fn send_event(&self, _player: Objid, _msg: NarrativeEvent) -> Result<(), SessionError> {
        Ok(())
    }

    fn send_system_msg(&self, _player: Objid, _msg: &str) -> Result<(), SessionError> {
        Ok(())
    }

    fn shutdown(&self, _msg: Option<String>) -> Result<(), SessionError> {
        Ok(())
    }
    fn connection_name(&self, player: Objid) -> Result<String, SessionError> {
        Ok(format!("player-{}", player.0))
    }
    fn disconnect(&self, _player: Objid) -> Result<(), SessionError> {
        Ok(())
    }
    fn connected_players(&self) -> Result<Vec<Objid>, SessionError> {
        Ok(vec![])
    }

    fn connected_seconds(&self, _player: Objid) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn idle_seconds(&self, _player: Objid) -> Result<f64, SessionError> {
        Ok(0.0)
    }
}

/// A 'mock' client connection which collects output in a vector of strings that tests can use to
/// verify output.
/// For now that's all it does, but facilities for pretending players are connected, mocking
/// hostnames, etc. can be added later.
struct Inner {
    received: Vec<NarrativeEvent>,
    committed: Vec<NarrativeEvent>,
}
pub struct MockClientSession {
    inner: RwLock<Inner>,
    system: Arc<RwLock<Vec<String>>>,
}
impl MockClientSession {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
                received: vec![],
                committed: vec![],
            }),
            system: Arc::new(Default::default()),
        }
    }
    pub fn received(&self) -> Vec<NarrativeEvent> {
        let inner = self.inner.read().unwrap();
        inner.received.clone()
    }
    pub fn committed(&self) -> Vec<NarrativeEvent> {
        let inner = self.inner.read().unwrap();
        inner.committed.clone()
    }
    pub fn system(&self) -> Vec<String> {
        self.system.read().unwrap().clone()
    }
}

impl Default for MockClientSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Session for MockClientSession {
    fn commit(&self) -> Result<(), SessionError> {
        let mut inner = self.inner.write().unwrap();
        inner.committed = std::mem::take(&mut inner.received);
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        self.inner.write().unwrap().received.clear();
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(MockClientSession {
            inner: RwLock::new(Inner {
                received: vec![],
                committed: vec![],
            }),
            system: self.system.clone(),
        }))
    }

    fn request_input(&self, player: Objid, _input_request_id: Uuid) -> Result<(), SessionError> {
        panic!(
            "MockClientSession::request_input called for player {}",
            player.0
        )
    }

    fn send_event(&self, _player: Objid, msg: NarrativeEvent) -> Result<(), SessionError> {
        self.inner.write().unwrap().received.push(msg);
        Ok(())
    }

    fn send_system_msg(&self, player: Objid, msg: &str) -> Result<(), SessionError> {
        self.system
            .write()
            .unwrap()
            .push(format!("{}: {}", player.0, msg));
        Ok(())
    }

    fn shutdown(&self, msg: Option<String>) -> Result<(), SessionError> {
        let mut system = self.system.write().unwrap();
        if let Some(msg) = msg {
            system.push(format!("shutdown: {}", msg));
        } else {
            system.push(String::from("shutdown"));
        }
        Ok(())
    }

    fn connection_name(&self, player: Objid) -> Result<String, SessionError> {
        Ok(format!("player-{}", player))
    }

    fn disconnect(&self, _player: Objid) -> Result<(), SessionError> {
        let mut system = self.system.write().unwrap();
        system.push(String::from("disconnect"));
        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Objid>, SessionError> {
        Ok(vec![])
    }

    fn connected_seconds(&self, _player: Objid) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn idle_seconds(&self, _player: Objid) -> Result<f64, SessionError> {
        Ok(0.0)
    }
}
