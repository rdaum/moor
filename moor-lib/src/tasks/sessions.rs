use anyhow::Error;
use async_trait::async_trait;
use moor_value::model::NarrativeEvent;
use moor_value::var::objid::Objid;
use std::sync::{Arc, RwLock};

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
// TODO: Will probably deprecate MOO's concept of 'disconnected' and 'connected' players in the long
//  run and emulate slack, discord, skype, etc which have a concept of 'presence' (online, offline,
//  away, etc) but keep a persistent virtual history. Challenge would be around making this work
//  nicely with existing MOO code.

// TODO: some of the methods here are cross-task session (shutdown, connected_players, etc.) and some
//   are per-Session (send_text, commit, rollback, etc.). This is a bit of a mess, and should be
//   broken up to make it clear. In particular this is a problem if a user is connected from a
//   different "kind" of session and a message is sent to them from a task running in another.
//   So `Session` needs to be broken away from `Connections`? -- the latter should be registered with
//   the Scheduler on a per-user basis, if we are to support the possibility of a) multiple
//   connections and b) connections from different kinds of session (e.g. websocket and repl and
//   telnet, etc) .
//   That or the 'session' gets implemented entirely separate from connection on the server side?
//   So we have "web socket connections" and "repl connections" that register themselves with
//   the session layer?

#[async_trait]
pub trait Session: Send + Sync {
    /// Commit for current activity, called by the scheduler when a task commits and *after* the world
    /// state has successfully been committed. This is the point at which the session should send
    /// its buffered output.
    /// The session should not be usable after this point.
    /// Note: there is no "two phase" process, so if I/O output fails, the world state will not be
    ///  rolled back. I/O output is not considered "critical" to the transaction's success, and
    ///  the world state's integrity and performance in that path is considered more important.
    ///  If this leads to weird symptoms, we can revisit this.
    // TODO: commit/rollback *could* consume `self` at this point, but this would make it difficult
    //   to manage e.g. mocking connections for unit tests etc. Can revisit.
    async fn commit(&self) -> Result<(), anyhow::Error>;

    /// Rollback for this session, called by the scheduler when a task rolls back and *after* the
    /// world state has successfully been rolled back.
    /// Should result in the session throwing away all buffered output.
    /// The session should not be usable after this point.
    async fn rollback(&self) -> Result<(), anyhow::Error>;

    /// "Fork" this session; create a new session which attaches to the same connection, but
    /// maintains its own buffer and state and can be committed/rolled back independently.
    /// Is used for forked tasks which end up running in their own transaction.
    /// Note: `disconnect` on one must also disconnect on all the other forks of the same lineage.
    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, anyhow::Error>;

    /// Spool output to the given player's connection.
    /// The actual output will not be sent until the task commits, and will be thrown out on
    /// rollback.
    async fn send_event(&self, player: Objid, event: NarrativeEvent) -> Result<(), anyhow::Error>;

    /// Send non-spooled output to the given player's connection
    /// Examples of the kinds of messages that would be sent here are state-independent messages
    /// like login/logout messages, system error messages ("task aborted") or messages that are not
    /// generally co-incident with the mutable state of the world.
    async fn send_system_msg(&self, player: Objid, msg: &str) -> Result<(), anyhow::Error>;

    /// Process a (wizard) request for system shutdown, with an optional shutdown message.
    async fn shutdown(&self, msg: Option<String>) -> Result<(), anyhow::Error>;

    /// The 'name' of the connection associated with the player.
    /// In a networked environment this is the hostname.
    /// LambdaMOO cores tend to expect this to be a resolved DNS hostname.
    // TODO: what do we do with the fact that a player may have multiple connections?
    async fn connection_name(&self, player: Objid) -> Result<String, anyhow::Error>;

    /// Disconnect the given player's connection.
    async fn disconnect(&self, player: Objid) -> Result<(), anyhow::Error>;

    /// Return the list of other currently-connected players.
    async fn connected_players(&self) -> Result<Vec<Objid>, anyhow::Error>;

    /// Return how many seconds the given player has been connected.
    async fn connected_seconds(&self, player: Objid) -> Result<f64, anyhow::Error>;

    /// Return how many seconds the given player has been idle (no tasks submitted).
    async fn idle_seconds(&self, player: Objid) -> Result<f64, anyhow::Error>;
}

/// A simple no-op implementation of the Sessions trait, for use in unit tests.
/// No output, and pretends no players are connected.
pub struct NoopClientSession {}
impl NoopClientSession {
    pub fn new() -> Self {
        NoopClientSession {}
    }
}

#[async_trait]
impl Session for NoopClientSession {
    async fn commit(&self) -> Result<(), Error> {
        Ok(())
    }
    async fn rollback(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, Error> {
        Ok(self.clone())
    }

    async fn send_event(&self, _player: Objid, _msg: NarrativeEvent) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn send_system_msg(&self, _player: Objid, _msg: &str) -> Result<(), Error> {
        Ok(())
    }

    async fn shutdown(&self, _msg: Option<String>) -> Result<(), Error> {
        Ok(())
    }
    async fn connection_name(&self, player: Objid) -> Result<String, Error> {
        Ok(format!("player-{}", player.0))
    }
    async fn disconnect(&self, _player: Objid) -> Result<(), Error> {
        Ok(())
    }
    async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        Ok(vec![])
    }

    async fn connected_seconds(&self, _player: Objid) -> Result<f64, Error> {
        Ok(0.0)
    }

    async fn idle_seconds(&self, _player: Objid) -> Result<f64, Error> {
        Ok(0.0)
    }
}

/// A 'mock' client connection which collects output in a vector of strings that tests can use to
/// verify output.
/// For now that's all it does, but facilities for pretending players are connected, mocking
/// hostnames, etc. can be added later.
struct Inner {
    received: Vec<String>,
    committed: Vec<String>,
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
    pub fn received(&self) -> Vec<String> {
        let inner = self.inner.read().unwrap();
        inner.received.clone()
    }
    pub fn committed(&self) -> Vec<String> {
        let inner = self.inner.read().unwrap();
        inner.committed.clone()
    }
    pub fn system(&self) -> Vec<String> {
        self.system.read().unwrap().clone()
    }
}
#[async_trait]
impl Session for MockClientSession {
    async fn commit(&self) -> Result<(), Error> {
        let mut inner = self.inner.write().unwrap();
        inner.committed = inner.received.clone();
        Ok(())
    }

    async fn rollback(&self) -> Result<(), Error> {
        self.inner.write().unwrap().received.clear();
        Ok(())
    }

    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, Error> {
        Ok(Arc::new(MockClientSession {
            inner: RwLock::new(Inner {
                received: vec![],
                committed: vec![],
            }),
            system: self.system.clone(),
        }))
    }

    async fn send_event(&self, _player: Objid, msg: NarrativeEvent) -> Result<(), Error> {
        self.inner
            .write()
            .unwrap()
            .received
            .push(msg.event().to_string());
        Ok(())
    }

    async fn send_system_msg(&self, player: Objid, msg: &str) -> Result<(), Error> {
        self.system
            .write()
            .unwrap()
            .push(format!("{}: {}", player.0, msg));
        Ok(())
    }

    async fn shutdown(&self, msg: Option<String>) -> Result<(), Error> {
        let mut system = self.system.write().unwrap();
        if let Some(msg) = msg {
            system.push(format!("shutdown: {}", msg));
        } else {
            system.push(String::from("shutdown"));
        }
        Ok(())
    }

    async fn connection_name(&self, player: Objid) -> Result<String, Error> {
        Ok(format!("player-{}", player))
    }

    async fn disconnect(&self, _player: Objid) -> Result<(), Error> {
        let mut system = self.system.write().unwrap();
        system.push(String::from("disconnect"));
        Ok(())
    }

    async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        Ok(vec![])
    }

    async fn connected_seconds(&self, _player: Objid) -> Result<f64, Error> {
        Ok(0.0)
    }

    async fn idle_seconds(&self, _player: Objid) -> Result<f64, Error> {
        Ok(0.0)
    }
}
