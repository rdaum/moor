//! VM opcode dispatch micro-benchmarks using the bench-utils framework
//! Measures CPU-level performance characteristics (IPC, frontend stalls, branch prediction)

use moor_bench_utils::{BenchContext, black_box};
use moor_common::{
    model::{CommitResult, ObjectKind, VerbArgsSpec, VerbFlag, WorldState, WorldStateSource},
    tasks::{AbortLimitReason, NoopClientSession, Session},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_db::{DatabaseConfig, TxDB};
use moor_kernel::{
    config::FeaturesConfig,
    testing::vm_test_utils::setup_task_context,
    vm::{VMHostResponse, builtins::BuiltinRegistry, vm_host::VmHost},
};
use moor_var::{List, NOTHING, SYSTEM_OBJECT, Symbol, v_obj};
use std::{sync::Arc, time::Duration};

const MAX_TICKS: usize = 100_000_000;
const CHUNK_SIZE: usize = 1; // Each "chunk" is one full execution run (100M opcodes)

fn create_db() -> TxDB {
    let (ws_source, _) = TxDB::open(None, DatabaseConfig::default());
    let mut tx = ws_source.new_world_state().unwrap();
    let _sysobj = tx
        .create_object(
            &SYSTEM_OBJECT,
            &NOTHING,
            &SYSTEM_OBJECT,
            BitEnum::all(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    ws_source
}

fn prepare_call_verb(
    world_state: &mut dyn WorldState,
    verb_name: &str,
    args: List,
    max_ticks: usize,
) -> VmHost {
    let mut vm_host = VmHost::new(0, 20, max_ticks, Duration::from_secs(1000));

    let verb_name = Symbol::mk(verb_name);
    let (program, verbdef) = world_state
        .find_method_verb_on(&SYSTEM_OBJECT, &SYSTEM_OBJECT, verb_name)
        .unwrap();
    vm_host.start_call_method_verb(
        0,
        SYSTEM_OBJECT,
        verbdef,
        verb_name,
        v_obj(SYSTEM_OBJECT),
        SYSTEM_OBJECT,
        args,
        v_obj(SYSTEM_OBJECT),
        "".to_string(),
        program,
    );
    vm_host
}

fn prepare_vm_execution(
    ws_source: &dyn WorldStateSource,
    program: &str,
    max_ticks: usize,
) -> VmHost {
    let program = compile(program, CompileOptions::default()).unwrap();
    let mut tx = ws_source.new_world_state().unwrap();
    tx.add_verb(
        &SYSTEM_OBJECT,
        &SYSTEM_OBJECT,
        vec![Symbol::mk("test")],
        &SYSTEM_OBJECT,
        VerbFlag::rxd(),
        VerbArgsSpec::this_none_this(),
        moor_var::program::ProgramType::MooR(program),
    )
    .unwrap();
    let vm_host = prepare_call_verb(tx.as_mut(), "test", List::mk_list(&[]), max_ticks);
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    vm_host
}

/// Execute VM until tick limit reached, return number of ticks executed
fn execute_until_ticks(session: Arc<dyn Session>, vm_host: &mut VmHost) -> usize {
    vm_host.reset_ticks();
    vm_host.reset_time();

    let config = FeaturesConfig::default();

    loop {
        match vm_host.exec_interpreter(0, session.as_ref(), &BuiltinRegistry::new(), &config) {
            VMHostResponse::ContinueOk => continue,
            VMHostResponse::AbortLimit(AbortLimitReason::Ticks(t)) => return t,
            _ => panic!("Unexpected VM response"),
        }
    }
}

/// Benchmark context for dispatch micro-tests
/// Pre-creates the VM and DB infrastructure once, then reuses it for each sample
struct DispatchContext {
    db: TxDB,
    vm_host: VmHost,
    session: Arc<dyn Session>,
}

impl BenchContext for DispatchContext {
    fn prepare(_num_chunks: usize) -> Self {
        // We use a default program here; actual contexts override this
        let db = create_db();
        let vm_host = prepare_vm_execution(&db, "while (1) 1; endwhile", MAX_TICKS);
        let tx = db.new_world_state().unwrap();
        let session = Arc::new(NoopClientSession::new());
        let _tx_guard = setup_task_context(tx);
        std::mem::forget(_tx_guard);

        DispatchContext {
            db,
            vm_host,
            session,
        }
    }

    fn chunk_size() -> Option<usize> {
        Some(CHUNK_SIZE)
    }

    fn operations_per_chunk() -> Option<u64> {
        Some(MAX_TICKS as u64)
    }
}

impl DispatchContext {
    /// Create a context with a specific program
    fn with_program(program: &str) -> Self {
        let db = create_db();
        let vm_host = prepare_vm_execution(&db, program, MAX_TICKS);
        let session = Arc::new(NoopClientSession::new());

        DispatchContext {
            db,
            vm_host,
            session,
        }
    }
}

/// Dispatch constant discard: pure dispatch overhead with minimal work
fn dispatch_constant_discard(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    let tx = ctx.db.new_world_state().unwrap();
    {
        let _tx_guard = setup_task_context(tx);
        let _ = black_box(execute_until_ticks(ctx.session.clone(), &mut ctx.vm_host));
    }
}

/// Dispatch push/pop: measures stack operation dispatch
fn dispatch_push_pop(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    let tx = ctx.db.new_world_state().unwrap();
    {
        let _tx_guard = setup_task_context(tx);
        let _ = black_box(execute_until_ticks(ctx.session.clone(), &mut ctx.vm_host));
    }
}

/// Dispatch simple add: dispatch + one binary operation
fn dispatch_simple_add(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    let tx = ctx.db.new_world_state().unwrap();
    {
        let _tx_guard = setup_task_context(tx);
        let _ = black_box(execute_until_ticks(ctx.session.clone(), &mut ctx.vm_host));
    }
}

/// Dispatch comparison: dispatch + comparison operation
fn dispatch_comparison(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    let tx = ctx.db.new_world_state().unwrap();
    {
        let _tx_guard = setup_task_context(tx);
        let _ = black_box(execute_until_ticks(ctx.session.clone(), &mut ctx.vm_host));
    }
}

pub fn main() {
    use moor_bench_utils::{generate_session_summary, op_bench_with_factory_filtered};
    use std::env;

    #[cfg(target_os = "linux")]
    {
        use moor_bench_utils::perf_event::{Builder, events::Hardware};
        if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
            eprintln!(
                "⚠️  Perf events are not available on this system (insufficient permissions or kernel support)."
            );
            eprintln!("   Continuing with timing-only benchmarks (performance counters disabled).");
            eprintln!();
        }
    }

    let args: Vec<String> = env::args().collect();
    let filter = if let Some(separator_pos) = args.iter().position(|arg| arg == "--") {
        args.get(separator_pos + 1).map(|s| s.as_str())
    } else {
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--") && !args[0].contains(arg.as_str()))
            .map(|s| s.as_str())
    };

    if let Some(f) = filter {
        eprintln!("Running benchmarks matching filter: '{f}'");
        eprintln!("Available filters: all, dispatch, or any benchmark name substring");
        eprintln!();
    }

    // Benchmark 1: dispatch_constant_discard
    op_bench_with_factory_filtered(
        "dispatch_constant_discard",
        "dispatch",
        |ctx: &mut DispatchContext, _chunk_size, _chunk_num| dispatch_constant_discard(ctx, 1, 0),
        &|| DispatchContext::with_program("while(1) 1; endwhile"),
        filter,
    );

    // Benchmark 2: dispatch_push_pop
    op_bench_with_factory_filtered(
        "dispatch_push_pop",
        "dispatch",
        |ctx: &mut DispatchContext, _chunk_size, _chunk_num| dispatch_push_pop(ctx, 1, 0),
        &|| DispatchContext::with_program("i=0; while(1) i; endwhile"),
        filter,
    );

    // Benchmark 3: dispatch_simple_add
    op_bench_with_factory_filtered(
        "dispatch_simple_add",
        "dispatch",
        |ctx: &mut DispatchContext, _chunk_size, _chunk_num| dispatch_simple_add(ctx, 1, 0),
        &|| DispatchContext::with_program("while(1) 1 + 1; endwhile"),
        filter,
    );

    // Benchmark 4: dispatch_comparison
    op_bench_with_factory_filtered(
        "dispatch_comparison",
        "dispatch",
        |ctx: &mut DispatchContext, _chunk_size, _chunk_num| dispatch_comparison(ctx, 1, 0),
        &|| DispatchContext::with_program("while(1) 1 == 1; endwhile"),
        filter,
    );

    if filter.is_some() {
        eprintln!("\nBenchmark filtering complete.");
    }

    generate_session_summary();
}
