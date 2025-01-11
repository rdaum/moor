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

use moor_values::Obj;
use neon::context::{Context, FunctionContext};
use neon::handle::{Handle, Root};
use neon::object::Object;
use neon::prelude::{
    Finalize, JsArray, JsBox, JsFunction, JsNumber, JsObject, JsPromise, JsResult, JsString,
    JsUndefined, JsValue, NeonResult,
};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_async_client::{
    make_host_token, proces_hosts_events, start_host_session, ListenersClient, ListenersMessage,
};
use rpc_common::{parse_keypair, HostToken, HostType};
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tracing::info;

/// Wraps up information about this "Host" which is a connection to the daemon, and a set of
/// listeners.
pub struct Host {
    pub(crate) inner: Arc<Mutex<Inner>>,
}

pub(crate) struct Inner {
    pub(crate) host_token: HostToken,
    pub(crate) zmq_ctx: tmq::Context,
    pub(crate) kill_switch: Arc<AtomicBool>,
    pub(crate) listeners_client: Option<ListenersClient>,
    pub(crate) rpc_client: Option<RpcSendClient>,
}

/// Setup the listeners client, which will be used to manage listeners on the host.
pub fn make_listeners_client<'a, C: Context<'a>>(
    cx: &mut C,
    host: Root<JsBox<Host>>,
    mut get_listeners_callback: Root<JsFunction>,
    mut add_listener_callback: Root<JsFunction>,
    mut remove_listener_callback: Root<JsFunction>,
) -> NeonResult<ListenersClient> {
    let host_box = host.to_inner(cx);
    let host = host_box.inner.clone();

    let runtime = crate::runtime(cx)?;
    let channel = cx.channel();
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    runtime.spawn(async move {
        loop {
            {
                let host = host.lock().unwrap();
                if host.kill_switch.load(std::sync::atomic::Ordering::SeqCst) {
                    info!("Kill switch activated, stopping...");
                    break;
                }
            }
            // TODO: shutdown on kill switch
            match rx.recv().await {
                None => {
                    info!("Listeners client channel closed");
                    break;
                }
                Some(ListenersMessage::GetListeners(reply)) => {
                    // Call the get_listeners_callback, and it should return a list of listeners
                    // as JS objects which we will then have to turn into Rust objects (Obj, SocketAddr)
                    let continuation = channel
                        .send(move |mut cx| {
                            // Our get listeners callback should be defined as a value on `this`

                            let callback = get_listeners_callback.clone(&mut cx);
                            let callback = callback.into_inner(&mut cx);

                            let undefined = cx.undefined();
                            let Ok(listeners) = callback.call(&mut cx, undefined, vec![]) else {
                                return cx.throw_error("Unable to get listeners");
                            };
                            let Ok(listeners) = listeners.downcast::<JsArray, _>(&mut cx) else {
                                return cx.throw_error("Listeners is not an array");
                            };
                            let Ok(listeners) = listeners.to_vec(&mut cx) else {
                                return cx.throw_error("Unable to convert listeners to vec");
                            };

                            // Convert the JS objects into Rust objects
                            // Array of tuples:
                            // First argument is an integer which we will turn into a literal Obj
                            // Second argument is a string to be parsed as a SocketAddr
                            let mut listeners_result = vec![];
                            for listener in listeners {
                                // (integer, string)
                                let Ok(listener) = listener.downcast::<JsObject, _>(&mut cx) else {
                                    return cx.throw_error("Listener is not an object");
                                };
                                let obj: Handle<JsNumber> = listener.get(&mut cx, "obj")?;
                                let addr: Handle<JsString> = listener.get(&mut cx, "addr")?;
                                let obj = obj.value(&mut cx) as i32;
                                let obj = Obj::mk_id(obj);

                                let addr = addr.value(&mut cx);
                                let Ok(addr) = addr.parse::<SocketAddr>() else {
                                    return cx.throw_error("Unable to parse address");
                                };
                                listeners_result.push((obj, addr));
                            }

                            reply.send(listeners_result).unwrap();

                            Ok(get_listeners_callback)
                        })
                        .join();
                    get_listeners_callback = match continuation {
                        Ok(continuation) => continuation,
                        Err(e) => {
                            info!("Unable to schedule continuation: {}", e);
                            break;
                        }
                    };
                }
                Some(ListenersMessage::AddListener(obj, addr)) => {
                    let continuation = channel
                        .send(move |mut cx| {
                            let callback = add_listener_callback.clone(&mut cx);
                            let callback = callback.into_inner(&mut cx);

                            let listener_entry = cx.empty_object();
                            let obj_id = obj.id().0;
                            let obj_id = cx.number(obj_id as f64);
                            listener_entry.set(&mut cx, "obj", obj_id)?;
                            let addr = cx.string(addr.to_string());
                            listener_entry.set(&mut cx, "addr", addr)?;

                            let listener_entry: Handle<JsValue> = listener_entry.upcast();

                            let undefined = cx.undefined();
                            let Ok(_) = callback.call(&mut cx, undefined, vec![listener_entry])
                            else {
                                return cx.throw_error("Unable to add listener");
                            };
                            Ok(add_listener_callback)
                        })
                        .join();
                    add_listener_callback = match continuation {
                        Ok(continuation) => continuation,
                        Err(e) => {
                            info!("Unable to schedule continuation: {}", e);
                            break;
                        }
                    };
                }
                Some(ListenersMessage::RemoveListener(sockaddr)) => {
                    let continuation = channel
                        .send(move |mut cx| {
                            let callback = remove_listener_callback.clone(&mut cx);
                            let callback = callback.into_inner(&mut cx);
                            let addr = cx.string(sockaddr.to_string());
                            let addr: Handle<JsValue> = addr.upcast();

                            let undefined = cx.undefined();
                            let Ok(_) = callback.call(&mut cx, undefined, vec![addr]) else {
                                return cx.throw_error("Unable to remove listener");
                            };
                            Ok(remove_listener_callback)
                        })
                        .join();
                    remove_listener_callback = match continuation {
                        Ok(continuation) => continuation,
                        Err(e) => {
                            info!("Unable to schedule continuation: {}", e);
                            break;
                        }
                    };
                }
            }
        }
    });
    Ok(ListenersClient::new(tx))
}

impl Finalize for Host {
    fn finalize<'a, C: Context<'a>>(self, _: &mut C) {
        // Shut down.
        let inner = self.inner.lock().unwrap();
        inner
            .kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);
        info!("Host dropped");
    }
}

/// Attach this Host to the daemon.
pub fn attach_to_daemon(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let host_box_js = cx.argument::<JsBox<Host>>(0)?;
    let host_box = host_box_js.inner.clone();
    let rpc_address = cx.argument::<JsString>(1)?.value(&mut cx);

    let rt = crate::runtime(&mut cx)?;
    let (deferred, promise) = cx.promise();

    let channel = cx.channel();

    let get_listeners_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    let add_listener_callback = cx.argument::<JsFunction>(3)?.root(&mut cx);
    let remove_listener_callback = cx.argument::<JsFunction>(4)?.root(&mut cx);

    let host_box_js = host_box_js.root(&mut cx);
    let listeners_client = make_listeners_client(
        &mut cx,
        host_box_js,
        get_listeners_callback,
        add_listener_callback,
        remove_listener_callback,
    )?;

    {
        let mut host_box = host_box.lock().unwrap();
        host_box.listeners_client = Some(listeners_client.clone());
    }
    rt.spawn(async move {
        // TODO: listeners

        let (host_token, zmq_ctx, kill_switch, listeners_client) = {
            let host_box = host_box.lock().unwrap();
            (
                host_box.host_token.clone(),
                host_box.zmq_ctx.clone(),
                host_box.kill_switch.clone(),
                listeners_client,
            )
        };

        let success = match start_host_session(
            &host_token,
            zmq_ctx,
            rpc_address,
            kill_switch,
            listeners_client,
        )
        .await
        {
            Ok(rpc_client) => {
                info!("Host session established");
                let mut host_box = host_box.lock().unwrap();

                host_box.rpc_client = Some(rpc_client);
                Ok(())
            }
            Err(e) => Err(format!("Unable to establish initial host session: {}", e)),
        };

        deferred.settle_with(&channel, move |mut cx| match success {
            Ok(_) => Ok(cx.undefined()),
            Err(e) => cx.throw_error(e),
        });
    });

    Ok(promise)
}

/// Start listening for events from the host.
pub fn listen_host_events(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let host_box = cx.argument::<JsBox<Host>>(0)?;
    let host_box = host_box.inner.clone();
    let events_address = cx.argument::<JsString>(1)?.value(&mut cx);
    let listen_address = cx.argument::<JsString>(2)?.value(&mut cx);
    let rt = crate::runtime(&mut cx)?;

    let (rpc_send_client, host_token, zmq_context, kill_switch, listeners_client) = {
        let mut host_box = host_box.lock().unwrap();
        let Some(rpc_send_client) = host_box.rpc_client.take() else {
            return cx.throw_error("Host not attached to daemon");
        };
        let Some(listeners_client) = host_box.listeners_client.clone() else {
            return cx.throw_error("Listeners client not initialized");
        };
        (
            rpc_send_client,
            host_box.host_token.clone(),
            host_box.zmq_ctx.clone(),
            host_box.kill_switch.clone(),
            listeners_client,
        )
    };
    rt.spawn(proces_hosts_events(
        rpc_send_client,
        host_token,
        zmq_context,
        events_address,
        listen_address,
        kill_switch,
        listeners_client,
        // TODO: Add NodeJS type
        HostType::WebSocket,
    ));

    Ok(cx.undefined())
}

/// Shutdown the host.
pub fn shutdown_host(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let host_box = cx.argument::<JsBox<Host>>(0)?;
    let host_box = host_box.inner.clone();
    let rt = crate::runtime(&mut cx)?;

    rt.spawn(async move {
        let host_box = host_box.lock().unwrap();
        host_box
            .kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);
    });

    Ok(cx.undefined())
}

impl Host {
    pub(crate) fn create_host(mut cx: FunctionContext) -> JsResult<JsBox<Host>> {
        // Public / private key pair for speaking to host should be in an the arguments to this
        // function as full PEM strings.
        let args = cx.argument::<JsObject>(0)?;

        let public_key: Handle<JsString> = args.get(&mut cx, "public_key")?;
        let private_key: Handle<JsString> = args.get(&mut cx, "private_key")?;

        let (privkey, _publickey) =
            match parse_keypair(&public_key.value(&mut cx), &private_key.value(&mut cx)) {
                Ok((prv, publ)) => (prv, publ),
                Err(e) => {
                    return cx.throw_error(e.to_string());
                }
            };

        let host_token = make_host_token(&privkey, HostType::TCP);

        let zmq_ctx = tmq::Context::new();

        let kill_switch = Arc::new(AtomicBool::new(false));
        let session = Host {
            inner: Arc::new(Mutex::new(Inner {
                host_token,
                zmq_ctx,
                kill_switch: kill_switch.clone(),
                listeners_client: None,
                rpc_client: None,
            })),
        };

        Ok(cx.boxed(session))
    }
}
