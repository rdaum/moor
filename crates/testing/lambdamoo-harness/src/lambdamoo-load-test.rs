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

//! LambdaMOO load test - measures verb dispatch performance in embedded LambdaMOO.
//!
//! This is analogous to direct-scheduler-load-test.rs but runs against
//! the original LambdaMOO implementation for comparative benchmarking.
//! It measures the cost of verb dispatch by creating test objects with verbs,
//! then directly calling verbs (bypassing the command parser).

use clap::Parser;
use clap_derive::Parser;
use lambdamoo_harness::{LambdaMooHarness, ffi};
use std::ffi::CString;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tabled::{Table, Tabled};
use tracing::info;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Path to LambdaMOO database file")]
    db_path: PathBuf,

    #[arg(long, help = "Number of test objects to create", default_value = "1")]
    num_objects: usize,

    #[arg(
        long,
        help = "Number of verb iterations per invocation (inner loop)",
        default_value = "7000"
    )]
    num_verb_iterations: usize,

    #[arg(
        long,
        help = "Number of times to invoke the top-level workload verb per player",
        default_value = "200"
    )]
    num_invocations: usize,

    #[arg(long, help = "Min number of concurrent players", default_value = "1")]
    min_concurrency: usize,

    #[arg(long, help = "Max number of concurrent players", default_value = "32")]
    max_concurrency: usize,

    #[arg(long, help = "Base player object ID", default_value = "3")]
    player_id: i32,

    #[arg(
        long,
        help = "Opcode mode: measure raw interpreter speed without verb dispatch",
        default_value = "false"
    )]
    opcode_mode: bool,

    #[arg(
        long,
        help = "Number of loop iterations per invocation in opcode mode",
        default_value = "100000"
    )]
    loop_iterations: usize,
}

#[derive(Tabled)]
struct BenchmarkRow {
    #[tabled(rename = "Conc")]
    concurrency: usize,
    #[tabled(rename = "Tasks")]
    tasks: usize,
    #[tabled(rename = "Verb Calls")]
    verb_calls: usize,
    #[tabled(rename = "Per-Verb")]
    per_verb_call: String,
    #[tabled(rename = "Wall Time")]
    wall_time: String,
    #[tabled(rename = "Total Thru")]
    total_throughput: String,
    #[tabled(rename = "Per-Thread Thru")]
    per_thread_throughput: String,
}

// LambdaMOO constants
const VF_READ: u32 = 0o1;
const VF_EXEC: u32 = 0o4;
const VF_DEBUG: u32 = 0o10;
#[allow(dead_code)]
const ASPEC_NONE: i32 = 0;
const ASPEC_THIS: i32 = 2;
const PREP_NONE: i32 = -1;

/// Compile a MOO program from source lines
unsafe fn compile_verb(
    code_lines: &[&str],
) -> Result<*mut ffi::Program, Box<dyn std::error::Error>> {
    unsafe {
        let mut code_list = ffi::new_list(0);
        for line in code_lines {
            let code_cstr = CString::new(*line)?;
            let code_str = ffi::str_dup(code_cstr.as_ptr());
            let mut str_var: ffi::Var = std::mem::zeroed();
            str_var.v.str_val = code_str;
            str_var.var_type = ffi::TYPE_STR;
            code_list = ffi::listappend(code_list, str_var);
        }

        let mut errors: ffi::Var = std::mem::zeroed();
        let program = ffi::parse_list_as_program(code_list, &mut errors);
        ffi::harness_free_var(code_list);
        if program.is_null() {
            ffi::harness_free_var(errors);
            return Err("Failed to compile verb".into());
        }
        ffi::harness_free_var(errors);
        Ok(program)
    }
}

/// Set up for opcode mode - a single verb that does a tight loop of arithmetic ops.
/// Returns (player_ids, bytecode_size_per_invocation).
fn setup_opcode_environment(
    base_player_id: i32,
    loop_iterations: usize,
    concurrency: usize,
) -> Result<(Vec<i32>, usize), Box<dyn std::error::Error>> {
    info!(
        "Setting up opcode test environment with {} loop iterations per invocation, {} concurrent players...",
        loop_iterations, concurrency
    );

    let mut player_ids = Vec::with_capacity(concurrency);
    let mut bytecode_size: usize = 0;

    // Build the opcode_test verb code - tight loop doing arithmetic
    let opcode_code = [
        "x = 0;".to_string(),
        format!("for i in [1..{}]", loop_iterations),
        "x = x + i;".to_string(),
        "endfor".to_string(),
        "return x;".to_string(),
    ];
    let opcode_code_refs: Vec<&str> = opcode_code.iter().map(|s| s.as_str()).collect();

    unsafe {
        // Create player objects
        for p in 0..concurrency {
            let player_id = if p == 0 {
                base_player_id
            } else {
                let oid = ffi::db_create_object();
                if oid < 0 {
                    return Err(format!("Failed to create player {}", p + 1).into());
                }
                let name = CString::new(format!("TestPlayer{}", p + 1))?;
                ffi::db_set_object_name(oid, name.as_ptr());
                ffi::db_set_object_owner(oid, base_player_id);
                ffi::db_set_object_flag(oid, 0x10); // FLAG_PLAYER
                oid
            };

            // Add opcode_test verb to this player
            let verb_name = CString::new("opcode_test")?;
            let flags = VF_READ | VF_EXEC | VF_DEBUG;
            let verb_name_moo = ffi::str_dup(verb_name.as_ptr());
            let verb_idx = ffi::db_add_verb(
                player_id,
                verb_name_moo,
                base_player_id,
                flags,
                ASPEC_THIS,
                PREP_NONE,
                ASPEC_THIS,
            );
            if verb_idx < 0 {
                return Err(
                    format!("Failed to add opcode_test verb to player #{}", player_id).into(),
                );
            }

            let h = ffi::db_find_defined_verb(player_id, verb_name.as_ptr(), 0);
            if h.ptr.is_null() {
                return Err(
                    format!("Failed to find opcode_test verb on player #{}", player_id).into(),
                );
            }
            let program = compile_verb(&opcode_code_refs)?;

            // Get the actual bytecode size from the compiled program (only need to do this once)
            if p == 0 {
                bytecode_size = ffi::harness_get_program_bytecode_size(program) as usize;
                info!(
                    "Compiled opcode_test verb: {} bytecode bytes",
                    bytecode_size
                );
            }

            ffi::db_set_verb_program(h, program);

            player_ids.push(player_id);
        }
    }

    info!(
        "Created {} players with opcode_test verbs: {:?}",
        player_ids.len(),
        player_ids
    );

    // Verify verbs are callable
    unsafe {
        for &player_id in &player_ids {
            let verb_name = CString::new("opcode_test").unwrap();
            let h = ffi::db_find_callable_verb(player_id, verb_name.as_ptr());
            if h.ptr.is_null() {
                return Err(
                    format!("Verb opcode_test not callable on player #{}", player_id).into(),
                );
            }
        }
    }
    info!("Verified all opcode_test verbs are callable");

    Ok((player_ids, bytecode_size))
}

/// Set up test objects with verbs using direct database API.
/// Returns (test_objects, player_ids) where player_ids are the players to use for connections.
fn setup_test_environment(
    base_player_id: i32,
    num_objects: usize,
    num_verb_iterations: usize,
    concurrency: usize,
) -> Result<(Vec<i32>, Vec<i32>), Box<dyn std::error::Error>> {
    info!(
        "Setting up test environment with {} objects, {} concurrent players...",
        num_objects, concurrency
    );

    let mut test_objects = Vec::with_capacity(num_objects);
    let mut player_ids = Vec::with_capacity(concurrency);

    unsafe {
        // Create test objects with load_test verbs
        for i in 0..num_objects {
            let oid = ffi::db_create_object();
            if oid < 0 {
                return Err(format!("Failed to create test object {}", i + 1).into());
            }

            let name = CString::new(format!("TestObject{}", i + 1))?;
            ffi::db_set_object_name(oid, name.as_ptr());
            ffi::db_set_object_owner(oid, base_player_id);

            // Add load_test verb: flags = r+x+d, argspec = this none this
            let verb_name = CString::new("load_test")?;
            let flags = VF_READ | VF_EXEC | VF_DEBUG;
            let verb_name_moo = ffi::str_dup(verb_name.as_ptr());
            let verb_idx = ffi::db_add_verb(
                oid,
                verb_name_moo,
                base_player_id,
                flags,
                ASPEC_THIS,
                PREP_NONE,
                ASPEC_THIS,
            );
            if verb_idx < 0 {
                return Err(format!("Failed to add verb to test object {}", i + 1).into());
            }

            let h = ffi::db_find_defined_verb(oid, verb_name.as_ptr(), 0);
            if h.ptr.is_null() {
                return Err(format!("Failed to find verb on test object {}", i + 1).into());
            }
            let program = compile_verb(&["return 1;"])?;
            ffi::db_set_verb_program(h, program);

            test_objects.push(oid);
        }

        // Build the invoke_load_test verb code (shared by all players)
        let obj_list: String = test_objects
            .iter()
            .map(|oid| format!("#{}", oid))
            .collect::<Vec<_>>()
            .join(", ");

        let invoke_code = [
            format!("for i in [1..{}]", num_verb_iterations),
            format!("for o in ({{{}}})", obj_list),
            "if (o:load_test() != 1)".to_string(),
            "return 0;".to_string(),
            "endif".to_string(),
            "endfor".to_string(),
            "endfor".to_string(),
            "return 1;".to_string(),
        ];
        let invoke_code_refs: Vec<&str> = invoke_code.iter().map(|s| s.as_str()).collect();

        // Create player objects (or use base player for first one)
        for p in 0..concurrency {
            let player_id = if p == 0 {
                base_player_id
            } else {
                let oid = ffi::db_create_object();
                if oid < 0 {
                    return Err(format!("Failed to create player {}", p + 1).into());
                }
                let name = CString::new(format!("TestPlayer{}", p + 1))?;
                ffi::db_set_object_name(oid, name.as_ptr());
                ffi::db_set_object_owner(oid, base_player_id);
                // Set player flag (FLAG_PLAYER = 0x10)
                ffi::db_set_object_flag(oid, 0x10);
                oid
            };

            // Add invoke_load_test verb to this player
            let invoke_verb_name = CString::new("invoke_load_test")?;
            let flags = VF_READ | VF_EXEC | VF_DEBUG;
            let invoke_verb_name_moo = ffi::str_dup(invoke_verb_name.as_ptr());
            let verb_idx = ffi::db_add_verb(
                player_id,
                invoke_verb_name_moo,
                base_player_id,
                flags,
                ASPEC_THIS,
                PREP_NONE,
                ASPEC_THIS,
            );
            if verb_idx < 0 {
                return Err(format!(
                    "Failed to add invoke_load_test verb to player #{}",
                    player_id
                )
                .into());
            }

            let h = ffi::db_find_defined_verb(player_id, invoke_verb_name.as_ptr(), 0);
            if h.ptr.is_null() {
                return Err(format!(
                    "Failed to find invoke_load_test verb on player #{}",
                    player_id
                )
                .into());
            }
            let program = compile_verb(&invoke_code_refs)?;
            ffi::db_set_verb_program(h, program);

            player_ids.push(player_id);
        }
    }

    info!(
        "Created {} test objects with load_test verbs",
        test_objects.len()
    );
    info!(
        "Created {} players with invoke_load_test verbs: {:?}",
        player_ids.len(),
        player_ids
    );

    // Verify verbs are callable
    unsafe {
        for &oid in &test_objects {
            let verb_name = CString::new("load_test").unwrap();
            let h = ffi::db_find_callable_verb(oid, verb_name.as_ptr());
            if h.ptr.is_null() {
                return Err(format!("Verb load_test not callable on object #{}", oid).into());
            }
        }
        for &player_id in &player_ids {
            let invoke_verb_name = CString::new("invoke_load_test").unwrap();
            let h = ffi::db_find_callable_verb(player_id, invoke_verb_name.as_ptr());
            if h.ptr.is_null() {
                return Err(format!(
                    "Verb invoke_load_test not callable on player #{}",
                    player_id
                )
                .into());
            }
        }
    }
    info!("Verified all verbs are callable");

    Ok((test_objects, player_ids))
}

/// Result of a verb call
struct VerbCallResult {
    outcome: i32,      // 0 = OUTCOME_DONE, 1 = OUTCOME_ABORTED, 2 = OUTCOME_BLOCKED
    return_value: i32, // The integer return value (only valid if outcome == 0 and type == INT)
    return_type: i32,  // The type of the return value
}

/// Pre-allocated strings for verb calls (avoid allocation in hot path)
struct VerbCallContext {
    invoke_verb_name: CString,
    argstr: CString,
}

impl VerbCallContext {
    fn new() -> Self {
        Self {
            invoke_verb_name: CString::new("invoke_load_test").unwrap(),
            argstr: CString::new("").unwrap(),
        }
    }

    fn new_opcode() -> Self {
        Self {
            invoke_verb_name: CString::new("opcode_test").unwrap(),
            argstr: CString::new("").unwrap(),
        }
    }

    /// Call invoke_load_test verb on the player - this loops internally
    fn call_invoke(&self, player_id: i32) -> VerbCallResult {
        unsafe {
            let args = ffi::new_list(0);
            let mut result: ffi::Var = std::mem::zeroed();

            let outcome = ffi::run_server_task(
                player_id,
                player_id,
                self.invoke_verb_name.as_ptr(),
                args,
                self.argstr.as_ptr(),
                &mut result,
            );

            // Extract return value before freeing
            let return_type = result.var_type;
            let return_value = if return_type == ffi::TYPE_INT {
                result.v.num
            } else {
                0
            };

            // Free the result
            ffi::harness_free_var(result);

            VerbCallResult {
                outcome,
                return_value,
                return_type,
            }
        }
    }
}

/// Run workload at a specific concurrency level
fn run_workload(
    ctx: &VerbCallContext,
    player_ids: &[i32],
    num_invocations: usize,
    num_verb_iterations: usize,
    num_objects: usize,
) -> Result<(Duration, usize, usize), Box<dyn std::error::Error>> {
    let concurrency = player_ids.len();
    let total_invocations = num_invocations * concurrency;
    // Count actual verb dispatches: invoke_load_test calls + (iterations * objects) load_test calls
    // This matches mooR's counting: (invocations * iterations * concurrency) + total_invocations
    let total_verb_calls =
        (num_verb_iterations * num_objects * total_invocations) + total_invocations;

    let mut outcome_errors: usize = 0;
    let mut value_errors: usize = 0;

    let start_time = Instant::now();

    // Each round runs invoke_load_test on ALL players (serialized in LambdaMOO)
    for invocation in 0..num_invocations {
        for player_id in player_ids {
            let result = ctx.call_invoke(*player_id);
            if result.outcome != 0 {
                outcome_errors += 1;
                if outcome_errors <= 3 {
                    eprintln!(
                        "invoke_load_test returned unexpected outcome: {}",
                        result.outcome
                    );
                }
            } else if result.return_type != ffi::TYPE_INT || result.return_value != 1 {
                value_errors += 1;
                if value_errors <= 3 {
                    eprintln!(
                        "invoke_load_test returned unexpected value: type={}, value={}",
                        result.return_type, result.return_value
                    );
                }
            }
        }

        // Progress update every 10%
        if (invocation + 1) % (num_invocations / 10).max(1) == 0 {
            eprint!(
                "\r  {} Running workload... {}/{} rounds",
                ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'][(invocation / 10) % 10],
                invocation + 1,
                num_invocations
            );
            std::io::Write::flush(&mut std::io::stderr()).ok();
        }
    }

    let total_time = start_time.elapsed();
    let total_errors = outcome_errors + value_errors;

    if total_errors > 0 {
        return Err(format!("{} errors in {} verb calls", total_errors, total_verb_calls).into());
    }

    Ok((total_time, total_invocations, total_verb_calls))
}

/// Run opcode workload - measures raw interpreter throughput without verb dispatch
/// Returns (wall_time, total_invocations, total_bytecode_bytes, total_loop_iterations)
fn run_opcode_workload(
    ctx: &VerbCallContext,
    player_ids: &[i32],
    num_invocations: usize,
    loop_iterations_per_invocation: usize,
    bytecode_size_per_invocation: usize,
) -> Result<(Duration, usize, usize, usize), Box<dyn std::error::Error>> {
    let concurrency = player_ids.len();
    let total_invocations = num_invocations * concurrency;
    // Use actual bytecode size from compiled program
    let total_bytecode_bytes = bytecode_size_per_invocation * total_invocations;
    let total_loop_iterations = loop_iterations_per_invocation * total_invocations;

    // Expected return value: sum(1..N) = N*(N+1)/2, with 32-bit wrapping
    // LambdaMOO uses 32-bit signed integers that wrap on overflow
    let n = loop_iterations_per_invocation as i64;
    let expected_sum_64 = n * (n + 1) / 2;
    let expected_sum_wrapped = (expected_sum_64 as i32) as i64; // Wrap to i32 then back

    let mut outcome_errors: usize = 0;
    let mut value_errors: usize = 0;
    let mut first_value: Option<i32> = None;

    let start_time = Instant::now();

    for invocation in 0..num_invocations {
        for player_id in player_ids {
            let result = ctx.call_invoke(*player_id);
            if result.outcome != 0 {
                outcome_errors += 1;
                if outcome_errors <= 3 {
                    eprintln!(
                        "opcode_test returned unexpected outcome: {}",
                        result.outcome
                    );
                }
            } else if result.return_type != ffi::TYPE_INT {
                value_errors += 1;
                if value_errors <= 3 {
                    eprintln!(
                        "opcode_test returned non-integer type: {}",
                        result.return_type
                    );
                }
            } else {
                // Record first value for consistency check
                match first_value {
                    None => {
                        first_value = Some(result.return_value);
                        eprintln!(
                            "First return value: {} (expected wrapped: {})",
                            result.return_value, expected_sum_wrapped
                        );
                    }
                    Some(expected) if result.return_value != expected => {
                        value_errors += 1;
                        if value_errors <= 3 {
                            eprintln!(
                                "Inconsistent return value! First was {}, got {}",
                                expected, result.return_value
                            );
                        }
                    }
                    Some(_) => {}
                }
            }
        }

        // Progress update every 10%
        if (invocation + 1) % (num_invocations / 10).max(1) == 0 {
            eprint!(
                "\r  {} Running opcode workload... {}/{} rounds",
                ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'][(invocation / 10) % 10],
                invocation + 1,
                num_invocations
            );
            std::io::Write::flush(&mut std::io::stderr()).ok();
        }
    }

    let total_time = start_time.elapsed();

    if outcome_errors > 0 {
        return Err(format!(
            "{} task errors in {} invocations",
            outcome_errors, total_invocations
        )
        .into());
    }
    if value_errors > 0 {
        return Err(format!(
            "{} inconsistent return values in {} invocations. First value was {:?} for {} iterations.",
            value_errors, total_invocations, first_value, loop_iterations_per_invocation
        ).into());
    }

    Ok((
        total_time,
        total_invocations,
        total_bytecode_bytes,
        total_loop_iterations,
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Args = Args::parse();

    info!(
        "Initializing LambdaMOO harness with database: {}",
        args.db_path.display()
    );

    // Initialize LambdaMOO with the database
    let _harness = LambdaMooHarness::new(&args.db_path)?;

    if args.opcode_mode {
        // Opcode mode - measure raw interpreter throughput
        let (all_player_ids, bytecode_size) =
            setup_opcode_environment(args.player_id, args.loop_iterations, args.max_concurrency)?;

        info!(
            "Starting OPCODE test: {} to {} concurrent, {} invocations each, {} loop iterations, {} bytecode bytes per invocation",
            args.min_concurrency,
            args.max_concurrency,
            args.num_invocations,
            args.loop_iterations,
            bytecode_size
        );

        let ctx = VerbCallContext::new_opcode();

        // Warm up
        info!("Running warm-up...");
        for i in 0..5 {
            let result = ctx.call_invoke(all_player_ids[0]);
            if result.outcome != 0 {
                return Err(format!("Warm-up {} failed: outcome={}", i, result.outcome).into());
            }
        }

        #[derive(tabled::Tabled)]
        struct OpcodeRow {
            #[tabled(rename = "Conc")]
            concurrency: usize,
            #[tabled(rename = "Invocations")]
            invocations: usize,
            #[tabled(rename = "Loop Iters")]
            loop_iterations: String,
            #[tabled(rename = "Bytecode")]
            bytecode_bytes: String,
            #[tabled(rename = "Wall Time")]
            wall_time: String,
            #[tabled(rename = "Per-Byte")]
            per_byte: String,
            #[tabled(rename = "Byte Throughput")]
            byte_throughput: String,
            #[tabled(rename = "Per-Iter")]
            per_iteration: String,
            #[tabled(rename = "Iter Throughput")]
            iter_throughput: String,
        }

        let mut table_rows = vec![];

        let mut concurrency = args.min_concurrency as f32;
        loop {
            if concurrency > args.max_concurrency as f32 {
                break;
            }
            let num_concurrent = concurrency as usize;
            let player_ids = &all_player_ids[0..num_concurrent];

            eprint!(
                "  Running concurrency {} opcode workload...",
                num_concurrent
            );
            std::io::Write::flush(&mut std::io::stderr()).ok();

            let (total_time, total_invocations, total_bytecode_bytes, total_loop_iterations) =
                run_opcode_workload(
                    &ctx,
                    player_ids,
                    args.num_invocations,
                    args.loop_iterations,
                    bytecode_size,
                )?;

            // Bytecode-based metrics
            let per_byte =
                Duration::from_secs_f64(total_time.as_secs_f64() / total_bytecode_bytes as f64);
            let byte_throughput = total_bytecode_bytes as f64 / total_time.as_secs_f64();

            // Loop iteration metrics (work-equivalent comparison)
            let per_iteration =
                Duration::from_secs_f64(total_time.as_secs_f64() / total_loop_iterations as f64);
            let iter_throughput = total_loop_iterations as f64 / total_time.as_secs_f64();

            eprintln!(
                "\r  ✓ Concurrency {}: {:?} for {} bytes ({:.1}ns/byte, {:.1}M bytes/s) | {} iterations ({:.1}ns/iter, {:.1}M iter/s)",
                num_concurrent,
                total_time,
                total_bytecode_bytes,
                per_byte.as_nanos(),
                byte_throughput / 1_000_000.0,
                total_loop_iterations,
                per_iteration.as_nanos(),
                iter_throughput / 1_000_000.0
            );

            table_rows.push(OpcodeRow {
                concurrency: num_concurrent,
                invocations: total_invocations,
                loop_iterations: format!("{}", total_loop_iterations),
                bytecode_bytes: format!("{}", total_bytecode_bytes),
                wall_time: format!("{:.2?}", total_time),
                per_byte: format!("{:.1}ns", per_byte.as_nanos()),
                byte_throughput: format!("{:.1}M/s", byte_throughput / 1_000_000.0),
                per_iteration: format!("{:.1}ns", per_iteration.as_nanos()),
                iter_throughput: format!("{:.1}M/s", iter_throughput / 1_000_000.0),
            });

            eprint!("\x1B[2J\x1B[1;1H");
            eprintln!("{}", Table::new(&table_rows));

            let mut next_concurrency = concurrency * 1.25;
            if next_concurrency as usize <= concurrency as usize {
                next_concurrency = concurrency + 1.0;
            }
            concurrency = next_concurrency;
        }

        println!("\nLambdaMOO Opcode Throughput Test Complete");
        println!("Note: Measures raw interpreter loop speed without verb dispatch overhead.");
        println!("Bytecode column shows actual bytecode bytes from compiled program.");
    } else {
        // Verb dispatch mode (original behavior)
        let (_test_objects, all_player_ids) = setup_test_environment(
            args.player_id,
            args.num_objects,
            args.num_verb_iterations,
            args.max_concurrency,
        )?;

        info!(
            "Starting VERB DISPATCH test: {} to {} concurrent, {} invocations each, {} iterations x {} objects per invocation",
            args.min_concurrency,
            args.max_concurrency,
            args.num_invocations,
            args.num_verb_iterations,
            args.num_objects
        );

        let ctx = VerbCallContext::new();

        info!("Running warm-up...");
        for i in 0..5 {
            let result = ctx.call_invoke(all_player_ids[0]);
            if result.outcome != 0 {
                return Err(format!("Warm-up {} failed: outcome={}", i, result.outcome).into());
            }
        }

        let mut table_rows = vec![];

        let mut concurrency = args.min_concurrency as f32;
        loop {
            if concurrency > args.max_concurrency as f32 {
                break;
            }
            let num_concurrent = concurrency as usize;
            let player_ids = &all_player_ids[0..num_concurrent];

            eprint!("  Running concurrency {} workload...", num_concurrent);
            std::io::Write::flush(&mut std::io::stderr()).ok();

            let (total_time, total_invocations, total_verb_calls) = run_workload(
                &ctx,
                player_ids,
                args.num_invocations,
                args.num_verb_iterations,
                args.num_objects,
            )?;

            let per_verb_call =
                Duration::from_secs_f64(total_time.as_secs_f64() / total_verb_calls as f64);

            eprintln!(
                "\r  ✓ Concurrency {}: {:?} for {} verb calls ({:?}/verb, {:.0} verb/s)",
                num_concurrent,
                total_time,
                total_verb_calls,
                per_verb_call,
                total_verb_calls as f64 / total_time.as_secs_f64()
            );

            let throughput = total_verb_calls as f64 / total_time.as_secs_f64();
            let per_thread_throughput = throughput / num_concurrent as f64;
            table_rows.push(BenchmarkRow {
                concurrency: num_concurrent,
                tasks: total_invocations,
                verb_calls: total_verb_calls,
                per_verb_call: format!("{:.0?}", per_verb_call),
                wall_time: format!("{:.2?}", total_time),
                total_throughput: format!("{:.2}M/s", throughput / 1_000_000.0),
                per_thread_throughput: format!("{:.2}M/s", per_thread_throughput / 1_000_000.0),
            });

            eprint!("\x1B[2J\x1B[1;1H");
            eprintln!("{}", Table::new(&table_rows));

            let mut next_concurrency = concurrency * 1.25;
            if next_concurrency as usize <= concurrency as usize {
                next_concurrency = concurrency + 1.0;
            }
            concurrency = next_concurrency;
        }

        println!("\nLambdaMOO Verb Dispatch Test Complete");
        println!(
            "Note: LambdaMOO is single-threaded - all tasks serialize regardless of 'concurrency'"
        );
    }

    Ok(())
}
