use crate::server::connection::Connection;
use crate::server::server::ConnectType::{Connected, Created, Reconnected};
use crate::server::{DisconnectReason, LoginType};
use anyhow::{anyhow, bail, Error};
use async_trait::async_trait;
use dashmap::DashMap;
use metrics_macros::{decrement_gauge, gauge, increment_counter, increment_gauge};
use moor_core::tasks::scheduler::{Scheduler, SchedulerError, TaskWaiterResult};
use moor_core::tasks::sessions::Session;
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;
use moor_values::var::variant::Variant;
use moor_values::var::{v_objid, v_string};
use moor_values::SYSTEM_OBJECT;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::Receiver;
use tokio::sync::Mutex;
use tracing::{trace, warn};

use super::ConnectType;

pub struct Server {
    connections: DashMap<Objid, Box<dyn Connection>>,
    scheduler: Scheduler,
    shutdown_sender: Sender<Option<String>>,
    // Downward counter for connection ids, starting at -4.
    next_connection_number: AtomicI64,
}

impl Server {
    pub fn new(scheduler: Scheduler, shutdown_sender: Sender<Option<String>>) -> Self {
        Self {
            connections: DashMap::new(),
            scheduler,
            shutdown_sender,
            next_connection_number: AtomicI64::new(-4),
        }
    }

    pub(crate) async fn new_session(
        self: Arc<Self>,
        player: Objid,
    ) -> anyhow::Result<Arc<BufferedSession>> {
        increment_counter!("server.new_session");
        let session = BufferedSession::new(player, self);
        Ok(Arc::new(session))
    }

    pub async fn new_connection<F: FnOnce(Objid) -> Result<Box<dyn Connection>, Error>>(
        &self,
        f: F,
    ) -> anyhow::Result<Objid> {
        increment_counter!("server.new_connection");
        let connection_oid = Objid(self.next_connection_number.fetch_sub(1, Ordering::SeqCst));
        let connection = f(connection_oid)?;
        self.connections.insert(connection_oid, connection);
        gauge!("server.connections", self.connections.len() as f64);
        Ok(connection_oid)
    }

    /// Marks that activity occurred on the given player's connection.
    /// Used for managing `idle_seconds`
    pub async fn record_activity(&self, player: Objid) -> anyhow::Result<()> {
        let Some(ref mut connection) = self.connections.get_mut(&player) else {
            warn!(
                "No connection for player: #{} during attempt at recording activity",
                player.0
            );

            // TODO: Not really an 'error' I suppose... ? Think about this.
            return Ok(());
        };
        connection.record_activity(Instant::now()).await;
        Ok(())
    }

    /// Send text to the given connection without going through the transactional buffering.
    /// Used by the server and by the internals of the connection itself.
    pub async fn write_messages(
        &self,
        request_author: Objid,
        messages: &[(Objid, NarrativeEvent)],
    ) -> anyhow::Result<()> {
        increment_counter!("server.write_messages");
        // To lazily hold the connections we're going to need...
        for (connection_destination, msg) in messages {
            let Some(ref mut conn) = self.connections.get_mut(connection_destination) else {
                // TODO This can be totally harmless, if a user disconnected while a transaction was in
                //  progress. But it can also be a sign of a bug, so we'll log it for now but remove the
                //  warning later.
                warn!(destination = ?connection_destination,
                      author = ?request_author, "No connection found");
                return Ok(());
            };
            let conn_player = conn.player().await;
            assert_eq!(conn_player, *connection_destination, "integrity error");

            conn.write_message(msg.clone()).await?;
        }

        Ok(())
    }

    /// Trigger $do_login_command with zero-args, to get the welcome message.
    pub async fn send_welcome_message(
        self: Arc<Self>,
        connection_oid: Objid,
    ) -> anyhow::Result<()> {
        increment_counter!("server.send_welcome_message");
        let Some(_) = self.connections.get_mut(&connection_oid) else {
            bail!(
                "No connection for {} during attempt at sending welcome message",
                connection_oid
            );
        };

        // Welcome message is triggered by invoking $do_login_command with zero-args. So the code
        // here roughly mimics `authenticate`
        trace!(?connection_oid, "send_welcome_message");
        let event_receiver = self.clone().do_login_command(connection_oid, &[]).await?;

        event_receiver.await?;
        Ok(())
    }

    /// Attempt authentication through $`do_login_command`( ... ) with fixed pre-determined
    /// arguments.
    pub async fn authenticate(
        self: Arc<Self>,
        connection_oid: Objid,
        login_type: LoginType,
        username: &str,
        password: &str,
    ) -> Result<Option<(ConnectType, Objid)>, Error> {
        increment_counter!("server.authenticate");
        let login_verb = match login_type {
            LoginType::Connect => {
                increment_counter!("server.authenticate.connect");
                "connect"
            }
            LoginType::Create => {
                increment_counter!("server.authenticate.create");
                "create"
            }
        };
        let event_receiver = self
            .clone()
            .do_login_command(
                connection_oid,
                &[
                    login_verb.to_string(),
                    username.to_string(),
                    password.to_string(),
                ],
            )
            .await?;

        self.finish_auth(connection_oid, login_type, event_receiver)
            .await
    }

    /// Attempt authentication through $`do_login_command`( ... ) with an arbitrary command set
    /// that may or may not be a create/connect command.
    pub async fn login_command_line(
        self: Arc<Self>,
        connection_oid: Objid,
        args: &[String],
    ) -> Result<Option<(ConnectType, Objid)>, Error> {
        if args.len() < 1 {
            bail!("No command line provided");
        }

        let event_receiver = self.clone().do_login_command(connection_oid, args).await?;

        // Wait on the event receiver. If it's success, then we authenticated. Otherwise, return
        // None -- because it's either auth failure or another kind of command executed.
        let login_type = if args[0] == "create" {
            LoginType::Create
        } else {
            LoginType::Connect
        };
        return self
            .finish_auth(connection_oid, login_type, event_receiver)
            .await;
    }

    async fn do_login_command(
        self: Arc<Self>,
        connection_oid: Objid,
        args: &[String],
    ) -> anyhow::Result<Receiver<TaskWaiterResult>> {
        increment_counter!("server.do_login_command");
        trace!(?connection_oid, "$do_login_command");
        let session = self.clone().new_session(connection_oid).await?;
        let task_id = self
            .clone()
            .scheduler
            .submit_verb_task(
                connection_oid,
                SYSTEM_OBJECT,
                "do_login_command".to_string(),
                args.into_iter().map(|s| v_string(s.clone())).collect(),
                SYSTEM_OBJECT,
                session,
            )
            .await?;
        let receiver = self.clone().scheduler.subscribe_to_task(task_id).await?;
        Ok(receiver)
    }

    async fn finish_auth(
        self: Arc<Self>,
        connection_oid: Objid,
        login_type: LoginType,
        event_receiver: Receiver<TaskWaiterResult>,
    ) -> Result<Option<(ConnectType, Objid)>, Error> {
        // Now we spin waiting for the task to complete.  The server will output to the connection obj
        // we created while that's happening.
        // We will wait on the subscription channel for this task,
        // And if it's successful and if it's an object that's our new player object to sign in as.
        // Otherwise, The Fail.
        let connect_result = event_receiver.await?;

        let TaskWaiterResult::Success(v) = connect_result else {
            bail!(
                "Execution failure in $do_login_command: {:?}",
                connect_result
            );
        };
        // Authentication failure if what was returned was not an object.
        let Variant::Obj(player) = v.variant() else {
            return Ok(None);
        };

        // We replace the transitory connection object with an actual player object.
        let Some((_, mut connection)) = self.connections.remove(&connection_oid) else {
            bail!("No connection for object: {:?}", connection_oid);
        };

        // Now stick the connection back in the map under the player object, updating it with the
        // player object, and letting it know if it was reconnected.
        connection.update_player(*player).await?;
        let login_result = match self.connections.insert(*player, connection) {
            Some(mut old_connection) => {
                increment_counter!("server.authenticate.reconnect");
                old_connection
                    .disconnect(DisconnectReason::Reconnected)
                    .await?;
                Reconnected
            }
            None => {
                if login_type == LoginType::Create {
                    increment_counter!("server.authenticate.create_success");
                    Created
                } else {
                    increment_counter!("server.authenticate.connect_success");
                    Connected
                }
            }
        };
        self.connections
            .get_mut(player)
            .unwrap()
            .notify_connected(*player, login_result)
            .await?;

        // Now submit $user_connected(player)/$user_reconnected(player) to the scheduler.
        // Which allows the core to send welcome messages, etc. to the user.
        self.submit_connected_task(*player, login_result).await;

        Ok(Some((login_result, *player)))
    }
    async fn submit_connected_task(self: Arc<Self>, player: Objid, initiation_type: ConnectType) {
        let session = self
            .clone()
            .new_session(player)
            .await
            .expect("could not create 'connected' task session for player");

        let connected_verb = match initiation_type {
            Connected => "user_connected".to_string(),
            Reconnected => "user_reconnected".to_string(),
            Created => "user_created".to_string(),
        };
        match self
            .scheduler
            .submit_verb_task(
                player,
                SYSTEM_OBJECT,
                connected_verb,
                vec![v_objid(player)],
                SYSTEM_OBJECT,
                session,
            )
            .await
        {
            Ok(_) => {
                trace!(player = ?player, "user_connected task submitted");
            }
            Err(e) => {
                warn!(player = ?player, "Could not issue user_connected task for connected player: {:?}", e);
            }
        }
    }

    pub async fn handle_inbound_command(
        self: Arc<Self>,
        player: Objid,
        cmd: &str,
    ) -> Result<(), SchedulerError> {
        increment_counter!("server.handle_inbound_command");

        // TODO: call :do_command first, and only call submit_command_task if that fails.
        let session = self
            .clone()
            .new_session(player)
            .await
            .expect("could not create 'command' task session for player");
        let task_id = self
            .scheduler
            .submit_command_task(player, cmd, session)
            .await?;
        // NOTE: The following will block the thread associated with the connection. Evaluate if
        //  this is going to lead to any concerns.
        match self.scheduler.subscribe_to_task(task_id).await {
            Ok(task_listener) => {
                let task_result = task_listener.await;
                match task_result {
                    Ok(TaskWaiterResult::Success(v)) => {
                        trace!(player = ?player, value = ?v, "command task completed successfully");
                    }
                    Ok(TaskWaiterResult::Error(scheduler_error)) => {
                        trace!(player = ?player, "command task aborted");
                        return Err(scheduler_error);
                    }
                    Err(e) => {
                        warn!(player = ?player, "command task failed: {:?}", e);
                        return Err(SchedulerError::CouldNotStartTask);
                    }
                }
            }
            Err(_) => return Err(SchedulerError::CouldNotStartTask),
        }
        Ok(())
    }

    /// Called when the listener has noticed that a connection is already closed.
    pub async fn disconnected(&self, player: Objid) -> anyhow::Result<Option<Box<dyn Connection>>> {
        increment_counter!("server.disconnected");
        let Some((_, conn)) = self.connections.remove(&player) else {
            warn!("No connection for object: {:?}", player);
            return Ok(None);
        };
        Ok(Some(conn))
    }

    pub async fn disconnect(&self, _requester: Objid, player: Objid) -> anyhow::Result<()> {
        increment_counter!("server.disconnect");
        let Some((_, mut conn)) = self.connections.remove(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        let conn_player = conn.player().await;
        assert_eq!(conn_player, player, "integrity error");
        conn.disconnect(DisconnectReason::None).await?;
        Ok(())
    }

    async fn connection_name(&self, _requester: Objid, player: Objid) -> Result<String, Error> {
        let Some(conn) = self.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        conn.connection_name(player).await
    }

    async fn connected_players(&self, _requester: Objid) -> Result<Vec<Objid>, Error> {
        self.connections.iter().map(|k| Ok(*k.pair().0)).collect()
    }

    async fn connected_seconds(&self, _requester: Objid, player: Objid) -> Result<Duration, Error> {
        let Some(conn) = self.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        let now = Instant::now();
        Ok(now - conn.connected_time().await)
    }

    async fn idle_seconds(&self, _requester: Objid, player: Objid) -> Result<Duration, Error> {
        increment_counter!("ws_host.sessions.request.idle_seconds");
        let Some(conn) = self.connections.get(&player) else {
            return Err(anyhow!("no known connection for objid: #{}", player.0));
        };
        let now = Instant::now();
        Ok(now - conn.last_activity().await)
    }
}

// A per-transaction `session` which holds an internal buffer and dispatches it up to the server
// to send to the appropriate connection.
pub struct BufferedSession {
    player: Objid,
    server: Arc<Server>,
    // TODO: manage this buffer better -- e.g. if it grows too big, for long-running tasks, etc. it
    //  should be mmap'd to disk or something.
    session_buffer: Mutex<Vec<(Objid, NarrativeEvent)>>,
}

impl BufferedSession {
    pub fn new(player: Objid, server: Arc<Server>) -> Self {
        increment_gauge!("server.buffered_sessions", 1.0);
        Self {
            player,
            server,
            session_buffer: Mutex::new(vec![]),
        }
    }
}
impl Drop for BufferedSession {
    fn drop(&mut self) {
        decrement_gauge!("server.buffered_sessions", 1.0)
    }
}

#[async_trait]
impl Session for BufferedSession {
    async fn commit(&self) -> Result<(), Error> {
        increment_counter!("buffered_session.commit");
        let mut buffer = self.session_buffer.lock().await;
        trace!(
            player = ?self.player,
            num_events = buffer.len(),
            "Flushing session"
        );

        let messages: Vec<(Objid, NarrativeEvent)> = buffer.drain(..).collect();
        self.server.write_messages(self.player, &messages).await?;
        Ok(())
    }

    async fn rollback(&self) -> Result<(), Error> {
        increment_counter!("buffered_session.rollback");
        let mut buffer = self.session_buffer.lock().await;
        buffer.clear();
        Ok(())
    }

    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, Error> {
        Ok(Arc::new(Self {
            player: self.player,
            server: self.server.clone(),
            session_buffer: Default::default(),
        }))
    }

    async fn send_event(&self, player: Objid, msg: NarrativeEvent) -> Result<(), Error> {
        increment_counter!("buffered_session.send_text");
        let mut buffer = self.session_buffer.lock().await;
        buffer.push((player, msg));
        Ok(())
    }

    async fn send_system_msg(&self, player: Objid, msg: &str) -> Result<(), Error> {
        increment_counter!("buffered_session.send_system_msg");
        // TODO: not all system messages should be durable...
        self.server
            .write_messages(
                self.player,
                &[(
                    player,
                    NarrativeEvent::new_durable(SYSTEM_OBJECT, msg.to_string()),
                )],
            )
            .await
    }

    async fn shutdown(&self, msg: Option<String>) -> Result<(), Error> {
        increment_counter!("buffered_session.shutdown");
        if let Some(msg) = msg.clone() {
            self.send_system_msg(self.player, &msg).await?;
        }
        self.server.shutdown_sender.send(msg).await.unwrap();
        Ok(())
    }

    async fn connection_name(&self, player: Objid) -> Result<String, Error> {
        let conn_string = self.server.connection_name(self.player, player).await?;
        Ok(conn_string)
    }

    async fn disconnect(&self, player: Objid) -> Result<(), Error> {
        increment_counter!("buffered_session.disconnect");
        self.server.disconnect(self.player, player).await?;
        Ok(())
    }

    async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        self.server.connected_players(self.player).await
    }

    async fn connected_seconds(&self, player: Objid) -> Result<f64, Error> {
        let duration = self.server.connected_seconds(self.player, player).await?;
        Ok(duration.as_secs_f64())
    }

    async fn idle_seconds(&self, player: Objid) -> Result<f64, Error> {
        let duration = self.server.idle_seconds(self.player, player).await?;
        Ok(duration.as_secs_f64())
    }
}
