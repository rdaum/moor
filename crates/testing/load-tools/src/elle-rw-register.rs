// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
//

//! Elle rw-register consistency checker workload generator
//! Tests read-write register operations for serializability anomalies

use clap::Parser;
use clap_derive::Parser;
use edn_format::{Keyword, Value};
use moor_common::model::{CommitResult, WorldStateSource};
use moor_db::TxDB;
use moor_model_checker::elle_common::{self, EdnEvent, EventType};
use moor_var::{Obj, Symbol, v_int};
use rand::Rng;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

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
    let mut rng = rand::rng();
    let mut events = Vec::new();
    let mut skipped_ops = 0;
    const MAX_RETRIES: usize = 100;

    for iteration in 0..num_iterations {
        if iteration > 0 && iteration % 100 == 0 {
            println!(
                "Thread {process_id} progress: {iteration}/{num_iterations} iterations, {skipped_ops} skipped"
            );
        }

        let register_idx = rng.random_range(0..register_symbols.len());
        let register_sym = register_symbols[register_idx];
        let is_write = rng.random_bool(0.5); // 50% reads, 50% writes

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
        eprintln!("Thread {process_id} skipped {skipped_ops} operations due to retry limit");
    }

    println!(
        "Thread {} completed with {} events",
        process_id,
        events.len()
    );
    Ok(events)
}

fn calculate_percentile(sorted_durations: &[Duration], percentile: f64) -> Duration {
    if sorted_durations.is_empty() {
        return Duration::ZERO;
    }
    let index = ((sorted_durations.len() as f64 - 1.0) * percentile).round() as usize;
    sorted_durations[index]
}

fn is_read_operation(event: &EdnEvent) -> bool {
    // Extract operation type from the value
    // Format: [[:r register-id value]] or [[:w register-id value]]
    if let Value::Vector(ops) = &event.value
        && let Some(Value::Vector(mop)) = ops.first()
        && let Some(Value::Keyword(op_type)) = mop.first()
    {
        return op_type.name() == "r";
    }
    false
}

fn print_performance_metrics(events: &[EdnEvent], total_duration: Duration) {
    // Pair up invoke/ok events to calculate operation durations
    let mut read_durations = Vec::new();
    let mut write_durations = Vec::new();
    let mut pending_ops: BTreeMap<(usize, String), Instant> = BTreeMap::new();

    for event in events.iter() {
        match event.event_type {
            EventType::Invoke => {
                // Use process_id and value as key to match invoke with ok
                let key = (event.process_id, format!("{:?}", event.value));
                pending_ops.insert(key, event.timestamp);
            }
            EventType::Ok => {
                let key = (event.process_id, format!("{:?}", event.value));
                if let Some(start_time) = pending_ops.remove(&key) {
                    let duration = event.timestamp.duration_since(start_time);
                    if is_read_operation(event) {
                        read_durations.push(duration);
                    } else {
                        write_durations.push(duration);
                    }
                }
            }
            EventType::Fail => {
                // Remove from pending on failure
                let key = (event.process_id, format!("{:?}", event.value));
                pending_ops.remove(&key);
            }
        }
    }

    // Sort for percentile calculations
    read_durations.sort();
    write_durations.sort();

    let total_ops = read_durations.len() + write_durations.len();
    let throughput = total_ops as f64 / total_duration.as_secs_f64();

    println!("\n════════════════════════════════════════════════════════════");
    println!("Performance Metrics");
    println!("════════════════════════════════════════════════════════════");
    println!("\nOverall:");
    println!("  Total operations:     {total_ops}");
    println!(
        "  Total duration:       {:.2}s",
        total_duration.as_secs_f64()
    );
    println!("  Throughput:           {throughput:.2} ops/sec");

    if !read_durations.is_empty() {
        let read_mean = read_durations.iter().sum::<Duration>().as_micros() as f64
            / read_durations.len() as f64;
        println!("\nRead Operations ({} total):", read_durations.len());
        println!("  Mean latency:         {:.2}ms", read_mean / 1000.0);
        println!(
            "  Median (p50):         {:.2}ms",
            calculate_percentile(&read_durations, 0.50).as_micros() as f64 / 1000.0
        );
        println!(
            "  p95:                  {:.2}ms",
            calculate_percentile(&read_durations, 0.95).as_micros() as f64 / 1000.0
        );
        println!(
            "  p99:                  {:.2}ms",
            calculate_percentile(&read_durations, 0.99).as_micros() as f64 / 1000.0
        );
        println!(
            "  Max:                  {:.2}ms",
            read_durations.last().unwrap().as_micros() as f64 / 1000.0
        );
        println!(
            "  Min:                  {:.2}ms",
            read_durations.first().unwrap().as_micros() as f64 / 1000.0
        );
    }

    if !write_durations.is_empty() {
        let write_mean = write_durations.iter().sum::<Duration>().as_micros() as f64
            / write_durations.len() as f64;
        println!("\nWrite Operations ({} total):", write_durations.len());
        println!("  Mean latency:         {:.2}ms", write_mean / 1000.0);
        println!(
            "  Median (p50):         {:.2}ms",
            calculate_percentile(&write_durations, 0.50).as_micros() as f64 / 1000.0
        );
        println!(
            "  p95:                  {:.2}ms",
            calculate_percentile(&write_durations, 0.95).as_micros() as f64 / 1000.0
        );
        println!(
            "  p99:                  {:.2}ms",
            calculate_percentile(&write_durations, 0.99).as_micros() as f64 / 1000.0
        );
        println!(
            "  Max:                  {:.2}ms",
            write_durations.last().unwrap().as_micros() as f64 / 1000.0
        );
        println!(
            "  Min:                  {:.2}ms",
            write_durations.first().unwrap().as_micros() as f64 / 1000.0
        );
    }

    println!("\n════════════════════════════════════════════════════════════\n");
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

    let workload_start = Instant::now();

    // Run workloads
    let mut all_events = elle_common::run_concurrent_workloads(
        db,
        obj,
        register_symbols,
        args.num_concurrent_workloads,
        args.num_workload_iterations,
        workload_thread,
    )?;

    let total_duration = workload_start.elapsed();

    // Sort by timestamp
    all_events.sort_by_key(|e| e.timestamp);

    println!("Collected {} events", all_events.len());

    // Print performance metrics
    print_performance_metrics(&all_events, total_duration);

    // Write EDN output
    elle_common::write_edn_history(&all_events, &args.output_file)?;

    println!("Wrote workload to {}", args.output_file.display());

    Ok(())
}
