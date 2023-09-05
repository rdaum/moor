use std::net::SocketAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use anyhow::bail;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::headers::authorization::Basic;
use axum::headers::Authorization;
use axum::response::IntoResponse;
use axum::TypedHeader;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{StreamExt, TryStreamExt};
use metrics_macros::increment_counter;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, trace, warn};

use moor_lib::tasks::scheduler::{Scheduler, SchedulerError, TaskWaiterResult};
use moor_value::var::objid::Objid;
use moor_value::var::variant::Variant;
use moor_value::var::{v_objid, v_str};
use moor_value::SYSTEM_OBJECT;

use crate::server::ws_sessions::WebSocketSessions;

#[derive(Clone)]
pub struct WebSocketServer {
    inner: Arc<RwLock<Inner>>,
}

struct Inner {
    sessions: Arc<RwLock<WebSocketSessions>>,
    scheduler: Scheduler,
    // Downward counter for connection ids, starting at -1.
    next_connection_number: AtomicI64,
}

impl WebSocketServer {
    pub fn new(scheduler: Scheduler, shutdown_sender: Sender<Option<String>>) -> Self {
        let sessions = WebSocketSessions::new(shutdown_sender);
        Self {
            inner: Arc::new(RwLock::new(Inner {
                scheduler,
                sessions: Arc::new(RwLock::new(sessions)),
                // Start at #-4, since #-3 and above are reserved.
                next_connection_number: AtomicI64::new(-4),
            })),
        }
    }
}

/// Handles connection to an existing player, via websocket connection & basic-auth.
pub async fn ws_connect_handler(
    ws: WebSocketUpgrade,
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_server): State<WebSocketServer>,
) -> impl IntoResponse {
    increment_counter!("ws_server.new_connection");

    info!("Connection from {}", addr);
    // TODO: only async Rust could produce an entity as demonic as this. Let's go on and pretend the
    // pain is all worth it.
    ws.on_upgrade(
        move |socket| async move { ws_server.handle_player_connect(addr, socket, auth).await },
    )
}

/// Handles the attempt to create a new player, via websocket connection & basic-auth.
pub async fn ws_create_handler(
    ws: WebSocketUpgrade,
    TypedHeader(auth): TypedHeader<Authorization<Basic>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ws_server): State<WebSocketServer>,
) -> impl IntoResponse {
    increment_counter!("ws_server.new_connection");

    info!("Connection from {}", addr);
    ws.on_upgrade(
        move |socket| async move { ws_server.handle_player_create(addr, socket, auth).await },
    )
}

enum SessionInitiation {
    Connected,
    Reconnected,
    Created,
}

enum LoginType {
    Connect,
    Create,
}

impl WebSocketServer {
    /// Websocket session handling for `connect` to an existing player with basic-auth credentials.
    #[instrument(skip(self, stream, auth))]
    pub async fn handle_player_connect(
        &self,
        peer: SocketAddr,
        stream: WebSocket,
        auth: Authorization<Basic>,
    ) {
        let (ws_sender, ws_receiver) = stream.split();

        // Get a connection number, registered in the server.
        let connection_oid = self.create_connection(peer, ws_sender).await;

        let (player, ws_sender) = match self
            .call_do_login_command(connection_oid, auth, LoginType::Connect)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                increment_counter!("ws_server.login_failure");
                warn!("Login failure: {}", e.to_string());
                self.deregister_connection(connection_oid).await;
                return;
            }
        };
        increment_counter!("ws_server.connect_player_success");

        // Register connection with player.
        let Ok(is_reconnected) = self.register_connection(ws_sender, peer, player).await else {
            increment_counter!("ws_server.connection_registration_failure");
            error!("Failed to register connection");
            return;
        };

        let initiation_type = if is_reconnected {
            SessionInitiation::Reconnected
        } else {
            SessionInitiation::Connected
        };

        // And thus the user is logged in.
        self.player_connection(player, peer, ws_receiver, initiation_type)
            .await;
    }

    /// Websocket session handling for `create` to for a new player with basic-auth credentials to
    /// establish the user's new player.
    #[instrument(skip(self, stream, auth))]
    pub async fn handle_player_create(
        &self,
        peer: SocketAddr,
        stream: WebSocket,
        auth: Authorization<Basic>,
    ) {
        let (ws_sender, ws_receiver) = stream.split();

        // Get a connection number, registered in the server.
        let connection_oid = self.create_connection(peer, ws_sender).await;

        let (player, ws_sender) = match self
            .call_do_login_command(connection_oid, auth, LoginType::Create)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                increment_counter!("ws_server.create_failure");
                warn!("Create failure: {}", e.to_string());
                self.deregister_connection(connection_oid).await;
                return;
            }
        };
        increment_counter!("ws_server.create_user_success");

        // Register connection with player.
        let Ok(_) = self.register_connection(ws_sender, peer, player).await else {
            increment_counter!("ws_server.connection_registration_failure");
            error!("Failed to register connection");
            return;
        };

        // And thus the user is logged in as a new player.
        self.player_connection(player, peer, ws_receiver, SessionInitiation::Created)
            .await;
    }

    /// The actual core websocket handling loop for an authenticated (connected/created) player.
    async fn player_connection(
        &self,
        player: Objid,
        peer: SocketAddr,
        ws_receiver: SplitStream<WebSocket>,
        initiation_type: SessionInitiation,
    ) {
        match initiation_type {
            SessionInitiation::Connected => {
                increment_counter!("ws_server.user_connected")
            }
            SessionInitiation::Reconnected => {
                increment_counter!("ws_server.user_reconnected")
            }
            SessionInitiation::Created => {
                increment_counter!("ws_server.user_created")
            }
        }

        // Now submit $user_connected(player)/$user_reconnected(player) to the scheduler.
        // Which allows the core to send welcome messages, etc. to the user.
        self.submit_connected_task(player, initiation_type).await;

        // Core entry/task submission loop, runs as long as the connection 'tis open.
        self.submission_loop(player, ws_receiver).await;

        // Now drop the connection from sessions.
        self.deregister_connection(player).await;
        info!("WebSocket session finished: {}", peer);
    }
    async fn submission_loop(&self, player: Objid, mut ws_receiver: SplitStream<WebSocket>) {
        while let Ok(Some(msg)) = ws_receiver.try_next().await {
            let cmd = match msg.into_text() {
                Ok(cmd) => cmd,
                Err(e) => {
                    increment_counter!("ws_server.error_decoding_message");
                    error!("Error decoding a message: {:?}", e);
                    continue;
                }
            };
            increment_counter!("ws_server.message_received");
            let cmd = cmd.as_str().trim();

            // Record activity on the connection, to compute idle_seconds.
            {
                let inner = self.inner.read().await;
                let mut sessions = inner.sessions.write().await;
                if let Err(e) = sessions.record_activity(player) {
                    warn!(player = ?player, "Error recording activity on connection: {:?}", e)
                }
            }
            let task_id = {
                let inner = self.inner.read().await;
                let sessions = inner.sessions.clone();
                let session = WebSocketSessions::new_session(sessions, player)
                    .await
                    .expect("could not create 'command' task session for player");
                inner
                    .scheduler
                    .submit_command_task(player, cmd, session)
                    .await
            };
            if let Err(e) = task_id {
                increment_counter!("ws_server.submit_error");

                match e {
                    SchedulerError::CouldNotParseCommand(_)
                    | SchedulerError::NoCommandMatch(_, _) => {
                        self.send_error(player, "I don't understand that.".to_string())
                            .await
                            .unwrap();
                    }
                    SchedulerError::PermissionDenied => {
                        self.send_error(player, "You can't do that.".to_string())
                            .await
                            .unwrap();
                    }
                    _ => {
                        self.send_error(
                            player,
                            "Internal error. Let your nearest wizard know".to_string(),
                        )
                        .await
                        .unwrap();
                        error!(player=?player, command=cmd, error=?e, "Internal error in command submission");
                    }
                }
            }
        }
    }

    async fn send_error(&self, player: Objid, msg: String) -> Result<(), anyhow::Error> {
        let inner = self.inner.read().await;
        inner
            .sessions
            .clone()
            .write()
            .await
            .write_msg(player, msg.as_str())
            .await
    }

    async fn create_connection(
        &self,
        peer: SocketAddr,
        ws_sender: SplitSink<WebSocket, Message>,
    ) -> Objid {
        {
            let inner = self.inner.read().await;
            let mut sessions = inner.sessions.write().await;
            // TODO: move next_connection_number to sessions? Or should scheduler in fact be managing this?
            let connection_oid = Objid(inner.next_connection_number.fetch_sub(1, Ordering::SeqCst));
            sessions
                .register_connection(connection_oid, peer, ws_sender)
                .await
                .expect("new connection");
            connection_oid
        }
    }

    async fn call_do_login_command(
        &self,
        connection_oid: Objid,
        auth: Authorization<Basic>,
        login_type: LoginType,
    ) -> Result<(Objid, SplitSink<WebSocket, Message>), anyhow::Error> {
        let login_verb = match login_type {
            LoginType::Connect => "connect",
            LoginType::Create => "create",
        };
        let event_receiver = {
            trace!(?connection_oid, "$do_login_command");
            // Call the scheduler to initiate $do_login_command
            let inner = self.inner.read().await;
            let session =
                WebSocketSessions::new_session(inner.sessions.clone(), connection_oid).await?;

            let task_id = inner
                .scheduler
                .submit_verb_task(
                    connection_oid,
                    SYSTEM_OBJECT,
                    "do_login_command".to_string(),
                    vec![
                        v_str(login_verb),
                        v_str(auth.username()),
                        v_str(auth.password()),
                    ],
                    SYSTEM_OBJECT,
                    session,
                )
                .await
                .unwrap();

            inner.scheduler.subscribe_to_task(task_id).await?
        };

        // Now we spin waiting for the task to complete.  The server will output to the connection obj
        // we created while that's happening
        // We will wait on the subscription channel for this task,
        // And if it's successful and if it's an object that's our new player object to sign in as.
        // Otherwise, The Fail.
        let connect_result = event_receiver.await?;
        let (player, ws_sender) = match connect_result {
            TaskWaiterResult::Success(v) => {
                let Variant::Obj(player) = v.variant() else {
                    bail!("Login failure from $do_login_command: {:?}", v);
                };

                let inner = self.inner.read().await;
                let sessions = &mut inner.sessions.write().await;
                let Some((ws_sender, _)) = sessions.unregister_connection(connection_oid).await?
                else {
                    bail!("No connection for object: {:?}", connection_oid);
                };
                (*player, ws_sender)
            }
            _ => {
                bail!("login failure");
            }
        };

        Ok((player, ws_sender))
    }

    async fn register_connection(
        &self,
        ws_sender: SplitSink<WebSocket, Message>,
        peer: SocketAddr,
        player: Objid,
    ) -> Result<bool, anyhow::Error> {
        let inner = self.inner.read().await;
        let mut sessions = inner.sessions.write().await;
        let is_reconnected = sessions
            .register_connection(player, peer, ws_sender)
            .await?;
        let connect_msg = if is_reconnected {
            "** Redirecting old connection to this port **"
        } else {
            "** Connected **"
        };
        sessions.write_msg(player, connect_msg).await?;
        Ok(is_reconnected)
    }

    async fn deregister_connection(&self, connection_object: Objid) {
        let inner = self.inner.read().await;
        let sessions = &mut inner.sessions.write().await;
        // TODO: properly handle reconnects.
        let (_, peer_addr) = match sessions.unregister_connection(connection_object).await {
            Ok(Some(connection)) => connection,
            Ok(None) => {
                trace!(
                    ?connection_object,
                    "connection already removed / no connection for object"
                );
                return;
            }
            Err(e) => {
                error!(
                    ?connection_object,
                    "error deregistering connection: {:?}", e
                );
                return;
            }
        };
        info!(player = ?connection_object, address = ?peer_addr, "disconnected");
        increment_counter!("ws_server.connection_finished");
    }

    async fn submit_connected_task(&self, player: Objid, initiation_type: SessionInitiation) {
        let sessions = self.inner.read().await.sessions.clone();
        let session = WebSocketSessions::new_session(sessions.clone(), player)
            .await
            .expect("could not create 'connected' task session for player");

        let connected_verb = match initiation_type {
            SessionInitiation::Connected => "user_connected".to_string(),
            SessionInitiation::Reconnected => "user_reconnected".to_string(),
            SessionInitiation::Created => "user_created".to_string(),
        };
        match self
            .inner
            .write()
            .await
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
}
