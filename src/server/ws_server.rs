use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, WebSocketStream};
use tungstenite::{Error, Message};

use crate::model::var::Objid;
use crate::server::scheduler::Scheduler;
use crate::server::ClientConnection;

pub struct WebSocketClientConnection {
    ws_sink: SplitSink<WebSocketStream<TcpStream>, Message>,
}

#[async_trait]
impl ClientConnection for WebSocketClientConnection {
    async fn send_text(&mut self, msg: String) -> Result<(), anyhow::Error> {
        self.ws_sink.send(msg.into()).await?;
        Ok(())
    }
}

async fn ws_accept_connection(
    scheduler: Arc<Mutex<Scheduler>>,
    peer: SocketAddr,
    stream: TcpStream,
) {
    if let Err(e) = ws_handle_connection(scheduler.clone(), peer, stream).await {
        if let Some(e) = e.downcast_ref::<tungstenite::Error>() {
            match e {
                Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
                err => eprintln!("Error processing connection: {}", err),
            }
        }
    }
}

async fn ws_handle_connection(
    scheduler: Arc<Mutex<Scheduler>>,
    peer: SocketAddr,
    stream: TcpStream,
) -> Result<(), anyhow::Error> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");

    eprintln!("New WebSocket connection: {}", peer);
    let (ws_sender, mut ws_receiver) = ws_stream.split();

    let client_connection = WebSocketClientConnection { ws_sink: ws_sender };
    let client_connection = Arc::new(Mutex::new(client_connection));
    while let Some(msg) = ws_receiver.next().await {
        let msg = msg?;
        if msg.is_text() || msg.is_binary() {
            let mut scheduler = scheduler.lock().await;
            let cmd = msg.into_text().unwrap();
            let cmd = cmd.as_str().trim();

            let setup_result = scheduler
                .setup_parse_command_task(Objid(2), cmd, client_connection.clone())
                .await;
            let Ok(task_id) = setup_result else {
                eprintln!("Unable to parse command ({}): {:?}", cmd, setup_result);
                let mut client_connection = client_connection.lock().await;
                client_connection.send_text(format!("Unable to parse command ({}): {:?}", cmd, setup_result)).await?;

                continue;
            };
            eprintln!("Task: {:?}", task_id);

            if let Err(e) = scheduler.start_task(task_id).await {
                eprintln!("Unable to execute: {}", e);
                let mut client_connection = client_connection.lock().await;

                client_connection
                    .send_text(format!("Unable to execute: {}", e))
                    .await?;

                continue;
            };
            client_connection
                .lock()
                .await
                .send_text(format!(
                    "Command parsed correctly and ran in task {:?}",
                    task_id
                ))
                .await?;
        }
    }

    Ok(())
}

pub async fn ws_server_start(
    scheduler: Arc<Mutex<Scheduler>>,
    addr: String,
) -> Result<(), anyhow::Error> {
    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    eprintln!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");

        tokio::spawn(ws_accept_connection(scheduler.clone(), peer, stream));
    }

    Ok(())
}
