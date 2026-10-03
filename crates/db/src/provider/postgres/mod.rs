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

//! Readable PostgreSQL world storage with thread-owned connections and bounded network I/O.
//!
//! Connections must be created and used on persistence or administrative workers.
//! Encoder workers validate reload limits before publication. The ordered writer applies SQL
//! asynchronously; transaction workers never access a libpq connection.

mod apply;
mod codec;
mod config;
mod connection;
mod encode;
mod metrics;
mod options;
mod reader;
mod rows;
mod schema;
mod seed;
mod snapshot;
mod sql;
mod state;
mod writer;
pub(crate) use encode::EncodedCommit;
pub(crate) use writer::PostgresWriter;

pub use config::{PostgresCommitPolicy, PostgresStorageConfig};
pub use connection::{PostgresConnection, PostgresParam, PostgresRow, PostgresStatementResult};
pub use options::{PostgresConnectOptions, PostgresEndpoint, PostgresSchema};
pub use schema::initialize_postgres_schema;

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
    #[error("PostgreSQL {operation} on {relation}: {source}")]
    Operation {
        relation: &'static str,
        operation: &'static str,
        source: Box<PostgresError>,
    },
    #[error("PostgreSQL relation {relation}, {key}: {source}")]
    Row {
        relation: &'static str,
        key: String,
        source: Box<PostgresError>,
    },
    #[error("PostgreSQL {field} codec: {detail}")]
    Codec { field: &'static str, detail: String },
    #[error("PostgreSQL writer ownership is unavailable or was lost")]
    OwnershipLost,
    #[error("invalid PostgreSQL stored {field}: {reason}")]
    Format {
        field: &'static str,
        reason: &'static str,
    },
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
    #[error("PostgreSQL requires libpq 16 or newer and server 16, 17, or 18")]
    UnsupportedVersion,
    #[error("PostgreSQL requires a thread-safe libpq build")]
    ThreadSafety,
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
    pub(crate) fn is_requested(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub(crate) fn check(&self, deadline: Instant) -> Result<(), PostgresError> {
        if self.is_requested() {
            return Err(PostgresError::Shutdown);
        }
        if Instant::now() >= deadline {
            return Err(PostgresError::Timeout);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
