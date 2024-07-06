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

use eyre::Error;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use clap::Parser;
use clap_derive::Parser;
use color_eyre::owo_colors::OwoColorize;
use moor_values::model::Event::Notify;
use moor_values::var::{Objid, Variant};
use rustyline::config::Configurer;
use rustyline::error::ReadlineError;
use rustyline::{ColorMode, DefaultEditor, ExternalPrinter};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use rpc_common::{
    AuthToken, BroadcastEvent, ClientToken, ConnectionEvent, RpcRequest, RpcResponse, RpcResult,
    BROADCAST_TOPIC,
};
use rpc_sync_client::RpcSendClient;
use rpc_sync_client::{broadcast_recv, narrative_recv};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        value_name = "rpc-server",
        help = "RPC server address",
        default_value = "ipc:///tmp/moor_rpc.sock"
    )]
    rpc_server: String,

    #[arg(
        long,
        value_name = "narrative-server",
        help = "Narrative server address",
        default_value = "ipc:///tmp/moor_narrative.sock"
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

fn establish_connection(
    client_id: Uuid,
    rpc_client: &mut RpcSendClient,
) -> Result<(ClientToken, Objid), Error> {
    match rpc_client.make_rpc_call(
        client_id,
        RpcRequest::ConnectionEstablish("console".to_string()),
    ) {
        Ok(RpcResult::Success(RpcResponse::NewConnection(token, conn_id))) => Ok((token, conn_id)),
        Ok(RpcResult::Success(response)) => {
            error!(?response, "Unexpected response");
            Err(Error::msg("Unexpected response"))
        }
        Ok(RpcResult::Failure(error)) => {
            error!(?error, "Failure connecting");
            Err(Error::msg("Failure connecting"))
        }
        Err(error) => {
            error!(?error, "Error connecting");
            Err(Error::msg("Error connecting"))
        }
    }
}

fn perform_auth(
    token: ClientToken,
    client_id: Uuid,
    rpc_client: &mut RpcSendClient,
    username: &str,
    password: &str,
) -> Result<(AuthToken, Objid), Error> {
    // Need to first authenticate with the server.
    match rpc_client.make_rpc_call(
        client_id,
        RpcRequest::LoginCommand(
            token,
            vec![
                "connect".to_string(),
                username.to_string(),
                password.to_string(),
            ],
            true,
        ),
    ) {
        Ok(RpcResult::Success(RpcResponse::LoginResult(Some((
            auth_token,
            connect_type,
            player,
        ))))) => {
            info!(?connect_type, ?player, "Authenticated");
            Ok((auth_token, player))
        }
        Ok(RpcResult::Success(RpcResponse::LoginResult(None))) => {
            error!("Authentication failed");
            Err(Error::msg("Authentication failed"))
        }
        Ok(RpcResult::Success(response)) => {
            error!(?response, "Unexpected response");
            Err(Error::msg("Unexpected response"))
        }
        Ok(RpcResult::Failure(failure)) => {
            error!(?failure, "Failure authenticating");
            Err(Error::msg("Failure authenticating"))
        }
        Err(error) => {
            error!(?error, "Error authenticating");
            Err(Error::msg("Error authenticating"))
        }
    }
}

fn handle_console_line(
    client_token: ClientToken,
    auth_token: AuthToken,
    client_id: Uuid,
    line: &str,
    rpc_client: &mut RpcSendClient,
    input_request_id: Option<Uuid>,
) {
    let line = line.trim();
    if let Some(input_request_id) = input_request_id {
        match rpc_client.make_rpc_call(
            client_id,
            RpcRequest::RequestedInput(
                client_token.clone(),
                auth_token.clone(),
                input_request_id.as_u128(),
                line.to_string(),
            ),
        ) {
            Ok(RpcResult::Success(RpcResponse::InputThanks)) => {
                trace!("Input complete");
            }
            Ok(RpcResult::Success(response)) => {
                warn!(?response, "Unexpected input response");
            }
            Ok(RpcResult::Failure(error)) => {
                error!(?error, "Failure executing input");
            }
            Err(error) => {
                error!(?error, "Error executing input");
            }
        }
        return;
    }

    match rpc_client.make_rpc_call(
        client_id,
        RpcRequest::Command(client_token.clone(), auth_token.clone(), line.to_string()),
    ) {
        Ok(RpcResult::Success(RpcResponse::CommandSubmitted(_))) => {
            trace!("Command complete");
        }
        Ok(RpcResult::Success(response)) => {
            warn!(?response, "Unexpected command response");
        }
        Ok(RpcResult::Failure(error)) => {
            error!(?error, "Failure executing command");
        }
        Err(error) => {
            error!(?error, "Error executing command");
        }
    }
}

fn console_loop(
    rpc_server: &str,
    narrative_server: &str,
    username: &str,
    password: &str,
) -> Result<(), Error> {
    let zmq_ctx = zmq::Context::new();

    let rpc_socket = zmq_ctx.socket(zmq::REQ)?;
    rpc_socket.connect(rpc_server)?;

    // Establish a connection to the RPC server
    let client_id = Uuid::new_v4();

    let mut rpc_client = RpcSendClient::new(rpc_socket);

    let (client_token, conn_obj_id) = establish_connection(client_id, &mut rpc_client)?;
    debug!("Transitional connection ID before auth: {:?}", conn_obj_id);

    // Now authenticate with the server.
    let (auth_token, player) = perform_auth(
        client_token.clone(),
        client_id,
        &mut rpc_client,
        username,
        password,
    )?;

    println!(
        "Authenticated as {:?} ({})",
        username.yellow(),
        player.yellow()
    );

    // Spawn a thread to listen for events on the narrative pubsub channel, and send them to the
    // console.
    let narr_sub_socket = zmq_ctx.socket(zmq::SUB)?;
    narr_sub_socket.connect(narrative_server)?;
    narr_sub_socket.set_subscribe(client_id.as_bytes())?;
    let input_request_id = Arc::new(Mutex::new(None));
    let output_input_request_id = input_request_id.clone();

    let mut rl = DefaultEditor::new().unwrap();
    let mut printer = rl.create_external_printer().unwrap();

    std::thread::Builder::new()
        .name("output-loop".to_string())
        .spawn(move || loop {
            match narrative_recv(client_id, &narr_sub_socket) {
                Ok(ConnectionEvent::Narrative(_, msg)) => {
                    let Notify(msg, _content_type) = msg.event();
                    // TODO: Handle non text/plain content type
                    let msg_text = match msg.variant() {
                        Variant::Str(s) => s.as_str().to_string(),
                        _ => msg.to_literal(),
                    };
                    printer.print(msg_text).unwrap();
                }
                Ok(ConnectionEvent::SystemMessage(o, msg)) => {
                    printer
                        .print(format!("System message from {}: {}", o.yellow(), msg.red()))
                        .unwrap();
                }
                Ok(ConnectionEvent::Disconnect()) => {
                    printer
                        .print("Received disconnect event; Session ending.".to_string())
                        .unwrap();
                    return;
                }
                Err(error) => {
                    printer
                        .print(format!(
                            "Error receiving narrative event {:?}; Session ending.",
                            error
                        ))
                        .unwrap();
                    return;
                }
                Ok(ConnectionEvent::RequestInput(requested_input_id)) => {
                    (*output_input_request_id.lock().unwrap()) =
                        Some(Uuid::from_u128(requested_input_id));
                }
            }
        })?;

    let mut broadcast_subscriber = zmq_ctx.socket(zmq::SUB)?;
    broadcast_subscriber.connect(narrative_server)?;
    broadcast_subscriber.set_subscribe(BROADCAST_TOPIC)?;

    let broadcast_rpc_socket = zmq_ctx.socket(zmq::REQ)?;
    broadcast_rpc_socket.connect(rpc_server)?;
    let mut broadcast_rpc_client = RpcSendClient::new(broadcast_rpc_socket);

    let broadcast_client_token = client_token.clone();
    std::thread::spawn(move || loop {
        match broadcast_recv(&mut broadcast_subscriber) {
            Ok(BroadcastEvent::PingPong(_)) => {
                if let Err(e) = broadcast_rpc_client.make_rpc_call(
                    client_id,
                    RpcRequest::Pong(broadcast_client_token.clone(), SystemTime::now()),
                ) {
                    error!("Error sending pong: {:?}", e);
                    return;
                }
            }
            Err(e) => {
                error!("Error receiving broadcast event: {:?}; Session ending.", e);
                return;
            }
        }
    });

    let edit_client_token = client_token.clone();
    let edit_auth_token = auth_token.clone();

    rl.set_color_mode(ColorMode::Enabled);

    loop {
        // TODO: unprovoked output from the narrative stream screws up the prompt midstream,
        //   but we have no real way to signal to this loop that it should newline for
        //   cleanliness. Need to figure out something for this.
        let input_request_id = input_request_id.lock().unwrap().take();
        let prompt = if let Some(input_request_id) = input_request_id {
            format!("{} > ", input_request_id)
        } else {
            "> ".to_string()
        };
        let output = rl.readline(prompt.as_str());
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
                );
            }
            Err(ReadlineError::Eof) => {
                eprintln!("{}", "<EOF>".red());
                break;
            }
            Err(ReadlineError::Interrupted) => {
                eprintln!("{}", "^C".red());
                continue;
            }
            Err(e) => {
                eprintln!("{}: {}", "Error".red(), e.red());
                break;
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    color_eyre::install()?;

    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(false)
        .with_line_number(false)
        .with_thread_names(false)
        .without_time()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    console_loop(
        &args.rpc_server,
        args.narrative_server.as_str(),
        &args.username,
        &args.password,
    )
}
