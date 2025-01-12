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

use crate::host::Host;
use crate::var_to_js_value;
use moor_values::model::ObjectRef;
use moor_values::tasks::Event;
use moor_values::{v_none, Obj, Symbol, SYSTEM_OBJECT};
use neon::context::{Context, FunctionContext};
use neon::object::Object;
use neon::prelude::{
    Finalize, Handle, JsArray, JsBox, JsFunction, JsPromise, JsResult, JsString, JsValue,
};
use rpc_async_client::pubsub_client::{broadcast_recv, events_recv};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::HostClientToDaemonMessage::ConnectionEstablish;
use rpc_common::{
    AuthToken, ClientEvent, ClientToken, ClientsBroadcastEvent, DaemonToClientReply,
    HostClientToDaemonMessage, HostType, ReplyResult, RpcError, CLIENT_BROADCAST_TOPIC,
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tmq::{request, subscribe};
use tokio::select;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Wraps up a connection to the daemon.
pub struct ConnectionHandle {
    inner: Arc<Mutex<ConnectionInner>>,
}

struct ConnectionInner {
    #[allow(dead_code)]
    connection_oid: Obj,
    client_token: ClientToken,
    auth_token: Option<AuthToken>,
    sender: tokio::sync::mpsc::Sender<(
        tokio::sync::oneshot::Sender<Result<ReplyResult, RpcError>>,
        HostClientToDaemonMessage,
    )>,
}

impl Finalize for ConnectionHandle {
    fn finalize<'a, C: Context<'a>>(self, _: &mut C) {
        info!("Connection dropped");
    }
}

/// Initiat ea login event for a connection.
pub fn connection_login(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let connection = cx.argument::<JsBox<ConnectionHandle>>(0)?;
    let verb = cx.argument::<JsString>(1)?.value(&mut cx);
    let username = cx.argument::<JsString>(2)?.value(&mut cx);
    let password = cx.argument::<JsString>(3)?.value(&mut cx);

    let runtime = crate::runtime(&mut cx)?;
    let channel = cx.channel();

    let (deferred, promise) = cx.promise();

    let (sender, client_token) = {
        let connection = connection.inner.lock().unwrap();
        (connection.sender.clone(), connection.client_token.clone())
    };
    let connection = connection.inner.clone();
    runtime.spawn(async move {
        let (reply, receive) = tokio::sync::oneshot::channel();
        if let Err(e) = sender
            .send((
                reply,
                HostClientToDaemonMessage::LoginCommand(
                    client_token,
                    SYSTEM_OBJECT,
                    vec![verb, username, password],
                    true,
                ),
            ))
            .await
        {
            info!("Unable to send login command: {:?}", e);
        }

        let result = match receive.await {
            Ok(Ok(ReplyResult::ClientSuccess(DaemonToClientReply::LoginResult(Some((
                auth_token,
                connect_type,
                player,
            )))))) => {
                info!("Login successful: {:?}", auth_token);
                Ok((auth_token, connect_type, player))
            }
            Ok(Ok(ReplyResult::Failure(f))) => {
                info!("Login failure: {:?}", f);
                Err(format!("Login failure: {:?}", f))
            }
            Ok(Err(e)) => {
                info!("Error in login response: {:?}", e);
                Err(format!("Error in login response: {:?}", e))
            }
            Ok(Ok(_)) => {
                info!("Unexpected response from login");
                Err("Unexpected response from login".to_string())
            }
            Err(e) => {
                info!("Unable to receive login response: {:?}", e);
                Err(format!("Unable to receive login response: {:?}", e))
            }
        };

        deferred.settle_with(&channel, move |mut cx| match result {
            Ok((auth_token, _connect_type, player)) => {
                // Set the auth token on the connection handle
                {
                    let mut connection = connection.lock().unwrap();
                    connection.auth_token = Some(auth_token.clone());
                }

                // And also return it.
                let auth_token = cx.string(auth_token.0);
                let player = cx.number(player.id().0);
                let array = JsArray::new(&mut cx, 2);
                array.set(&mut cx, 0, auth_token)?;
                array.set(&mut cx, 1, player)?;

                Ok(array)
            }
            Err(e) => cx.throw_error(e),
        });
    });

    Ok(promise)
}

pub fn new_connection(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let host = cx.argument::<JsBox<Host>>(0)?;
    let rpc_address = cx.argument::<JsString>(1)?.value(&mut cx);
    let events_address = cx.argument::<JsString>(2)?.value(&mut cx);
    let peer_addr = cx.argument::<JsString>(3)?.value(&mut cx);

    let Ok(peer_addr) = peer_addr.parse::<SocketAddr>() else {
        return cx.throw_error(format!("Unable to parse peer address: {}", peer_addr));
    };

    let mut system_message_callback = cx.argument::<JsFunction>(4)?.root(&mut cx);
    let mut narrative_event_callback = cx.argument::<JsFunction>(5)?.root(&mut cx);
    let mut request_input_callback = cx.argument::<JsFunction>(6)?.root(&mut cx);
    let disconnect_callback = cx.argument::<JsFunction>(7)?.root(&mut cx);
    let mut task_error_callback = cx.argument::<JsFunction>(8)?.root(&mut cx);
    let mut task_success_callback = cx.argument::<JsFunction>(9)?.root(&mut cx);

    let host = host.inner.clone();

    let (zmq_ctx, kill_switch) = {
        let host = host.lock().unwrap();
        (host.zmq_ctx.clone(), host.kill_switch.clone())
    };

    let runtime = crate::runtime(&mut cx)?;
    let channel = cx.channel();

    let (deferred, promise) = cx.promise();

    runtime.spawn(async move {
        let client_id = Uuid::new_v4();
         let rpc_request_sock = match request(&zmq_ctx)
            .set_rcvtimeo(100)
            .set_sndtimeo(100)
            .connect(rpc_address.as_str()) {
             Ok(r) => {r}
             Err(e) => {
                 deferred.settle_with(&channel, move |mut cx| {
                     cx.throw_error::<String, neon::handle::Handle<'_, JsBox<ConnectionHandle>>>(format!("Unable to connect to RPC server: {}", e))
                 });
                 return;
             }
         };


        // And let the RPC server know we're here, and it should start sending events on the
        // narrative subscription.
        let mut rpc_client = RpcSendClient::new(rpc_request_sock);
        let (client_token, connection_oid) = match rpc_client
            .make_client_rpc_call(client_id, ConnectionEstablish(peer_addr.to_string()))
            .await
        {
            Ok(ReplyResult::ClientSuccess(DaemonToClientReply::NewConnection(token, objid))) => {
                debug!("Connection established, connection ID: {}", objid);
                (token, objid)
            }
            Ok(ReplyResult::Failure(f)) => {
                deferred.settle_with(&channel, move |mut cx| {
                    cx.throw_error::<String, neon::handle::Handle<'_, JsBox<ConnectionHandle>>>(format!("Failure response from RPC server: {:?}", f))
                });
                return;
            }
            Ok(r) => {
                deferred.settle_with(&channel, move |mut cx| {
                    cx.throw_error::<String, neon::handle::Handle<'_, JsBox<ConnectionHandle>>>(format!("Unexpected response from RPC server: {:?}", r))
                });
                return;
            }
            Err(e) => {
                deferred.settle_with(&channel, move |mut cx| {
                    cx.throw_error::<String, neon::handle::Handle<'_, JsBox<ConnectionHandle>>>(format!("Unable to establish connection: {}", e))
                });
                return;
            }
        };
        debug!(client_id = ?client_id, connection = ?connection_oid, "Connection established");

        // Before attempting login, we subscribe to the events socket, using our client
        // id. The daemon should be sending events here.
        let events_sub = match subscribe(&zmq_ctx)
            .connect(events_address.as_str())
        {
            Ok(s) => {s}
            Err(e) => {
                deferred.settle_with(&channel, move |mut cx| {
                    cx.throw_error::<String, neon::handle::Handle<'_, JsBox<ConnectionHandle>>>(format!("Unable to connect to events socket: {}", e))
                });
                return;
            }
        };
        let mut events_sub = match events_sub
            .subscribe(&client_id.as_bytes()[..])
        {
            Ok(s) => {s}
            Err(e) => {
                deferred.settle_with(&channel, move |mut cx| {
                    cx.throw_error::<String, neon::handle::Handle<'_, JsBox<ConnectionHandle>>>(format!("Unable to subscribe to events socket: {}", e))
                });
                return;
            }
        };

        let broadcast_sub = match subscribe(&zmq_ctx)
            .connect(events_address.as_str())
        {
            Ok(s) => {s}
            Err(e) => {
                deferred.settle_with(&channel, move |mut cx| {
                    cx.throw_error::<String, neon::handle::Handle<'_, JsBox<ConnectionHandle>>>(format!("Unable to connect to broadcast subscriber: {}", e))
                });
                return;
            }
        };

        let mut broadcast_sub = match  broadcast_sub
            .subscribe(CLIENT_BROADCAST_TOPIC)
        {
            Ok(s) => {s}
            Err(e) => {
                deferred.settle_with(&channel, move |mut cx| {
                    cx.throw_error::<String, neon::handle::Handle<'_, JsBox<ConnectionHandle>>>(format!("Unable to subscribe to broadcast messages for client connection: {}", e))
                });
                return;
            }
        };


        info!(
            "Subscribed on pubsub events socket for {:?}, socket addr {}",
            client_id, events_address
        );

        let (conn_send, mut conn_recv) = tokio::sync::mpsc::channel(10);

        let conn_handle = ConnectionHandle {
            inner: Arc::new(Mutex::new(ConnectionInner {
                connection_oid: connection_oid.clone(),
                client_token: client_token.clone(),
                auth_token: None,
                sender: conn_send.clone(),
            }))
        };

        deferred.settle_with(&channel, move |mut cx| {
            let handle = cx.boxed(conn_handle);
            info!("Connection established, promise fulfilled to: {:?}", handle);
            Ok(handle)
        });

        debug!("Entering connection loop");
        loop {
            if kill_switch.load(std::sync::atomic::Ordering::SeqCst) {
                info!("Kill switch activated, stopping...");
                break;
            }

            select! {
                // Receive messages from conn_recv and turn them into outbound messages
                Some((reply, msg)) = conn_recv.recv() => {
                    if let Err(e) = reply.send(rpc_client.make_client_rpc_call(client_id, msg).await) {
                        error!("Unable to send reply: {:?}", e);
                        return;
                    }
                }

                Ok(event) = broadcast_recv(&mut broadcast_sub) => {
                    match event {
                        ClientsBroadcastEvent::PingPong(_server_time) => {
                            let _ = rpc_client.make_client_rpc_call(client_id,
                                HostClientToDaemonMessage::ClientPong(client_token.clone(), SystemTime::now(), connection_oid.clone(), HostType::WebSocket, peer_addr.clone())).await;
                        }
                    }
                }
                Ok(event) = events_recv(client_id.clone(), &mut events_sub) => {
                    match event {
                        ClientEvent::SystemMessage(_author, msg) => {
                            debug!("System message: {}", msg);
                            let continuation = channel.send(move |mut cx| {
                                let callback = system_message_callback.clone(&mut cx);
                                let callback = callback.into_inner(&mut cx);
                                let msg = cx.string(msg);
                                let msg: Handle<JsValue> = msg.upcast();
                                let undefined = cx.undefined();
                                let Ok(_) = callback.call(&mut cx, undefined, vec![msg]) else {
                                    return cx.throw_error("Unable to call system message callback");
                                };
                                Ok(system_message_callback)
                            }).join();
                            system_message_callback = match continuation {
                                Ok(continuation) => continuation,
                                Err(e) => {
                                    info!("Unable to schedule continuation: {}", e);
                                    break;
                                }
                            };
                        }
                        ClientEvent::Narrative(_author, event) => {
                            debug!("Narrative event: {:?}", event);
                            let continuation = channel.send(move |mut cx| {
                                let callback = narrative_event_callback.clone(&mut cx);
                                let callback = callback.into_inner(&mut cx);
                                let (event, content_type) = match event.event {
                                    Event::Notify(what, content_type) => {
                                        let v = match var_to_js_value(&mut cx, &what) {
                                            Ok(v) => v,
                                            Err(e) => {
                                                return cx.throw_error(e.to_string());
                                            }
                                        };

                                        let c : Handle<JsValue> = match content_type {
                                            Some(c) => {
                                                let c = cx.string(c.as_str());
                                                c.upcast()
                                            }
                                            None => cx.undefined().upcast()
                                        };

                                        (v, c)
                                    }


                                };

                                let event: Handle<JsValue> = event.upcast();
                                let undefined = cx.undefined();
                                let Ok(_) = callback.call(&mut cx, undefined, vec![event, content_type]) else {
                                    return cx.throw_error("Unable to call narrative event callback");
                                };
                                Ok(narrative_event_callback)
                            }).join();
                            narrative_event_callback = match continuation {
                                Ok(continuation) => continuation,
                                Err(e) => {
                                    info!("Unable to schedule continuation: {}", e);
                                    break;
                                }
                            };
                        }
                        ClientEvent::RequestInput(request_id) => {
                            debug!("Requesting input for request ID: {}", request_id);
                            // Server is requesting some input back through corelated with `request_id`
                            let continuation = channel.send(move |mut cx| {
                                let callback = request_input_callback.clone(&mut cx);
                                let callback = callback.into_inner(&mut cx);
                                let request_id = cx.string(request_id.to_string());
                                let request_id: Handle<JsValue> = request_id.upcast();
                                let undefined = cx.undefined();
                                let Ok(_) = callback.call(&mut cx, undefined, vec![request_id]) else {
                                    return cx.throw_error("Unable to call request input callback");
                                };
                                Ok(request_input_callback)
                            }).join();
                            request_input_callback = match continuation {
                                Ok(continuation) => continuation,
                                Err(e) => {
                                    info!("Unable to schedule continuation: {}", e);
                                    break;
                                }
                            };
                        }
                        ClientEvent::Disconnect() => {
                            debug!("Disconnecting");
                            channel.send(move |mut cx| {
                                let callback = disconnect_callback.clone(&mut cx);
                                let callback = callback.into_inner(&mut cx);
                                let undefined = cx.undefined();
                                let Ok(_) = callback.call(&mut cx, undefined, vec![]) else {
                                    return cx.throw_error("Unable to call disconnect callback");
                                };
                                Ok(disconnect_callback)
                            });
                            return;
                        }
                        ClientEvent::TaskError(_ti, te) => {
                            debug!("Task error: {:?}", te);
                            let continuation = channel.send(move |mut cx| {
                                let callback = task_error_callback.clone(&mut cx);
                                let callback = callback.into_inner(&mut cx);
                                let te = te.to_string();
                                let te = cx.string(te);
                                let te: Handle<JsValue> = te.upcast();
                                let undefined = cx.undefined();
                                let Ok(_) = callback.call(&mut cx, undefined, vec![te]) else {
                                    return cx.throw_error("Unable to call task error callback");
                                };
                                Ok(task_error_callback)
                            }).join();
                            task_error_callback = match continuation {
                                Ok(continuation) => continuation,
                                Err(e) => {
                                    info!("Unable to schedule continuation: {}", e);
                                    break;
                                }
                            };
                        }
                        ClientEvent::TaskSuccess(ti, _result) => {
                            debug!("Task success");
                            let continuation = channel.send(move |mut cx| {
                                let callback = task_success_callback.clone(&mut cx);
                                let callback = callback.into_inner(&mut cx);
                                let task_id = cx.number(ti as f64).upcast();
                                let undefined = cx.undefined();
                                let Ok(_) = callback.call(&mut cx, undefined, vec![task_id]) else {
                                    return cx.throw_error("Unable to call task success callback");
                                };
                                Ok(task_success_callback)
                            }).join();
                            task_success_callback = match continuation {
                                Ok(continuation) => continuation,
                                Err(e) => {
                                    info!("Unable to schedule continuation: {}", e);
                                    break;
                                }
                            };
                        }
                    }
                }
            }
        }
    });

    Ok(promise)
}

/// Transmit a message to the daemon over this connection
pub fn connection_command(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let connection = cx.argument::<JsBox<ConnectionHandle>>(0)?;
    let message = cx.argument::<JsString>(1)?.value(&mut cx);

    let runtime = crate::runtime(&mut cx)?;
    let channel = cx.channel();

    let (deferred, promise) = cx.promise();

    let (sender, client_token, auth_token) = {
        let connection = connection.inner.lock().unwrap();

        let Some(auth_token) = connection.auth_token.clone() else {
            return cx.throw_error("Connection not logged in");
        };

        (
            connection.sender.clone(),
            connection.client_token.clone(),
            auth_token,
        )
    };

    runtime.spawn(async move {
        let (reply, receive) = tokio::sync::oneshot::channel();
        if let Err(e) = sender
            .send((
                reply,
                HostClientToDaemonMessage::Command(
                    client_token,
                    auth_token,
                    SYSTEM_OBJECT,
                    message,
                ),
            ))
            .await
        {
            error!("Unable to send message: {:?}", e);
            deferred.settle_with(&channel, move |mut cx| {
                cx.throw_error::<String, neon::handle::Handle<'_, JsPromise>>(format!(
                    "Unable to send message: {:?}",
                    e
                ))
            });
            return;
        }

        let result = match receive.await {
            Ok(Ok(ReplyResult::ClientSuccess(DaemonToClientReply::TaskSubmitted(task_id)))) => {
                debug!("Task submitted: {:?}", task_id);
                Ok(task_id)
            }
            Ok(Ok(ReplyResult::Failure(f))) => {
                debug!("Message failure: {:?}", f);
                Err(format!("Message failure: {:?}", f))
            }
            Ok(Err(e)) => {
                debug!("Error in message response: {:?}", e);
                Err(format!("Error in message response: {:?}", e))
            }
            Ok(Ok(m)) => {
                debug!("Unexpected response from message");
                Err(format!("Unexpected response from message: {:?}", m))
            }
            Err(e) => {
                debug!("Unable to receive message response: {:?}", e);
                Err(format!("Unable to receive message response: {:?}", e))
            }
        };

        deferred.settle_with(&channel, move |mut cx| match result {
            // TODO: create a "Task" object that we can then use to track the task
            Ok(task_id) => Ok(cx.number(task_id as f64)),
            Err(e) => cx.throw_error(e),
        });
    });

    Ok(promise)
}

pub fn connection_welcome_message(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let connection = cx.argument::<JsBox<ConnectionHandle>>(0)?;

    let runtime = crate::runtime(&mut cx)?;
    let channel = cx.channel();

    let (deferred, promise) = cx.promise();

    let (sender, client_token) = {
        let connection = connection.inner.lock().unwrap();

        (connection.sender.clone(), connection.client_token.clone())
    };
    runtime.spawn(async move {
        let (reply, receive) = tokio::sync::oneshot::channel();

        // welcome message is a login without any args.
        if let Err(e) = sender
            .send((
                reply,
                HostClientToDaemonMessage::RequestSysProp(
                    client_token.clone(),
                    ObjectRef::SysObj(vec![Symbol::mk("login")]),
                    Symbol::mk("welcome_message"),
                ),
            ))
            .await
        {
            error!("Unable to send welcome message request: {:?}", e);
            deferred.settle_with(&channel, move |mut cx| {
                cx.throw_error::<String, neon::handle::Handle<'_, JsPromise>>(format!(
                    "Unable to send welcome message request: {:?}",
                    e
                ))
            });
            return;
        }

        let result = match receive.await {
            Ok(Ok(ReplyResult::ClientSuccess(DaemonToClientReply::SysPropValue(Some(value))))) => {
                debug!("Welcome message: {:?}", value);
                Ok(value)
            }
            Ok(Ok(ReplyResult::ClientSuccess(DaemonToClientReply::SysPropValue(None)))) => {
                debug!("No welcome message");
                Ok(v_none())
            }
            Ok(Ok(ReplyResult::Failure(f))) => {
                debug!("Welcome message failure: {:?}", f);
                Err(format!("Welcome message failure: {:?}", f))
            }
            Ok(Err(e)) => {
                debug!("Error in welcome message response: {:?}", e);
                Err(format!("Error in welcome message response: {:?}", e))
            }
            Ok(Ok(m)) => {
                debug!("Unexpected response from welcome message");
                Err(format!("Unexpected response from welcome message: {:?}", m))
            }
            Err(e) => {
                debug!("Unable to receive welcome message response: {:?}", e);
                Err(format!(
                    "Unable to receive welcome message response: {:?}",
                    e
                ))
            }
        };

        deferred.settle_with(&channel, move |mut cx| match result {
            Ok(value) => {
                let value = var_to_js_value(&mut cx, &value)?;
                Ok(value)
            }
            Err(e) => cx.throw_error(e),
        });
    });

    Ok(promise)
}

pub fn connection_disconnect(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let connection = cx.argument::<JsBox<ConnectionHandle>>(0)?;

    let runtime = crate::runtime(&mut cx)?;
    let channel = cx.channel();

    let (deferred, promise) = cx.promise();

    let (sender, client_token) = {
        let connection = connection.inner.lock().unwrap();
        (connection.sender.clone(), connection.client_token.clone())
    };

    runtime.spawn(async move {
        let (reply, receive) = tokio::sync::oneshot::channel();
        if let Err(e) = sender
            .send((reply, HostClientToDaemonMessage::Detach(client_token)))
            .await
        {
            error!("Unable to send disconnect: {:?}", e);
        }

        let result = match receive.await {
            Ok(Ok(ReplyResult::ClientSuccess(DaemonToClientReply::Disconnected))) => {
                debug!("Disconnected");
                Ok(())
            }
            Ok(Ok(ReplyResult::Failure(f))) => {
                debug!("Disconnect failure: {:?}", f);
                Err(format!("Disconnect failure: {:?}", f))
            }
            Ok(Err(e)) => {
                debug!("Error in disconnect response: {:?}", e);
                Err(format!("Error in disconnect response: {:?}", e))
            }
            Ok(Ok(m)) => {
                debug!("Unexpected response from disconnect");
                Err(format!("Unexpected response from disconnect: {:?}", m))
            }
            Err(e) => {
                debug!("Unable to receive disconnect response: {:?}", e);
                Err(format!("Unable to receive disconnect response: {:?}", e))
            }
        };

        deferred.settle_with(&channel, move |mut cx| match result {
            Ok(_) => Ok(cx.undefined()),
            Err(e) => cx.throw_error(e),
        });
    });

    Ok(promise)
}

pub fn connection_get_oid(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let connection = cx.argument::<JsBox<ConnectionHandle>>(0)?;

    let runtime = crate::runtime(&mut cx)?;
    let channel = cx.channel();

    let (deferred, promise) = cx.promise();

    let connection = connection.inner.clone();
    runtime.spawn(async move {
        let connection = connection.lock().unwrap();
        let oid = connection.connection_oid.clone();
        deferred.settle_with(&channel, move |mut cx| {
            let oid = cx.number(oid.id().0 as f64);
            Ok(oid)
        });
    });

    Ok(promise)
}

pub fn connection_is_authenticated(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let connection = cx.argument::<JsBox<ConnectionHandle>>(0)?;

    let runtime = crate::runtime(&mut cx)?;
    let channel = cx.channel();

    let (deferred, promise) = cx.promise();

    let connection = connection.inner.clone();
    runtime.spawn(async move {
        let connection = connection.lock().unwrap();
        let is_authenticated = connection.auth_token.is_some();
        deferred.settle_with(&channel, move |mut cx| {
            let is_authenticated = cx.boolean(is_authenticated);
            Ok(is_authenticated)
        });
    });

    Ok(promise)
}
