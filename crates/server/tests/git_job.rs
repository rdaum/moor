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

//! Verify that the combined server dispatches isolated jobs before starting its runtime.
use moor_git_worker::{job::Executor, protocol::map};
use moor_var::{Associative, v_int, v_str};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn server_child_retrieves_git_without_starting_world() {
    let root = tempfile::tempdir().unwrap();
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/repo", server.local_addr().unwrap());
    let responder = tokio::spawn(async move {
        let (mut connection, _) = tokio::time::timeout(Duration::from_secs(5), server.accept())
            .await
            .unwrap()
            .unwrap();
        let mut request = Vec::new();
        loop {
            let byte = connection.read_u8().await.unwrap();
            request.push(byte);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
            assert!(request.len() < 16384);
        }
        assert!(
            String::from_utf8_lossy(&request)
                .starts_with("GET /repo/info/refs?service=git-upload-pack ")
        );
        connection
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
    });
    let executor = Executor::new(
        env!("CARGO_BIN_EXE_moor").into(),
        root.path().join("jobs"),
        Default::default(),
        1,
    )
    .unwrap();
    let response = executor
        .perform(
            vec![
                v_str("refs"),
                map(&[("schema", v_int(1)), ("repository", v_str(&url))]),
            ],
            Some(Duration::from_secs(5)),
        )
        .await
        .unwrap();
    let error = response.as_map().unwrap().get(&v_str("error")).unwrap();
    assert_eq!(
        error.as_map().unwrap().get(&v_str("code")).unwrap(),
        v_str("authentication_required"),
        "{response:?}"
    );
    responder.await.unwrap();
    assert_eq!(
        std::fs::read_dir(root.path().join("jobs")).unwrap().count(),
        0
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}
