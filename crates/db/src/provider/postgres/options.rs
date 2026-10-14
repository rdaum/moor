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

use super::PostgresError;
use std::{fmt, net::IpAddr, path::PathBuf};

/// A numeric address avoids blocking DNS in libpq's connection polling.
#[derive(Clone, Debug)]
pub enum PostgresEndpoint {
    Tcp(IpAddr),
    Unix(PathBuf),
}

/// Validated SQL schema identifier. Qualification never depends on search_path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresSchema(String);

impl PostgresSchema {
    pub fn new(name: &str) -> Result<Self, PostgresError> {
        if name.is_empty()
            || name.len() > 63
            || !name
                .bytes()
                .enumerate()
                .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
        {
            return Err(PostgresError::Configuration(
                "schema must be an ASCII identifier of 1..63 bytes",
            ));
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Quote each identifier separately; relation names follow the same validation rules.
    pub fn qualify(&self, relation: &str) -> Result<String, PostgresError> {
        let relation = Self::new(relation)?;
        Ok(format!("\"{}\".\"{}\"", self.0, relation.0))
    }
}

/// libpq configuration plus an explicit endpoint and per-row decode limits.
///
/// `connection` accepts a libpq connection string, URI, or `service=NAME`.
/// Service connections require an explicit PGSERVICEFILE containing the selected service.
/// LDAP service lookup is rejected. Keep this local file unchanged during connection setup.
/// Credentials, TLS options, and password files are handled by libpq. The endpoint,
/// application name, UTF8 client encoding, and catalog-only search path are enforced.
#[derive(Clone)]
pub struct PostgresConnectOptions {
    pub connection: String,
    pub endpoint: PostgresEndpoint,
    pub application_name: String,
    pub max_row_bytes: usize,
    pub max_columns: usize,
}

impl PostgresConnectOptions {
    pub fn new(connection: impl Into<String>, endpoint: PostgresEndpoint) -> Self {
        Self {
            connection: connection.into(),
            endpoint,
            application_name: "moor-persistence".into(),
            max_row_bytes: 16 * 1024 * 1024,
            max_columns: 256,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), PostgresError> {
        if self.application_name.is_empty() {
            return Err(PostgresError::Configuration(
                "application name must not be empty",
            ));
        }
        if self.max_row_bytes == 0 || self.max_columns == 0 {
            return Err(PostgresError::Configuration("row limits must be positive"));
        }
        if self.connection.contains('\0') || self.application_name.contains('\0') {
            return Err(PostgresError::Configuration(
                "connection options cannot contain NUL",
            ));
        }
        if let PostgresEndpoint::Unix(path) = &self.endpoint
            && (!path.is_absolute() || path.to_str().is_none_or(|p| p.contains(['\0', ','])))
        {
            return Err(PostgresError::Configuration(
                "socket directory must be an absolute UTF8 path without commas",
            ));
        }
        Ok(())
    }
}

impl fmt::Debug for PostgresConnectOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresConnectOptions")
            .field("connection", &"<redacted>")
            .field("endpoint", &"<redacted>")
            .field("max_row_bytes", &self.max_row_bytes)
            .field("max_columns", &self.max_columns)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn socket_directory_cannot_become_a_libpq_host_list() {
        let options = PostgresConnectOptions::new(
            "",
            PostgresEndpoint::Unix("/tmp/socket,example.com".into()),
        );
        assert!(options.validate().is_err());
        let mut options =
            PostgresConnectOptions::new("", PostgresEndpoint::Unix("/tmp/socket".into()));
        assert!(options.validate().is_ok());
        options.application_name.clear();
        assert!(options.validate().is_err());
    }
    #[test]
    fn identifiers_are_qualified_and_bounded() {
        let schema = PostgresSchema::new("Mixed_Case").unwrap();
        assert_eq!(
            schema.qualify("object_names").unwrap(),
            "\"Mixed_Case\".\"object_names\""
        );
        for name in ["", "x.y", "x\"; DROP SCHEMA public;--", "1abc", "é"] {
            assert!(PostgresSchema::new(name).is_err());
        }
        assert!(PostgresSchema::new(&"x".repeat(64)).is_err());
    }
    #[test]
    fn debug_redacts_connection_details() {
        let options = PostgresConnectOptions::new(
            "password=test-secret",
            PostgresEndpoint::Tcp("127.0.0.1".parse().unwrap()),
        );
        let rendered = format!("{options:?}");
        assert!(!rendered.contains("test-secret"));
        assert!(!rendered.contains("127.0.0.1"));
    }
}
