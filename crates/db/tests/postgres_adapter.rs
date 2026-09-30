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

//! Live adapter contract tests; scripts/test-postgres-adapter.sh supplies the server fixture.
#![cfg(feature = "postgres")]

use moor_db::{
    PostgresConnectOptions, PostgresConnection, PostgresEndpoint, PostgresError, PostgresParam,
    PostgresShutdown,
};
use std::{
    net::TcpListener,
    time::{Duration, Instant},
};

fn options() -> PostgresConnectOptions {
    PostgresConnectOptions::new(
        std::env::var("MOOR_PG_TEST_CONNINFO").expect("set MOOR_PG_TEST_CONNINFO"),
        PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
    )
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}
fn connect() -> PostgresConnection {
    PostgresConnection::connect(&options(), deadline(), PostgresShutdown::default()).unwrap()
}

#[test]
#[ignore = "requires PostgreSQL 17/18 fixture and libpq 17+"]
fn parameters_preparation_and_streaming() {
    let mut conn = connect();
    let text = "quote '; SELECT 999; -- 🐄";
    let params = [
        PostgresParam::Text(0, text),
        PostgresParam::Null(0),
        PostgresParam::Text(0, ""),
        PostgresParam::Binary(17, &[0, 255, 42]),
    ];
    conn.prepare(
        "values",
        "SELECT $1::text, $2::text, $3::text, encode($4::bytea, 'hex')",
        &[25, 25, 25, 17],
        deadline(),
    )
    .unwrap();
    let mut seen = 0;
    let result = conn
        .execute_prepared("values", &params, deadline(), |row| {
            seen += 1;
            assert_eq!(
                row.columns,
                vec![
                    Some(text.as_bytes().to_vec()),
                    None,
                    Some(vec![]),
                    Some(b"00ff2a".to_vec())
                ]
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, 1);
    assert_eq!(result.rows, 1);
    let mut count = 0;
    let result = conn
        .query(
            "SELECT n FROM pg_catalog.generate_series(1,10000) AS n",
            &[],
            deadline(),
            |row| {
                count += 1;
                assert_eq!(
                    row.columns[0].as_deref().unwrap(),
                    count.to_string().as_bytes()
                );
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(result.rows, 10000);
    conn.query("SELECT current_setting('search_path'), current_setting('client_encoding'), current_setting('application_name')", &[], deadline(), |row| {
        assert_eq!(row.columns, vec![Some(b"pg_catalog".to_vec()), Some(b"UTF8".to_vec()), Some(b"moor-persistence".to_vec())]); Ok(())
    }).unwrap();
    conn.query(
        "CREATE TEMP TABLE adapter_values (value text)",
        &[],
        deadline(),
        |_| unreachable!(),
    )
    .unwrap();
    let result = conn
        .query(
            "INSERT INTO pg_temp.adapter_values VALUES ($1)",
            &[PostgresParam::Text(25, text)],
            deadline(),
            |_| unreachable!(),
        )
        .unwrap();
    assert_eq!(result.affected_rows, Some(1));
}

#[test]
#[ignore = "requires PostgreSQL 17/18 fixture and libpq 17+"]
fn errors_limits_and_callback_abort_close_connection() {
    let mut conn = connect();
    let error = conn
        .query(
            "SELECT 'private-data', no_such_private_column",
            &[],
            deadline(),
            |_| Ok(()),
        )
        .unwrap_err();
    assert_eq!(error, PostgresError::SqlState("42703".into()));
    assert!(!error.to_string().contains("private"));
    assert!(conn.is_closed());
    assert_eq!(
        conn.query("SELECT 1", &[], deadline(), |_| Ok(())),
        Err(PostgresError::Closed)
    );
    let mut config = options();
    config.max_row_bytes = 8;
    let mut conn =
        PostgresConnection::connect(&config, deadline(), PostgresShutdown::default()).unwrap();
    assert_eq!(
        conn.query("SELECT repeat('x', 9)", &[], deadline(), |_| Ok(())),
        Err(PostgresError::RowLimit)
    );
    assert!(conn.is_closed());
    let mut config = options();
    config.max_columns = 1;
    let mut conn =
        PostgresConnection::connect(&config, deadline(), PostgresShutdown::default()).unwrap();
    assert_eq!(
        conn.query("SELECT 1,2", &[], deadline(), |_| Ok(())),
        Err(PostgresError::RowLimit)
    );
    let mut conn = connect();
    assert_eq!(
        conn.query(
            "SELECT n FROM generate_series(1,10000) n",
            &[],
            deadline(),
            |_| Err(PostgresError::Shutdown)
        ),
        Err(PostgresError::Shutdown)
    );
    assert!(conn.is_closed());
    let mut conn = connect();
    assert!(matches!(
        conn.query("COPY (SELECT 1) TO STDOUT", &[], deadline(), |_| Ok(())),
        Err(PostgresError::Protocol(_))
    ));
    assert!(conn.is_closed());
}

#[test]
#[ignore = "requires PostgreSQL 17/18 fixture and libpq 17+"]
fn query_deadline_and_shutdown_are_bounded() {
    let mut conn = connect();
    let start = Instant::now();
    assert_eq!(
        conn.query(
            "SELECT pg_sleep(10)",
            &[],
            start + Duration::from_millis(150),
            |_| Ok(())
        ),
        Err(PostgresError::Timeout)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(conn.is_closed());
    let shutdown = PostgresShutdown::default();
    let mut conn = PostgresConnection::connect(&options(), deadline(), shutdown.clone()).unwrap();
    let signal = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        shutdown.request();
    });
    let start = Instant::now();
    assert_eq!(
        conn.query("SELECT pg_sleep(10)", &[], deadline(), |_| Ok(())),
        Err(PostgresError::Shutdown)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    signal.join().unwrap();
    assert!(conn.is_closed());
}

#[test]
#[ignore = "requires libpq 17+"]
fn stalled_handshake_obeys_deadline_and_shutdown() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let config = PostgresConnectOptions::new(
        format!(
            "user=postgres dbname=postgres port={} sslmode=disable",
            listener.local_addr().unwrap().port()
        ),
        PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
    );
    let start = Instant::now();
    assert!(matches!(
        PostgresConnection::connect(
            &config,
            start + Duration::from_millis(150),
            PostgresShutdown::default()
        ),
        Err(PostgresError::Timeout)
    ));
    assert!(start.elapsed() < Duration::from_secs(2));
    let shutdown = PostgresShutdown::default();
    let signal = shutdown.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        signal.request();
    });
    assert!(matches!(
        PostgresConnection::connect(&config, deadline(), shutdown),
        Err(PostgresError::Shutdown)
    ));
    thread.join().unwrap();
}

#[test]
#[ignore = "requires PostgreSQL 17/18 fixture and libpq 17+"]
fn disconnect_does_not_reconnect() {
    let mut conn = connect();
    let mut pid = String::new();
    conn.query("SELECT pg_backend_pid()", &[], deadline(), |row| {
        pid = String::from_utf8(row.columns[0].clone().unwrap()).unwrap();
        Ok(())
    })
    .unwrap();
    connect()
        .query(
            "SELECT pg_terminate_backend($1::int)",
            &[PostgresParam::Text(23, &pid)],
            deadline(),
            |_| Ok(()),
        )
        .unwrap();
    assert!(conn.query("SELECT 1", &[], deadline(), |_| Ok(())).is_err());
    assert!(conn.is_closed());
}

#[test]
#[ignore = "requires TLS fixture from scripts/test-postgres-adapter.sh"]
fn service_password_file_tls_and_encoding_policy() {
    assert_eq!(std::env::var("MOOR_PG_TEST_TLS").as_deref(), Ok("1"));
    let mut conn = connect();
    conn.query(
        "SELECT ssl FROM pg_catalog.pg_stat_ssl WHERE pid = pg_backend_pid()",
        &[],
        deadline(),
        |row| {
            assert_eq!(row.columns[0].as_deref(), Some(b"t".as_slice()));
            Ok(())
        },
    )
    .unwrap();
    // Password-file lookup and certificate validation both use the configured hostname.
    let mut invalid = options();
    invalid.connection.push_str(" host=invalid.example");
    assert!(matches!(
        PostgresConnection::connect(&invalid, deadline(), PostgresShutdown::default()),
        Err(PostgresError::Connection)
    ));
    let mut invalid = options();
    invalid.connection.push_str(" password=wrong-password");
    assert!(matches!(
        PostgresConnection::connect(&invalid, deadline(), PostgresShutdown::default()),
        Err(PostgresError::Connection)
    ));
    let mut invalid = options();
    invalid.connection.push_str(" dbname=adapter_latin1");
    assert!(matches!(
        PostgresConnection::connect(&invalid, deadline(), PostgresShutdown::default()),
        Err(PostgresError::Encoding)
    ));
}

#[test]
fn configuration_and_expired_deadlines_fail_before_network_io() {
    let mut config = PostgresConnectOptions::new(
        "password=secret\0",
        PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
    );
    assert!(matches!(
        PostgresConnection::connect(&config, deadline(), PostgresShutdown::default()),
        Err(PostgresError::Configuration(_))
    ));
    config.connection.clear();
    assert!(matches!(
        PostgresConnection::connect(&config, Instant::now(), PostgresShutdown::default()),
        Err(PostgresError::Timeout)
    ));
    let shutdown = PostgresShutdown::default();
    shutdown.request();
    assert!(matches!(
        PostgresConnection::connect(&config, deadline(), shutdown),
        Err(PostgresError::Shutdown)
    ));
}

#[test]
#[ignore = "requires PostgreSQL 17/18 fixture and libpq 17+"]
fn callback_panic_closes_unfinished_stream() {
    let mut conn = connect();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = conn.query(
            "SELECT n FROM generate_series(1,10000) n",
            &[],
            deadline(),
            |_| panic!("callback panic"),
        );
    }));
    assert!(panic.is_err());
    assert!(conn.is_closed());
}

#[test]
#[ignore = "requires socket fixture from scripts/test-postgres-adapter.sh"]
fn unix_endpoint_overrides_connection_address() {
    let path = std::env::var("MOOR_PG_TEST_SOCKET").unwrap();
    let mut options = options();
    // The mapped TCP port is not the port used for the Unix socket filename.
    options.connection.push_str(" hostaddr=192.0.2.1 port=5432");
    options.endpoint = PostgresEndpoint::Unix(path.into());
    let mut conn =
        PostgresConnection::connect(&options, deadline(), PostgresShutdown::default()).unwrap();
    conn.query(
        "SELECT inet_client_addr() IS NULL",
        &[],
        deadline(),
        |row| {
            assert_eq!(row.columns[0].as_deref(), Some(b"t".as_slice()));
            Ok(())
        },
    )
    .unwrap();
}

#[test]
#[ignore = "requires libpq 17+"]
fn stalled_upload_obeys_deadline() {
    use std::io::{Read, Write};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (stop, stopped) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut length = [0; 4];
        socket.read_exact(&mut length).unwrap();
        let length = u32::from_be_bytes(length) as usize;
        assert!((8..65536).contains(&length));
        let mut startup = vec![0; length - 4];
        socket.read_exact(&mut startup).unwrap();
        // Minimal protocol-3 handshake, followed by a peer that never reads query data.
        socket.write_all(&[b'R', 0, 0, 0, 8, 0, 0, 0, 0]).unwrap();
        for (key, value) in [
            ("server_version", "17.0"),
            ("server_encoding", "UTF8"),
            ("client_encoding", "UTF8"),
        ] {
            let mut packet = vec![b'S'];
            packet.extend_from_slice(&((key.len() + value.len() + 6) as u32).to_be_bytes());
            packet.extend_from_slice(key.as_bytes());
            packet.push(0);
            packet.extend_from_slice(value.as_bytes());
            packet.push(0);
            socket.write_all(&packet).unwrap();
        }
        socket.write_all(&[b'Z', 0, 0, 0, 5, b'I']).unwrap();
        let _ = stopped.recv_timeout(Duration::from_secs(5));
    });
    let config = PostgresConnectOptions::new(
        format!("host=localhost port={port} dbname=postgres user=postgres sslmode=disable"),
        PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
    );
    let mut conn =
        PostgresConnection::connect(&config, deadline(), PostgresShutdown::default()).unwrap();
    let bytes = vec![42; 32 * 1024 * 1024];
    let start = Instant::now();
    assert_eq!(
        conn.query(
            "SELECT $1::bytea",
            &[PostgresParam::Binary(17, &bytes)],
            start + Duration::from_millis(150),
            |_| Ok(())
        ),
        Err(PostgresError::Timeout)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(conn.is_closed());
    stop.send(()).unwrap();
    server.join().unwrap();
}
