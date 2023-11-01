use anyhow::Error;
use std::process::exit;
use std::sync::Arc;
use std::time::SystemTime;

use clap::Parser;
use clap_derive::Parser;
use moor_values::var::objid::Objid;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use tmq::request;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;
use tokio::task::block_in_place;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use rpc_common::pubsub_client::{broadcast_recv, narrative_recv};
use rpc_common::rpc_client::RpcSendClient;
use rpc_common::{
    AuthToken, BroadcastEvent, ClientToken, ConnectionEvent, RpcRequest, RpcResponse, RpcResult,
    BROADCAST_TOPIC,
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        value_name = "rpc-server",
        help = "RPC server address",
        default_value = "tcp://0.0.0.0:7899"
    )]
    rpc_server: String,

    #[arg(
        long,
        value_name = "narrative-server",
        help = "Narrative server address",
        default_value = "tcp://0.0.0.0:7898"
    )]
    narrative_server: String,

    #[arg(
        long,
        value_name = "username",
        help = "Username to use for authentication",
        default_value = "Wizard"
    )]
    username: String,

    #[arg(
        long,
        value_name = "password",
        help = "Password to use for authentication",
        default_value = ""
    )]
    password: String,
}

async fn establish_connection(
    client_id: Uuid,
    rpc_client: &mut RpcSendClient,
) -> Result<(ClientToken, Objid), anyhow::Error> {
    match rpc_client
        .make_rpc_call(
            client_id,
            RpcRequest::ConnectionEstablish("console".to_string()),
        )
        .await
    {
        Ok(RpcResult::Success(RpcResponse::NewConnection(token, conn_id))) => Ok((token, conn_id)),
        Ok(RpcResult::Success(other)) => {
            error!("Unexpected response: {:?}", other);
            Err(Error::msg("Unexpected response"))
        }
        Ok(RpcResult::Failure(e)) => {
            error!("Failure connecting: {:?}", e);
            Err(Error::msg("Failure connecting"))
        }
        Err(e) => {
            error!("Error connecting: {:?}", e);
            Err(Error::msg("Error connecting"))
        }
    }
}

async fn perform_auth(
    token: ClientToken,
    client_id: Uuid,
    rpc_client: &mut RpcSendClient,
    username: &str,
    password: &str,
) -> Result<(AuthToken, Objid), Error> {
    // Need to first authenticate with the server.
    match rpc_client
        .make_rpc_call(
            client_id,
            RpcRequest::LoginCommand(
                token,
                vec![
                    "connect".to_string(),
                    username.to_string(),
                    password.to_string(),
                ],
            ),
        )
        .await
    {
        Ok(RpcResult::Success(RpcResponse::LoginResult(Some((
            auth_token,
            connect_type,
            player,
        ))))) => {
            info!("Authenticated as {:?} with id {:?}", connect_type, player);
            Ok((auth_token, player))
        }
        Ok(RpcResult::Success(RpcResponse::LoginResult(None))) => {
            error!("Authentication failed");
            Err(Error::msg("Authentication failed"))
        }
        Ok(RpcResult::Success(other)) => {
            error!("Unexpected response: {:?}", other);
            Err(Error::msg("Unexpected response"))
        }
        Ok(RpcResult::Failure(e)) => {
            error!("Failure authenticating: {:?}", e);
            Err(Error::msg("Failure authenticating"))
        }
        Err(e) => {
            error!("Error authenticating: {:?}", e);
            Err(Error::msg("Error authenticating"))
        }
    }
}

async fn handle_console_line(
    client_token: ClientToken,
    auth_token: AuthToken,
    client_id: Uuid,
    line: &str,
    rpc_client: &mut RpcSendClient,
    input_request_id: Option<Uuid>,
) {
    // Lines are either 'eval' or 'command', depending on the mode we're in.
    // TODO: The intent here is to do something like Julia's repl interface where they have a pkg
    //  mode (initiated by initial ] keystroke) and default repl mode.
    //  For us, our initial keystroke will provoke evaluation through `Eval` but default will be
    //  to send standard MOO commands.
    //  But For now, we'll just act as if we're a telnet connection. User can do eval with ; via
    //  the core.
    let line = line.trim();
    if let Some(input_request_id) = input_request_id {
        match rpc_client
            .make_rpc_call(
                client_id,
                RpcRequest::RequestedInput(
                    client_token.clone(),
                    auth_token.clone(),
                    input_request_id.as_u128(),
                    line.to_string(),
                ),
            )
            .await
        {
            Ok(RpcResult::Success(RpcResponse::InputThanks)) => {
                trace!("Input complete");
            }
            Ok(RpcResult::Success(other)) => {
                warn!("Unexpected input response: {:?}", other);
            }
            Ok(RpcResult::Failure(e)) => {
                error!("Failure executing input: {:?}", e);
            }
            Err(e) => {
                error!("Error executing input: {:?}", e);
            }
        }
        return;
    }

    match rpc_client
        .make_rpc_call(
            client_id,
            RpcRequest::Command(client_token.clone(), auth_token.clone(), line.to_string()),
        )
        .await
    {
        Ok(RpcResult::Success(RpcResponse::CommandSubmitted(_))) => {
            trace!("Command complete");
        }
        Ok(RpcResult::Success(other)) => {
            warn!("Unexpected command response: {:?}", other);
        }
        Ok(RpcResult::Failure(e)) => {
            error!("Failure executing command: {:?}", e);
        }
        Err(e) => {
            error!("Error executing command: {:?}", e);
        }
    }
}

async fn console_loop(
    rpc_server: &str,
    narrative_server: &str,
    username: &str,
    password: &str,
) -> Result<(), anyhow::Error> {
    let zmq_ctx = tmq::Context::new();

    // Establish a connection to the RPC server
    let client_id = Uuid::new_v4();

    let rcp_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(rpc_server)
        .expect("Unable to bind RPC server for connection");

    let mut rpc_client = RpcSendClient::new(rcp_request_sock);

    let (client_token, conn_obj_id) = establish_connection(client_id, &mut rpc_client).await?;
    debug!("Transitional connection ID before auth: {:?}", conn_obj_id);

    // Now authenticate with the server.
    let (auth_token, player) = perform_auth(
        client_token.clone(),
        client_id,
        &mut rpc_client,
        username,
        password,
    )
    .await?;

    info!("Authenticated as {:?} /  {}", username, player);

    // Spawn a thread to listen for events on the narrative pubsub channel, and send them to the
    // console.
    let narrative_subscriber = tmq::subscribe(&zmq_ctx)
        .connect(narrative_server)
        .expect("Unable to connect to narrative pubsub server");
    let mut narrative_subscriber = narrative_subscriber
        .subscribe(client_id.as_bytes())
        .expect("Unable to subscribe to narrative pubsub server");
    let input_request_id = Arc::new(Mutex::new(None));
    let output_input_request_id = input_request_id.clone();
    let output_loop = tokio::spawn(async move {
        loop {
            match narrative_recv(client_id, &mut narrative_subscriber).await {
                Ok(ConnectionEvent::Narrative(_, msg)) => {
                    println!("{}", msg.event());
                }
                Ok(ConnectionEvent::SystemMessage(o, msg)) => {
                    eprintln!("SYSMSG: {}: {}", o, msg);
                }
                Ok(ConnectionEvent::Disconnect()) => {
                    error!("Received disconnect event; Session ending.");
                    return;
                }
                Err(e) => {
                    error!("Error receiving narrative event: {:?}; Session ending.", e);
                    return;
                }
                Ok(ConnectionEvent::RequestInput(requested_input_id)) => {
                    (*output_input_request_id.lock().await) =
                        Some(Uuid::from_u128(requested_input_id));
                }
            }
        }
    });

    let broadcast_subscriber = tmq::subscribe(&zmq_ctx)
        .connect(narrative_server)
        .expect("Unable to connect to narrative pubsub server");
    let mut broadcast_subscriber = broadcast_subscriber
        .subscribe(BROADCAST_TOPIC)
        .expect("Unable to subscribe to narrative pubsub server");
    let broadcast_rcp_request_sock = request(&zmq_ctx)
        .set_rcvtimeo(100)
        .set_sndtimeo(100)
        .connect(rpc_server)
        .expect("Unable to bind RPC server for connection");
    let mut broadcast_rpc_client = RpcSendClient::new(broadcast_rcp_request_sock);

    let broadcast_client_token = client_token.clone();
    let broadcast_loop = tokio::spawn(async move {
        loop {
            match broadcast_recv(&mut broadcast_subscriber).await {
                Ok(BroadcastEvent::PingPong(_)) => {
                    if let Err(e) = broadcast_rpc_client
                        .make_rpc_call(
                            client_id,
                            RpcRequest::Pong(broadcast_client_token.clone(), SystemTime::now()),
                        )
                        .await
                    {
                        error!("Error sending pong: {:?}", e);
                        return;
                    }
                }
                Err(e) => {
                    error!("Error receiving broadcast event: {:?}; Session ending.", e);
                    return;
                }
            }
        }
    });

    let edit_client_token = client_token.clone();
    let edit_auth_token = auth_token.clone();
    let edit_loop = tokio::spawn(async move {
        let mut rl = DefaultEditor::new().unwrap();
        loop {
            // TODO: unprovoked output from the narrative stream screws up the prompt midstream,
            //   but we have no real way to signal to this loop that it should newline for
            //   cleanliness. Need to figure out something for this.
            let input_request_id = input_request_id.lock().await.take();
            let prompt = if let Some(input_request_id) = input_request_id {
                format!("{} > ", input_request_id)
            } else {
                "> ".to_string()
            };
            let output = block_in_place(|| rl.readline(prompt.as_str()));
            match output {
                Ok(line) => {
                    rl.add_history_entry(line.clone())
                        .expect("Could not add history");
                    handle_console_line(
                        edit_client_token.clone(),
                        edit_auth_token.clone(),
                        client_id,
                        &line,
                        &mut rpc_client,
                        input_request_id,
                    )
                    .await;
                }
                Err(ReadlineError::Eof) => {
                    println!("<EOF>");
                    break;
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    continue;
                }
                Err(e) => {
                    println!("Error: {e:?}");
                    break;
                }
            }
        }
    });

    select! {
        _ = output_loop => {
            info!("ZMQ client loop exited, stopping...");
        }
        _ = broadcast_loop => {
            info!("Broadcast loop exited, stopping...");
        }
        _ = edit_loop => {
            info!("Edit loop exited, stopping...");
        }
    }
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), anyhow::Error> {
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    let mut hup_signal =
        signal(SignalKind::hangup()).expect("Unable to register HUP signal handler");
    let mut stop_signal =
        signal(SignalKind::interrupt()).expect("Unable to register STOP signal handler");

    select! {
        _ = console_loop(&args.rpc_server, args.narrative_server.as_str(),
                      &args.username, &args.password) => {
            info!("console session exited, quitting...");
            exit(0);
        }
        _ = hup_signal.recv() => {
            info!("HUP received, quitting...");
            exit(0);
        },
        _ = stop_signal.recv() => {
            info!("STOP received, quitting...");
            exit(0);
        }
    }
}
