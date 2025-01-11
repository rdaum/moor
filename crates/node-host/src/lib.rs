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

use host::Host;
use neon::prelude::*;
use once_cell::sync::OnceCell;
use tokio::runtime::Runtime;

mod connection;
mod host;

fn runtime<'a, C: Context<'a>>(cx: &mut C) -> NeonResult<&'static Runtime> {
    static RUNTIME: OnceCell<Runtime> = OnceCell::new();

    RUNTIME.get_or_try_init(|| Runtime::new().or_else(|err| cx.throw_error(err.to_string())))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    cx.export_function("createHost", Host::create_host)?;
    cx.export_function("attachToDaemon", host::attach_to_daemon)?;
    cx.export_function("listenHostEvents", host::listen_host_events)?;
    cx.export_function("shutdownHost", host::shutdown_host)?;
    cx.export_function("newConnection", connection::new_connection)?;
    cx.export_function("connectionLogin", connection::connection_login)?;
    cx.export_function("connectionSend", connection::connection_send)?;
    cx.export_function("connectionDisconnect", connection::connection_disconnect)?;

    Ok(())
}
