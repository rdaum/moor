// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Check client linkage and an administrative connection without opening world storage.
use moor_db::{PostgresConnectOptions, PostgresConnection, PostgresEndpoint, PostgresShutdown};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = std::env::var("MOOR_PG_CONNINFO")?;
    let address = std::env::var("MOOR_PG_ADDRESS")?.parse()?;
    let options = PostgresConnectOptions::new(connection, PostgresEndpoint::Tcp(address));
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut client = PostgresConnection::connect(&options, deadline, PostgresShutdown::default())?;
    client.query(
        "SELECT current_setting('server_version'), current_setting('server_encoding')",
        &[],
        deadline,
        |row| {
            println!(
                "server={} encoding={}",
                String::from_utf8_lossy(row.columns[0].as_deref().unwrap()),
                String::from_utf8_lossy(row.columns[1].as_deref().unwrap())
            );
            Ok(())
        },
    )?;
    Ok(())
}
