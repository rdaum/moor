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

//! LambdaMOO test harness for comparative testing against mooR.
//!
//! This crate wraps the original LambdaMOO C implementation to enable
//! direct command injection and output capture for correctness and
//! performance comparisons.
//!
//! Before building, run the setup script to fetch and configure sources:
//! ```text
//! ./crates/testing/lambdamoo-harness/setup-lambdamoo.sh
//! ```

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};
use std::path::Path;
use std::sync::Mutex;

/// LambdaMOO FFI bindings.
pub mod ffi {
    use super::*;

    /// LambdaMOO var_type enum
    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub enum VarType {
        Int = 0,
        Obj = 1,
        Str = 2, // _TYPE_STR, but with COMPLEX_FLAG this becomes 0x82
        Err = 3,
        List = 4, // _TYPE_LIST, but with COMPLEX_FLAG this becomes 0x84
        Clear = 5,
        None = 6,
        Catch = 7,
        Finally = 8,
        Float = 9, // _TYPE_FLOAT, but with COMPLEX_FLAG this becomes 0x89
    }

    /// Union part of Var - represents the value
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub union VarValue {
        pub str_val: *const c_char,
        pub num: i32,
        pub obj: i32,
        pub err: c_int,
        pub list: *mut Var,
        pub fnum: *mut f64,
    }

    /// LambdaMOO Var type - holds any MOO value
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct Var {
        pub v: VarValue,
        pub var_type: c_int, // var_type enum
    }

    // Type constants (with TYPE_COMPLEX_FLAG = 0x80 for complex types)
    pub const TYPE_INT: c_int = 0;
    pub const TYPE_STR: c_int = 2 | 0x80; // 0x82 = 130
    pub const TYPE_LIST: c_int = 4 | 0x80; // 0x84 = 132

    unsafe extern "C" {
        // ============================================================
        // Harness-specific API
        // ============================================================

        /// Initialize the test harness (call before network_initialize)
        pub fn harness_init();

        /// Cleanup the harness
        pub fn harness_cleanup();

        /// Get captured output. Returns pointer to buffer, sets *len to length.
        pub fn harness_get_output(len: *mut usize) -> *const c_char;

        /// Clear the output buffer
        pub fn harness_clear_output();

        /// Create a fake connection for testing. Returns connection ID or -1.
        pub fn harness_create_connection(player: i32) -> c_int;

        /// Queue a line of input for a connection
        pub fn harness_queue_input(connection_id: c_int, line: *const c_char) -> c_int;

        /// Close a harness connection
        pub fn harness_close_connection(connection_id: c_int);

        // ============================================================
        // LambdaMOO core API
        // ============================================================

        // Database functions
        pub fn db_initialize(argc: *mut c_int, argv: *mut *mut *mut c_char) -> c_int;
        pub fn db_load() -> c_int;
        pub fn db_shutdown();

        // Network functions
        pub fn network_initialize(argc: c_int, argv: *mut *mut c_char, desc: *mut Var) -> c_int;
        pub fn network_shutdown();
        pub fn network_process_io(timeout: c_int) -> c_int;

        // Server functions
        pub fn register_bi_functions();
        pub fn load_server_options();

        // Task functions
        pub fn run_ready_tasks();

        // Output function - Objid is int32 in LambdaMOO
        pub fn notify(player: i32, message: *const c_char);

        // ============================================================
        // Direct database/verb API (bypasses command parser)
        // ============================================================

        // Create a new object, returns its Objid
        pub fn db_create_object() -> i32;

        // Set object name
        pub fn db_set_object_name(oid: i32, name: *const c_char);

        // Set object owner
        pub fn db_set_object_owner(oid: i32, owner: i32);

        // Set object flags
        pub fn db_set_object_flag(oid: i32, flag: c_int);

        // Add a verb to an object
        // Returns verb index, or -1 on error
        pub fn db_add_verb(
            oid: i32,
            vnames: *const c_char,
            owner: i32,
            flags: c_uint,
            dobj: c_int,
            prep: c_int,
            iobj: c_int,
        ) -> c_int;

        // Find a verb handle for programming
        pub fn db_find_defined_verb(
            oid: i32,
            vname: *const c_char,
            allow_numbers: c_int,
        ) -> VerbHandle;

        // Find a callable verb (requires VF_EXEC flag)
        pub fn db_find_callable_verb(oid: i32, vname: *const c_char) -> VerbHandle;

        // Program a verb with MOO code
        pub fn db_set_verb_program(h: VerbHandle, program: *mut Program);

        // Compile MOO code (list of strings) to a program
        pub fn parse_list_as_program(code: Var, errors: *mut Var) -> *mut Program;

        // Create a MOO string (makes a copy)
        pub fn str_dup(s: *const c_char) -> *const c_char;

        // Append to a list, returns new list
        pub fn listappend(list: Var, value: Var) -> Var;

        // Run a verb directly (bypasses command parser)
        pub fn run_server_task(
            player: i32,
            what: i32,
            verb: *const c_char,
            args: Var,
            argstr: *const c_char,
            result: *mut Var,
        ) -> c_int;

        // Create an empty list
        pub fn new_list(size: c_int) -> Var;

        // Free a Var (wrapper around inline free_var)
        pub fn harness_free_var(v: Var);

        // Get bytecode size from a compiled program
        pub fn harness_get_program_bytecode_size(prog: *mut Program) -> c_uint;

        // Property functions
        pub fn db_add_propdef(
            oid: i32,
            pname: *const c_char,
            value: Var,
            owner: i32,
            flags: c_uint,
        ) -> c_int;

        pub fn db_find_property(oid: i32, name: *const c_char, value: *mut Var) -> PropHandle;

        pub fn db_set_property_value(h: PropHandle, value: Var);
    }

    /// Opaque verb handle
    #[repr(C)]
    pub struct VerbHandle {
        pub ptr: *mut std::ffi::c_void,
    }

    /// Property handle
    #[repr(C)]
    pub struct PropHandle {
        pub built_in: c_int,
        pub definer: i32,
        pub ptr: *mut std::ffi::c_void,
    }

    /// Opaque program structure
    #[repr(C)]
    pub struct Program {
        _private: [u8; 0],
    }
}

/// Error type for harness operations.
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("Failed to initialize database: {0}")]
    DbInitFailed(String),

    #[error("Failed to load database: {0}")]
    DbLoadFailed(String),

    #[error("Failed to initialize network layer: {0}")]
    NetworkInitFailed(String),

    #[error("Failed to create connection")]
    ConnectionFailed,

    #[error("Failed to queue input")]
    InputQueueFailed,

    #[error("Invalid database path: {0}")]
    InvalidPath(String),

    #[error("Harness already initialized")]
    AlreadyInitialized,

    #[error("Harness not initialized")]
    NotInitialized,

    #[error(
        "LambdaMOO sources not available. Run: ./crates/testing/lambdamoo-harness/setup-lambdamoo.sh"
    )]
    SourcesNotAvailable,
}

/// Global lock to ensure single-threaded access to LambdaMOO
/// (LambdaMOO is not thread-safe)
static HARNESS_LOCK: Mutex<()> = Mutex::new(());

/// The main LambdaMOO test harness.
///
/// Provides a safe Rust interface to initialize LambdaMOO with a database,
/// send commands, and capture output.
pub struct LambdaMooHarness {
    initialized: bool,
}

impl LambdaMooHarness {
    /// Initialize LambdaMOO with the given database file.
    ///
    /// # Arguments
    /// * `db_path` - Path to a LambdaMOO database file (e.g., Minimal.db)
    ///
    /// # Safety
    /// This function is not thread-safe. Only one harness can be active at a time.
    pub fn new(db_path: &Path) -> Result<Self, HarnessError> {
        let _lock = HARNESS_LOCK.lock().unwrap();

        // Convert path to C string
        let db_path_str = db_path
            .to_str()
            .ok_or_else(|| HarnessError::InvalidPath(format!("{:?}", db_path)))?;
        let db_path_cstring = CString::new(db_path_str)
            .map_err(|_| HarnessError::InvalidPath(db_path_str.to_string()))?;

        // LambdaMOO requires an output database path for checkpoints.
        // We use /tmp since we don't care about checkpoint output.
        let output_db_cstring = CString::new("/tmp/lambdamoo-harness-dump.db").unwrap();

        unsafe {
            // Initialize the harness (output capture, etc.)
            ffi::harness_init();

            // Build argc/argv for db_initialize
            // After server.c skips program name and options, db_initialize expects:
            //   argv[0] = input-db-file
            //   argv[1] = output-db-file
            let mut argv_ptrs: Vec<*mut c_char> = vec![
                db_path_cstring.as_ptr() as *mut c_char,
                output_db_cstring.as_ptr() as *mut c_char,
            ];
            let mut argc: c_int = 2;
            let mut argv_ptr = argv_ptrs.as_mut_ptr();

            // Initialize database subsystem
            if ffi::db_initialize(&mut argc, &mut argv_ptr) == 0 {
                ffi::harness_cleanup();
                return Err(HarnessError::DbInitFailed(
                    "db_initialize returned 0".to_string(),
                ));
            }

            // Initialize network layer (our harness)
            let mut desc: ffi::Var = std::mem::zeroed();
            if ffi::network_initialize(0, std::ptr::null_mut(), &mut desc) == 0 {
                ffi::harness_cleanup();
                return Err(HarnessError::NetworkInitFailed(
                    "network_initialize returned 0".to_string(),
                ));
            }

            // Register built-in functions
            ffi::register_bi_functions();

            // Load the database
            if ffi::db_load() == 0 {
                ffi::harness_cleanup();
                return Err(HarnessError::DbLoadFailed("db_load returned 0".to_string()));
            }

            // Load server options from $server_options
            ffi::load_server_options();
        }

        Ok(Self { initialized: true })
    }

    /// Create a new connection for a player object.
    ///
    /// Returns a connection handle that can be used to send commands.
    pub fn create_connection(&self, player_objid: i32) -> Result<Connection, HarnessError> {
        if !self.initialized {
            return Err(HarnessError::NotInitialized);
        }

        let conn_id = unsafe { ffi::harness_create_connection(player_objid) };
        if conn_id < 0 {
            return Err(HarnessError::ConnectionFailed);
        }

        Ok(Connection { id: conn_id })
    }

    /// Send a command and run until tasks complete, returning output.
    ///
    /// This queues the command, pumps the task loop, and returns captured output.
    pub fn execute_command(
        &self,
        conn: &Connection,
        command: &str,
    ) -> Result<String, HarnessError> {
        if !self.initialized {
            return Err(HarnessError::NotInitialized);
        }

        let command_cstring = CString::new(command).map_err(|_| HarnessError::InputQueueFailed)?;

        unsafe {
            // Clear any previous output
            ffi::harness_clear_output();

            // Queue the command
            if ffi::harness_queue_input(conn.id, command_cstring.as_ptr()) == 0 {
                return Err(HarnessError::InputQueueFailed);
            }

            // Process I/O (delivers queued input to server)
            ffi::network_process_io(0);

            // Run ready tasks until none remain
            // We pump multiple times to handle any forked/suspended tasks
            for _ in 0..100 {
                ffi::run_ready_tasks();
                // Small pump to handle any pending I/O
                if ffi::network_process_io(0) == 0 {
                    break;
                }
            }

            // Get captured output
            let mut len: usize = 0;
            let output_ptr = ffi::harness_get_output(&mut len);
            if len == 0 {
                return Ok(String::new());
            }

            let output = CStr::from_ptr(output_ptr).to_string_lossy().into_owned();
            Ok(output)
        }
    }

    /// Run tasks for the specified number of iterations.
    ///
    /// Useful for benchmarking or waiting for background tasks.
    pub fn pump_tasks(&self, iterations: usize) {
        if !self.initialized {
            return;
        }

        unsafe {
            for _ in 0..iterations {
                ffi::network_process_io(0);
                ffi::run_ready_tasks();
            }
        }
    }

    /// Get any pending output without clearing it.
    pub fn get_output(&self) -> String {
        if !self.initialized {
            return String::new();
        }

        unsafe {
            let mut len: usize = 0;
            let output_ptr = ffi::harness_get_output(&mut len);
            if len == 0 {
                return String::new();
            }

            CStr::from_ptr(output_ptr).to_string_lossy().into_owned()
        }
    }

    /// Clear the output buffer.
    pub fn clear_output(&self) {
        if self.initialized {
            unsafe {
                ffi::harness_clear_output();
            }
        }
    }
}

impl Drop for LambdaMooHarness {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                ffi::network_shutdown();
                ffi::db_shutdown();
                ffi::harness_cleanup();
            }
            self.initialized = false;
        }
    }
}

/// A connection to a player in the LambdaMOO harness.
pub struct Connection {
    id: c_int,
}

impl Connection {
    /// Get the connection ID.
    pub fn id(&self) -> i32 {
        self.id
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            ffi::harness_close_connection(self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_symbol_linkage() {
        // Verify we can reference the C symbols (don't call them yet)
        use crate::ffi::*;
        use std::os::raw::{c_char, c_int};
        let _db_init: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int =
            db_initialize;
        let _notify: unsafe extern "C" fn(c_int, *const c_char) = notify;
        let _harness_init: unsafe extern "C" fn() = harness_init;
        let _harness_get_output: unsafe extern "C" fn(*mut usize) -> *const c_char =
            harness_get_output;

        // If this compiles and links, the symbols are available
    }

    #[test]
    fn test_harness_basic() {
        use crate::ffi::*;

        unsafe {
            // Initialize the harness
            harness_init();

            // Get output (should be empty initially)
            let mut len: usize = 0;
            let _output = harness_get_output(&mut len);
            assert_eq!(len, 0);

            // Clear and verify
            harness_clear_output();
            let _output = harness_get_output(&mut len);
            assert_eq!(len, 0);

            // Cleanup
            harness_cleanup();
        }
    }
}
