use crate::server::connection::Connection;
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;
use std::collections::BTreeMap;
use std::time::SystemTime;

/// The stream of events is committed into a database, and when a new connection is established,
/// the player is "replayed" from the database back to the new connection from the point where they
/// left off (or from where the client requests it)
pub struct NarrativeLog {
    /// The events that happened on this connection, since forever.
    /// This is a list of (timestamp, event) pairs consisting of the time when the event happened
    /// and the event itself.
    /// This is kept sorted based on event timestamp so that we can replay the events in proper
    /// order.
    // TODO: Grows unbounded. this will be replaced by a database at a later date.
    event_stream: BTreeMap<SystemTime, NarrativeEvent>,
    /// The player that this log is for.
    player: Objid,
    /// The currently attached delegate connections (if any) to which live events are forwarded
    /// as they come in.
    connections: Vec<Box<dyn Connection>>,
    /// The time when this connection was created
    creation_time: SystemTime,
    /// The time of the last activity on this connection
    last_activity: SystemTime,
}

impl NarrativeLog {
    pub fn new(player: Objid, connection: Box<dyn Connection>) -> Result<Self, anyhow::Error> {
        Ok(Self {
            event_stream: BTreeMap::new(),
            player,
            connections: vec![connection],
            creation_time: SystemTime::now(),
            last_activity: SystemTime::now(),
        })
    }
    pub async fn attach(&mut self, delegate: Box<dyn Connection>, _fork_time: SystemTime) {
        self.connections.push(delegate);
        // TODO: replay events since 'fork time' to the new delegate.
        // self.replay_events(fork_time, self.delegates.last_mut().unwrap());
    }
    pub async fn detach(&mut self, delegate: Box<dyn Connection>) {
        self.connections.retain(|d| !std::ptr::eq(&delegate, d));
    }
}
