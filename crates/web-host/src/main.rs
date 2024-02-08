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

mod client;
mod host;

use crate::client::{editor_handler, js_handler, root_handler};
use crate::host::WebHost;

use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use clap_derive::Parser;

use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        value_name = "listen-address",
        help = "HTTP listen address",
        default_value = "0.0.0.0:8888"
    )]
    listen_address: String,

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
}

fn mk_routes(web_host: WebHost) -> eyre::Result<Router> {
    let webhost_router = Router::new()
        .route(
            "/ws/attach/connect/:token",
            get(host::ws_connect_attach_handler),
        )
        .route("/", get(root_handler))
        .route("/browser.html", get(client::browser_handler))
        .route("/moor.js", get(js_handler))
        .route("/editor.js", get(editor_handler))
        .route(
            "/ws/attach/create/:token",
            get(host::ws_create_attach_handler),
        )
        .route("/auth/connect", post(host::connect_auth_handler))
        .route("/auth/create", post(host::create_auth_handler))
        .route("/welcome", get(host::welcome_message_handler))
        .route("/eval", post(host::eval_handler))
        .with_state(web_host);

    Ok(Router::new().nest("/", webhost_router))
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
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

    let ws_host = WebHost::new(args.rpc_server, args.narrative_server);

    let main_router = mk_routes(ws_host).expect("Unable to create main router");

    let address = &args.listen_address.parse::<SocketAddr>().unwrap();
    info!(address=?address, "Listening");

    let listener = TcpListener::bind(address)
        .await
        .expect("Unable to bind HTTP listener");

    axum::serve(
        listener,
        main_router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();

    Ok(())
}
