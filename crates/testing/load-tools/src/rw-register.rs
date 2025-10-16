//! Elle rw-register consistency checker workload generator
//! Tests read-write register operations for serializability anomalies

use clap::Parser;
use clap_derive::Parser;
use edn_format::{Keyword, Value};
use moor_common::model::{CommitResult, WorldStateSource};
use moor_db::{Database, TxDB};
use moor_model_checker::elle_common::{self, EdnEvent};
use moor_var::{Obj, Symbol, v_int};
use rand::Rng;
use std::{path::PathBuf, sync::Arc, time::Instant};

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Database path", default_value = "test_db")]
    db_path: PathBuf,
    #[arg(long, default_value = "5")]
    num_registers: usize,
    #[arg(long, default_value = "20")]
    num_concurrent_workloads: usize,
    #[arg(long, default_value = "1000")]
    num_workload_iterations: usize,
    #[arg(long, default_value = "workload.edn")]
    output_file: PathBuf,
}

fn workload_thread(
    db: Arc<TxDB>,
    obj: Obj,
    register_symbols: Vec<Symbol>,
    process_id: usize,
    num_iterations: usize,
) -> Result<Vec<EdnEvent>, eyre::Error> {
    let mut rng = rand::thread_rng();
    let mut events = Vec::new();
    let mut skipped_ops = 0;
    const MAX_RETRIES: usize = 100;

    for iteration in 0..num_iterations {
        if iteration > 0 && iteration % 100 == 0 {
            println!(
                "Thread {} progress: {}/{} iterations, {} skipped",
                process_id, iteration, num_iterations, skipped_ops
            );
        }

        let register_idx = rng.gen_range(0..register_symbols.len());
        let register_sym = register_symbols[register_idx];
        let is_write = rng.gen_bool(0.5); // 50% reads, 50% writes

        if is_write {
            let value_to_write = (process_id * 10000 + iteration) as i64;
            let mut retry_count = 0;

            loop {
                if retry_count >= MAX_RETRIES {
                    skipped_ops += 1;
                    break;
                }
                retry_count += 1;

                let start = Instant::now();
                let mut tx = db.new_world_state()?;
                tx.update_property(&obj, &obj, register_sym, &v_int(value_to_write))?;

                match tx.commit()? {
                    CommitResult::Success { .. } => {
                        // Format: [[:w register-id value]]
                        let mop = Value::Vector(vec![
                            Value::Keyword(Keyword::from_name("w")),
                            Value::Integer(register_idx as i64),
                            Value::Integer(value_to_write),
                        ]);
                        let value_vec = Value::Vector(vec![mop]);
                        events.push(EdnEvent::invoke(
                            start,
                            process_id,
                            "txn".to_string(),
                            value_vec.clone(),
                        ));
                        events.push(EdnEvent::ok(
                            Instant::now(),
                            process_id,
                            "txn".to_string(),
                            value_vec,
                        ));
                        break;
                    }
                    CommitResult::ConflictRetry => continue,
                }
            }
        } else {
            // Read
            let mut retry_count = 0;

            loop {
                if retry_count >= MAX_RETRIES {
                    skipped_ops += 1;
                    break;
                }
                retry_count += 1;

                let start = Instant::now();
                let tx = db.new_world_state()?;
                let value = tx.retrieve_property(&obj, &obj, register_sym)?;

                match tx.commit()? {
                    CommitResult::Success { .. } => {
                        let int_value = value.as_integer().unwrap_or(0);
                        // Format: [[:r register-id value]]
                        let mop = Value::Vector(vec![
                            Value::Keyword(Keyword::from_name("r")),
                            Value::Integer(register_idx as i64),
                            Value::Integer(int_value),
                        ]);
                        let value_vec = Value::Vector(vec![mop]);
                        events.push(EdnEvent::invoke(
                            start,
                            process_id,
                            "txn".to_string(),
                            value_vec.clone(),
                        ));
                        events.push(EdnEvent::ok(
                            Instant::now(),
                            process_id,
                            "txn".to_string(),
                            value_vec,
                        ));
                        break;
                    }
                    CommitResult::ConflictRetry => continue,
                }
            }
        }
    }

    if skipped_ops > 0 {
        eprintln!(
            "Thread {} skipped {} operations due to retry limit",
            process_id, skipped_ops
        );
    }

    println!(
        "Thread {} completed with {} events",
        process_id,
        events.len()
    );
    Ok(events)
}

fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
    let args: Args = Args::parse();

    println!("RW-Register Elle workload");
    println!("Configuration:");
    println!("  Registers: {}", args.num_registers);
    println!("  Concurrent workloads: {}", args.num_concurrent_workloads);
    println!(
        "  Iterations per workload: {}",
        args.num_workload_iterations
    );
    println!("  Output file: {}", args.output_file.display());
    println!();

    // Create temporary directory for database if using default path
    let temp_dir = if args.db_path == PathBuf::from("test_db") {
        Some(tempfile::tempdir()?)
    } else {
        None
    };

    let db_path = if let Some(ref temp_dir) = temp_dir {
        temp_dir.path().join("test_db")
    } else {
        args.db_path.clone()
    };

    let (db, _) = TxDB::open(Some(&db_path), Default::default());
    let db = Arc::new(db);

    // Setup database - initialize all registers to 0
    let (obj, register_symbols) =
        elle_common::setup_test_database(&db, args.num_registers, "register", |_| v_int(0))?;

    // Run workloads
    let mut all_events = elle_common::run_concurrent_workloads(
        db,
        obj,
        register_symbols,
        args.num_concurrent_workloads,
        args.num_workload_iterations,
        workload_thread,
    )?;

    // Sort by timestamp
    all_events.sort_by_key(|e| e.timestamp);

    println!("Collected {} events", all_events.len());

    // Write EDN output
    elle_common::write_edn_history(&all_events, &args.output_file)?;

    println!("Wrote workload to {}", args.output_file.display());

    Ok(())
}
