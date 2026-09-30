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

//! Thread-owned libpq connections with bounded network I/O.
//!
//! Connections must be created and used on persistence or administrative workers.
//! This module does not implement world storage or run SQL on transaction workers.

mod connection;
mod options;

pub use connection::{PostgresConnection, PostgresParam, PostgresRow, PostgresStatementResult};
pub use options::{PostgresConnectOptions, PostgresEndpoint, PostgresSchema};

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

/// Redacted adapter errors. Server messages can contain credentials or application values.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PostgresError {
    #[error("invalid PostgreSQL configuration: {0}")]
    Configuration(&'static str),
    #[error("PostgreSQL operation exceeded its deadline")]
    Timeout,
    #[error("PostgreSQL worker is shutting down")]
    Shutdown,
    #[error("PostgreSQL connection failed or was lost")]
    Connection,
    #[error("PostgreSQL connection is closed")]
    Closed,
    #[error("PostgreSQL returned SQLSTATE {0}")]
    SqlState(String),
    #[error("unexpected PostgreSQL protocol result: {0}")]
    Protocol(&'static str),
    #[error("PostgreSQL row exceeds its configured limit")]
    RowLimit,
    #[error("PostgreSQL requires libpq 17 or newer and server 17 or 18")]
    UnsupportedVersion,
    #[error("PostgreSQL database and client encoding must be UTF8")]
    Encoding,
}

/// Shared shutdown signal, checked before I/O and at most every 25 ms while polling.
#[derive(Clone, Default, Debug)]
pub struct PostgresShutdown(Arc<AtomicBool>);

impl PostgresShutdown {
    pub fn request(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub(crate) fn check(&self, deadline: Instant) -> Result<(), PostgresError> {
        if self.0.load(Ordering::Acquire) {
            return Err(PostgresError::Shutdown);
        }
        if Instant::now() >= deadline {
            return Err(PostgresError::Timeout);
        }
        Ok(())
    }
}
