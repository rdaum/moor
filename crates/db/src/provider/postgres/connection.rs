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

use super::{PostgresConnectOptions, PostgresEndpoint, PostgresError, PostgresShutdown};
use pq_sys as pq;
use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    ptr::{self, NonNull},
    rc::Rc,
    time::{Duration, Instant},
};

/// A parameter's type OID may be zero when the SQL expression supplies its type.
/// Prepared execution uses the statement's established types instead of these OIDs.
/// Binary values must use the PostgreSQL wire format for that type.
#[derive(Clone, Copy)]
pub enum PostgresParam<'a> {
    Null(u32),
    Text(u32, &'a str),
    Binary(u32, &'a [u8]),
}

/// One owned row in PostgreSQL text format. NULL is distinct from an empty value.
#[derive(Debug, PartialEq, Eq)]
pub struct PostgresRow {
    pub columns: Vec<Option<Vec<u8>>>,
}

/// Counts for a completed statement. `rows` counts callbacks; `affected_rows` comes from the command tag.
#[derive(Debug, PartialEq, Eq)]
pub struct PostgresStatementResult {
    pub rows: u64,
    pub affected_rows: Option<u64>,
}

/// A connection belongs to the thread that creates it; it is neither Send nor Sync.
///
/// Query callbacks see bounded, owned rows and must return promptly. A timeout, shutdown,
/// callback failure, SQL error, or unsupported protocol response closes the connection.
/// No operation reconnects automatically or retries an ambiguous write.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<moor_db::PostgresConnection>();
/// ```
///
/// ```compile_fail
/// fn require_sync<T: Sync>() {}
/// require_sync::<moor_db::PostgresConnection>();
/// ```
pub struct PostgresConnection {
    raw: Option<NonNull<pq::PGconn>>,
    shutdown: PostgresShutdown,
    max_row_bytes: usize,
    max_columns: usize,
    _thread: PhantomData<Rc<()>>,
}

impl PostgresConnection {
    /// Connect on a dedicated worker. The deadline covers setup and libpq polling.
    pub fn connect(
        options: &PostgresConnectOptions,
        deadline: Instant,
        shutdown: PostgresShutdown,
    ) -> Result<Self, PostgresError> {
        options.validate()?;
        shutdown.check(deadline)?;
        // SAFETY: PQlibVersion has no arguments or per-connection state.
        if unsafe { pq::PQlibVersion() } < 170000 {
            return Err(PostgresError::UnsupportedVersion);
        }
        let connection = connection_settings(options)?;
        shutdown.check(deadline)?;
        let (host_key, host_value) = match &options.endpoint {
            PostgresEndpoint::Tcp(ip) => ("hostaddr", ip.to_string()),
            PostgresEndpoint::Unix(path) => ("host", path.to_str().unwrap().to_owned()),
        };
        // Later entries override connection-string/service settings. Disallow authentication
        // mechanisms that can perform their own blocking hostname lookups.
        let entries = [
            ("dbname", connection.as_str()),
            (host_key, host_value.as_str()),
            ("application_name", options.application_name.as_str()),
            ("client_encoding", "UTF8"),
            ("options", "-c search_path=pg_catalog"),
            ("gssencmode", "disable"),
            ("require_auth", "!gss,!sspi"),
        ];
        let keys: Vec<_> = entries
            .iter()
            .map(|(k, _)| cstring(k))
            .collect::<Result<_, _>>()?;
        let values: Vec<_> = entries
            .iter()
            .map(|(_, v)| cstring(v))
            .collect::<Result<_, _>>()?;
        let key_ptrs: Vec<_> = keys
            .iter()
            .map(|s| s.as_ptr())
            .chain([ptr::null()])
            .collect();
        let value_ptrs: Vec<_> = values
            .iter()
            .map(|s| s.as_ptr())
            .chain([ptr::null()])
            .collect();
        // SAFETY: arrays are equally sized, NULL-terminated, and their C strings remain alive.
        let raw = NonNull::new(unsafe {
            pq::PQconnectStartParams(key_ptrs.as_ptr(), value_ptrs.as_ptr(), 1)
        })
        .ok_or(PostgresError::Connection)?;
        let conn = Self {
            raw: Some(raw),
            shutdown,
            max_row_bytes: options.max_row_bytes,
            max_columns: options.max_columns,
            _thread: PhantomData,
        };
        // SAFETY: raw is owned by conn; the callback retains no borrowed data or state.
        unsafe {
            pq::PQsetNoticeProcessor(raw.as_ptr(), Some(ignore_notice), ptr::null_mut());
        }
        // PQconnectStart requires waiting for writability before the first connectPoll.
        let mut status = pq::PostgresPollingStatusType::PGRES_POLLING_WRITING;
        loop {
            conn.shutdown.check(deadline)?;
            match status {
                pq::PostgresPollingStatusType::PGRES_POLLING_OK => break,
                pq::PostgresPollingStatusType::PGRES_POLLING_READING => {
                    conn.wait(libc::POLLIN, deadline)?;
                }
                pq::PostgresPollingStatusType::PGRES_POLLING_WRITING => {
                    conn.wait(libc::POLLOUT, deadline)?;
                }
                _ => return Err(PostgresError::Connection),
            }
            // SAFETY: conn owns raw exclusively; the socket was polled for the requested event.
            status = unsafe { pq::PQconnectPoll(raw.as_ptr()) };
        }
        // SAFETY: the connection is established, valid, and exclusively owned.
        unsafe {
            if pq::PQsetnonblocking(raw.as_ptr(), 1) != 0 {
                return Err(PostgresError::Connection);
            }
            let version = pq::PQserverVersion(raw.as_ptr());
            if !(170000..190000).contains(&version) {
                return Err(PostgresError::UnsupportedVersion);
            }
            for name in [c"server_encoding", c"client_encoding"] {
                let encoding = pq::PQparameterStatus(raw.as_ptr(), name.as_ptr());
                if encoding.is_null() || CStr::from_ptr(encoding).to_bytes() != b"UTF8" {
                    return Err(PostgresError::Encoding);
                }
            }
        }
        conn.shutdown.check(deadline)?;
        Ok(conn)
    }

    pub fn is_closed(&self) -> bool {
        self.raw.is_none()
    }

    /// Send one parameterized statement. Results are delivered one row at a time.
    pub fn query(
        &mut self,
        sql: &str,
        params: &[PostgresParam<'_>],
        deadline: Instant,
        mut row: impl FnMut(PostgresRow) -> Result<(), PostgresError>,
    ) -> Result<PostgresStatementResult, PostgresError> {
        let sql = cstring(sql)?;
        let params = Parameters::new(params)?;
        self.run(
            deadline,
            true,
            |conn| {
                // SAFETY: every pointer refers to a buffer in sql/params, retained through run.
                unsafe {
                    pq::PQsendQueryParams(
                        conn,
                        sql.as_ptr(),
                        params.count,
                        params.types.as_ptr(),
                        params.pointers.as_ptr(),
                        params.lengths.as_ptr(),
                        params.formats.as_ptr(),
                        0,
                    )
                }
            },
            &mut row,
        )
    }

    /// Prepare a statement on this connection. No SQL identifiers are interpolated.
    pub fn prepare(
        &mut self,
        name: &str,
        sql: &str,
        types: &[u32],
        deadline: Instant,
    ) -> Result<(), PostgresError> {
        let name = cstring(name)?;
        let sql = cstring(sql)?;
        let count = parameter_count(types.len())?;
        self.run(
            deadline,
            false,
            |conn| {
                // SAFETY: name, sql, and types are valid for the call and remain alive through run.
                unsafe {
                    pq::PQsendPrepare(conn, name.as_ptr(), sql.as_ptr(), count, types.as_ptr())
                }
            },
            &mut |_| Err(PostgresError::Protocol("rows during preparation")),
        )?;
        Ok(())
    }

    /// Execute a previously prepared statement, delivering one owned row per callback.
    pub fn execute_prepared(
        &mut self,
        name: &str,
        params: &[PostgresParam<'_>],
        deadline: Instant,
        mut row: impl FnMut(PostgresRow) -> Result<(), PostgresError>,
    ) -> Result<PostgresStatementResult, PostgresError> {
        let name = cstring(name)?;
        let params = Parameters::new(params)?;
        self.run(
            deadline,
            true,
            |conn| {
                // SAFETY: all parameter arrays have count entries and retain their backing buffers.
                unsafe {
                    pq::PQsendQueryPrepared(
                        conn,
                        name.as_ptr(),
                        params.count,
                        params.pointers.as_ptr(),
                        params.lengths.as_ptr(),
                        params.formats.as_ptr(),
                        0,
                    )
                }
            },
            &mut row,
        )
    }

    fn run(
        &mut self,
        deadline: Instant,
        stream: bool,
        send: impl FnOnce(*mut pq::PGconn) -> i32,
        row: &mut impl FnMut(PostgresRow) -> Result<(), PostgresError>,
    ) -> Result<PostgresStatementResult, PostgresError> {
        let mut guard = ActiveQuery {
            connection: self,
            complete: false,
        };
        let connection = &mut *guard.connection;
        let result = (|| {
            let raw = connection.raw()?.as_ptr();
            connection.shutdown.check(deadline)?;
            if send(raw) != 1 {
                return Err(PostgresError::Connection);
            }
            // SAFETY: one query was just sent, with no intervening result retrieval.
            if stream && unsafe { pq::PQsetSingleRowMode(raw) } != 1 {
                return Err(PostgresError::Protocol("single-row mode unavailable"));
            }
            loop {
                connection.shutdown.check(deadline)?;
                // SAFETY: this worker owns the connection; nonblocking mode is enabled.
                match unsafe { pq::PQflush(raw) } {
                    0 => break,
                    1 => {
                        let events = connection.wait(libc::POLLIN | libc::POLLOUT, deadline)?;
                        if events & (libc::POLLIN | libc::POLLHUP) != 0 {
                            connection.consume()?;
                        }
                    }
                    _ => return Err(PostgresError::Connection),
                }
            }
            let mut summary = PostgresStatementResult {
                rows: 0,
                affected_rows: None,
            };
            let mut complete = false;
            loop {
                connection.shutdown.check(deadline)?;
                // SAFETY: connection remains exclusively owned and no PGresult is live here.
                while unsafe { pq::PQisBusy(raw) } != 0 {
                    connection.wait(libc::POLLIN, deadline)?;
                    connection.consume()?;
                }
                // SAFETY: isBusy returned zero, so getResult will not block.
                let result = unsafe { pq::PQgetResult(raw) };
                let Some(result) = NonNull::new(result).map(QueryResult) else {
                    if !complete {
                        return Err(PostgresError::Protocol("missing statement completion"));
                    }
                    return Ok(summary);
                };
                // SAFETY: QueryResult owns a live PGresult through this iteration.
                match unsafe { pq::PQresultStatus(result.0.as_ptr()) } {
                    pq::ExecStatusType::PGRES_SINGLE_TUPLE if stream && !complete => {
                        row(result.row(connection.max_columns, connection.max_row_bytes)?)?;
                        summary.rows =
                            summary.rows.checked_add(1).ok_or(PostgresError::RowLimit)?;
                    }
                    pq::ExecStatusType::PGRES_TUPLES_OK | pq::ExecStatusType::PGRES_COMMAND_OK
                        if !complete =>
                    {
                        // SAFETY: result is live; final single-row-mode results contain no rows.
                        if unsafe { pq::PQntuples(result.0.as_ptr()) } != 0 {
                            return Err(PostgresError::Protocol("unbounded result"));
                        }
                        complete = true;
                        summary.affected_rows = result.affected_rows()?;
                    }
                    pq::ExecStatusType::PGRES_FATAL_ERROR
                    | pq::ExecStatusType::PGRES_NONFATAL_ERROR => return Err(result.error()),
                    _ => return Err(PostgresError::Protocol("unsupported result status")),
                }
            }
        })();
        guard.complete = result.is_ok();
        result
    }

    fn raw(&self) -> Result<NonNull<pq::PGconn>, PostgresError> {
        self.raw.ok_or(PostgresError::Closed)
    }

    fn consume(&self) -> Result<(), PostgresError> {
        // SAFETY: the worker exclusively owns this nonblocking connection.
        if unsafe { pq::PQconsumeInput(self.raw()?.as_ptr()) } == 1 {
            return Ok(());
        }
        Err(PostgresError::Connection)
    }

    fn wait(&self, events: i16, deadline: Instant) -> Result<i16, PostgresError> {
        loop {
            self.shutdown.check(deadline)?;
            // SAFETY: the connection is live; libpq may change sockets between connectPoll calls.
            let socket = unsafe { pq::PQsocket(self.raw()?.as_ptr()) };
            if socket < 0 {
                return Err(PostgresError::Connection);
            }
            let timeout = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(25));
            let millis = timeout.as_millis().max(1) as i32;
            let mut fd = libc::pollfd {
                fd: socket,
                events,
                revents: 0,
            };
            // SAFETY: fd points to one initialized pollfd; poll only borrows it for this call.
            let status = unsafe { libc::poll(&mut fd, 1, millis) };
            if status < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(PostgresError::Connection);
            }
            if status == 0 {
                continue;
            }
            if fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
                return Err(PostgresError::Connection);
            }
            // Let libpq consume EOF or its final error message after a hangup.
            return Ok(fd.revents);
        }
    }

    fn close(&mut self) {
        if let Some(raw) = self.raw.take() {
            // SAFETY: this is the unique owner; no result or callback retains the connection.
            unsafe {
                pq::PQfinish(raw.as_ptr());
            }
        }
    }
}

impl Drop for PostgresConnection {
    fn drop(&mut self) {
        self.close();
    }
}

// A caught callback panic must not leave a connection with an unfinished result stream.
struct ActiveQuery<'a> {
    connection: &'a mut PostgresConnection,
    complete: bool,
}
impl Drop for ActiveQuery<'_> {
    fn drop(&mut self) {
        if !self.complete {
            self.connection.close();
        }
    }
}

// libpq normally prints notices to stderr; suppress messages that may contain application data.
unsafe extern "C" fn ignore_notice(_: *mut libc::c_void, _: *const libc::c_char) {}

fn cstring(value: &str) -> Result<CString, PostgresError> {
    CString::new(value).map_err(|_| PostgresError::Configuration("text contains NUL"))
}

fn parameter_count(count: usize) -> Result<i32, PostgresError> {
    u16::try_from(count)
        .map(i32::from)
        .map_err(|_| PostgresError::Configuration("too many parameters"))
}

struct Parameters<'a> {
    _borrow: PhantomData<&'a [u8]>,
    count: i32,
    types: Vec<u32>,
    pointers: Vec<*const libc::c_char>,
    lengths: Vec<i32>,
    formats: Vec<i32>,
    _text: Vec<CString>,
}

impl<'a> Parameters<'a> {
    fn new(params: &[PostgresParam<'a>]) -> Result<Self, PostgresError> {
        let count = parameter_count(params.len())?;
        let mut result = Self {
            _borrow: PhantomData,
            count,
            types: Vec::with_capacity(params.len()),
            pointers: Vec::with_capacity(params.len()),
            lengths: Vec::with_capacity(params.len()),
            formats: Vec::with_capacity(params.len()),
            _text: Vec::new(),
        };
        for param in params {
            match param {
                PostgresParam::Null(oid) => {
                    result.types.push(*oid);
                    result.pointers.push(ptr::null());
                    result.lengths.push(0);
                    result.formats.push(0);
                }
                PostgresParam::Text(oid, text) => {
                    i32::try_from(text.len()).map_err(|_| {
                        PostgresError::Configuration("parameter exceeds libpq length limit")
                    })?;
                    let text = cstring(text)?;
                    result.types.push(*oid);
                    result.pointers.push(text.as_ptr());
                    result.lengths.push(0);
                    result.formats.push(0);
                    result._text.push(text);
                }
                PostgresParam::Binary(oid, bytes) => {
                    let length = i32::try_from(bytes.len()).map_err(|_| {
                        PostgresError::Configuration("parameter exceeds libpq length limit")
                    })?;
                    result.types.push(*oid);
                    result.pointers.push(bytes.as_ptr().cast());
                    result.lengths.push(length);
                    result.formats.push(1);
                }
            }
        }
        Ok(result)
    }
}

struct QueryResult(NonNull<pq::PGresult>);
impl QueryResult {
    fn row(&self, max_columns: usize, max_bytes: usize) -> Result<PostgresRow, PostgresError> {
        let raw = self.0.as_ptr();
        // SAFETY: self uniquely owns this result. All column indices are checked against nfields;
        // copied values use PQgetlength, not C-string termination, and cannot outlive the result.
        unsafe {
            if pq::PQntuples(raw) != 1 {
                return Err(PostgresError::Protocol("expected one row"));
            }
            let count = usize::try_from(pq::PQnfields(raw))
                .map_err(|_| PostgresError::Protocol("negative field count"))?;
            if count > max_columns {
                return Err(PostgresError::RowLimit);
            }
            let mut columns = Vec::with_capacity(count);
            let mut bytes = 0usize;
            for col in 0..count as i32 {
                if pq::PQgetisnull(raw, 0, col) != 0 {
                    columns.push(None);
                    continue;
                }
                let len = usize::try_from(pq::PQgetlength(raw, 0, col))
                    .map_err(|_| PostgresError::Protocol("negative field length"))?;
                bytes = bytes
                    .checked_add(len)
                    .filter(|n| *n <= max_bytes)
                    .ok_or(PostgresError::RowLimit)?;
                let value = pq::PQgetvalue(raw, 0, col);
                if value.is_null() {
                    return Err(PostgresError::Protocol("missing field value"));
                }
                columns.push(Some(std::slice::from_raw_parts(value.cast(), len).to_vec()));
            }
            Ok(PostgresRow { columns })
        }
    }

    fn affected_rows(&self) -> Result<Option<u64>, PostgresError> {
        // SAFETY: cmdTuples returns a NUL-terminated string owned by this live result.
        let raw = unsafe { pq::PQcmdTuples(self.0.as_ptr()) };
        if raw.is_null() {
            return Err(PostgresError::Protocol("missing command count"));
        }
        // SAFETY: raw was checked and is valid until this result is dropped.
        let text = unsafe { CStr::from_ptr(raw) }
            .to_str()
            .map_err(|_| PostgresError::Protocol("invalid command count"))?;
        if text.is_empty() {
            return Ok(None);
        }
        text.parse()
            .map(Some)
            .map_err(|_| PostgresError::Protocol("invalid command count"))
    }

    fn error(&self) -> PostgresError {
        // SAFETY: the returned field is owned by this result and copied before drop.
        let raw =
            unsafe { pq::PQresultErrorField(self.0.as_ptr(), i32::from(pq::PG_DIAG_SQLSTATE)) };
        if raw.is_null() {
            return PostgresError::Connection;
        }
        // SAFETY: libpq guarantees that non-NULL diagnostic fields are NUL-terminated.
        let state = unsafe { CStr::from_ptr(raw) }.to_bytes();
        if state.len() != 5 || !state.iter().all(u8::is_ascii_alphanumeric) {
            return PostgresError::Protocol("invalid SQLSTATE");
        }
        PostgresError::SqlState(String::from_utf8_lossy(state).into_owned())
    }
}
impl Drop for QueryResult {
    fn drop(&mut self) {
        // SAFETY: self is the unique owner of the result and no borrowed fields escape it.
        unsafe {
            pq::PQclear(self.0.as_ptr());
        }
    }
}

/// Service lookup must remain local: libpq otherwise supports blocking LDAP requests here.
fn connection_settings(options: &PostgresConnectOptions) -> Result<String, PostgresError> {
    let connection = cstring(&options.connection)?;
    // SAFETY: input is NUL-terminated. With a NULL error pointer libpq frees its diagnostics.
    // PQconninfoParse does not apply defaults, read service files, or perform network I/O.
    let raw = NonNull::new(unsafe { pq::PQconninfoParse(connection.as_ptr(), ptr::null_mut()) })
        .ok_or(PostgresError::Configuration(
            "invalid connection specification",
        ))?;
    let info = ConnectionInfo(raw);
    let mut service = std::env::var_os("PGSERVICE").map(|s| s.to_string_lossy().into_owned());
    let mut settings = String::new();
    // SAFETY: libpq returns a keyword-NULL-terminated array. Values remain owned by info.
    unsafe {
        let mut entry = info.0.as_ptr();
        while !(*entry).keyword.is_null() {
            if !(*entry).val.is_null() {
                let key = CStr::from_ptr((*entry).keyword)
                    .to_str()
                    .map_err(|_| PostgresError::Configuration("invalid connection keyword"))?;
                let value = CStr::from_ptr((*entry).val)
                    .to_str()
                    .map_err(|_| PostgresError::Configuration("connection values must be UTF8"))?;
                if key == "service" {
                    service = Some(value.to_owned());
                }
                settings.push_str(key);
                settings.push_str("='");
                for character in value.chars() {
                    if matches!(character, '\\' | '\'') {
                        settings.push('\\');
                    }
                    settings.push(character);
                }
                settings.push_str("' ");
            }
            entry = entry.add(1);
        }
    }
    if matches!(options.endpoint, PostgresEndpoint::Unix(_)) {
        // Empty array values are ignored by PQconnectStartParams. An expanded conninfo
        // value can explicitly clear hostaddr, including service/environment defaults.
        settings.push_str("hostaddr='' ");
    }
    let Some(service) = service else {
        return Ok(settings);
    };
    // Requiring an explicit file also prevents fallback to an unexamined system service file.
    let path = std::env::var_os("PGSERVICEFILE").ok_or(PostgresError::Configuration(
        "service connections require PGSERVICEFILE",
    ))?;
    let file = std::fs::read_to_string(path)
        .map_err(|_| PostgresError::Configuration("cannot read service file"))?;
    validate_service_file(&file, &service)?;
    Ok(settings)
}

fn validate_service_file(file: &str, service: &str) -> Result<(), PostgresError> {
    let group = format!("[{service}]");
    let mut found = false;
    for line in file.lines().map(str::trim_ascii) {
        if line.starts_with("ldap") {
            return Err(PostgresError::Configuration(
                "LDAP service lookup is unsupported",
            ));
        }
        found |= line.starts_with(&group);
    }
    if !found {
        return Err(PostgresError::Configuration(
            "service is missing from PGSERVICEFILE",
        ));
    }
    Ok(())
}

struct ConnectionInfo(NonNull<pq::PQconninfoOption>);
impl Drop for ConnectionInfo {
    fn drop(&mut self) {
        // SAFETY: self uniquely owns this array and all libpq-allocated strings within it.
        unsafe {
            pq::PQconninfoFree(self.0.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_lookup_cannot_escape_to_ldap_or_system_file() {
        assert!(validate_service_file("[world]\nhost=localhost\n", "world").is_ok());
        assert!(validate_service_file("[other]\nhost=localhost\n", "world").is_err());
        assert!(validate_service_file("[world]\n ldap://directory/db\n", "world").is_err());
    }

    #[test]
    fn parameters_reject_nul_and_preserve_empty_binary() {
        assert!(Parameters::new(&[PostgresParam::Text(25, "a\0b")]).is_err());
        let params =
            Parameters::new(&[PostgresParam::Binary(17, &[]), PostgresParam::Null(17)]).unwrap();
        assert!(!params.pointers[0].is_null());
        assert!(params.pointers[1].is_null());
        assert_eq!(params.lengths, [0, 0]);
        assert_eq!(parameter_count(65535), Ok(65535));
        assert!(parameter_count(65536).is_err());
    }
}
