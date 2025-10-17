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
//

//! Direct database list-append workload for Elle consistency testing.
//! Bypasses the scheduler and tests moor_db directly with concurrent threads.
//! Outputs EDN format for elle-cli analysis.

use clap::Parser;
use clap_derive::Parser;
use edn_format::{Keyword, Value};
use moor_common::model::{ObjAttrs, ObjectKind, WorldStateSource};
use moor_db::{Database, TxDB};
use moor_var::{Obj, Symbol, v_int, v_list};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Database path", default_value = "test_db")]
    db_path: PathBuf,

    #[arg(
        long,
        value_name = "num-props",
        help = "Number of properties to use in the workload",
        default_value = "5"
    )]
    num_props: usize,

    #[arg(
        long,
        value_name = "num-concurrent-workloads",
        help = "Number of concurrent workloads to run",
        default_value = "20"
    )]
    num_concurrent_workloads: usize,

    #[arg(
        long,
        value_name = "num-workload-iterations",
        help = "Number of iterations per workload",
        default_value = "1000"
    )]
    num_workload_iterations: usize,

    #[arg(
        long,
        value_name = "output-file",
        help = "File to write the workload to",
        default_value = "workload.edn"
    )]
    output_file: PathBuf,
}

#[derive(Debug, Clone)]
enum WorkItem {
    Append(usize, Vec<(usize, i64)>),
    Read(usize, Vec<(usize, Vec<i64>)>),
    AppendEnd(usize, Vec<(usize, i64)>),
    ReadEnd(usize, Vec<(usize, Vec<i64>)>),
}

fn setup_database(db: &TxDB, num_props: usize) -> Result<(Obj, Vec<Symbol>), eyre::Error> {
    let mut loader = db.loader_client()?;

    // Create test object
    let obj_attrs = ObjAttrs::default();
    let obj = loader.create_object(ObjectKind::NextObjid, &obj_attrs)?;

    // Create properties for list-append workload
    let mut prop_symbols = vec![];
    for i in 0..num_props {
        let prop_name = format!("prop_{}", i);
        let prop_sym = Symbol::mk(&prop_name);
        loader.define_property(
            &obj,
            &obj,
            prop_sym,
            &obj,
            Default::default(),
            Some(v_list(&[])),
        )?;
        prop_symbols.push(prop_sym);
    }

    match loader.commit()? {
        moor_common::model::CommitResult::Success { .. } => Ok((obj, prop_symbols)),
        moor_common::model::CommitResult::ConflictRetry => {
            Err(eyre::eyre!("Conflict during setup"))
        }
    }
}

fn workload_thread(
    db: Arc<TxDB>,
    obj: Obj,
    prop_symbols: Vec<Symbol>,
    process_id: usize,
    num_iterations: usize,
) -> Result<Vec<(Instant, WorkItem)>, eyre::Error> {
    let mut workload = vec![];
    let mut counter: i64 = (process_id as i64) * 1_000_000; // Each thread gets its own range
    let mut total_retries = 0;
    let mut skipped_ops = 0;

    for iteration in 0..num_iterations {
        // Print progress every 100 iterations
        if iteration > 0 && iteration % 100 == 0 {
            println!(
                "Thread {} progress: {}/{} iterations, {} total retries, {} skipped",
                process_id, iteration, num_iterations, total_retries, skipped_ops
            );
        }

        // Pick random property and operation type once per iteration
        let prop_idx = rand::random::<usize>() % prop_symbols.len();
        let prop_sym = prop_symbols[prop_idx];
        let is_read = rand::random::<bool>();

        // Retry loop for the same operation on conflict (with max retries to avoid infinite loops)
        let max_retries = 100;
        let mut retry_count = 0;
        'retry: loop {
            if retry_count >= max_retries {
                // Too many retries, skip this operation
                skipped_ops += 1;
                break 'retry;
            }
            if retry_count > 0 {
                total_retries += 1;
            }
            retry_count += 1;

            if is_read {
                // Read workload
                let start = Instant::now();

                let tx = db.new_world_state()?;
                let value = tx.retrieve_property(&obj, &obj, prop_sym)?;

                match tx.commit()? {
                    moor_common::model::CommitResult::Success { .. } => {
                        let values = if let Some(list) = value.as_list() {
                            list.iter().filter_map(|v| v.as_integer()).collect()
                        } else {
                            vec![]
                        };

                        workload.push((
                            start,
                            WorkItem::Read(process_id, vec![(prop_idx, values.clone())]),
                        ));
                        workload.push((
                            Instant::now(),
                            WorkItem::ReadEnd(process_id, vec![(prop_idx, values)]),
                        ));
                        break 'retry; // Success, move to next iteration
                    }
                    moor_common::model::CommitResult::ConflictRetry => {
                        // Retry the same read
                        continue 'retry;
                    }
                }
            } else {
                // Append workload - use unique counter-based values per thread
                let start = Instant::now();

                let mut tx = db.new_world_state()?;
                let value = tx.retrieve_property(&obj, &obj, prop_sym)?;

                // Generate a few unique values from this thread's range
                let num_values = (rand::random::<usize>() % 10) + 1;
                let mut new_values_to_append = Vec::new();
                for _ in 0..num_values {
                    counter += 1;
                    new_values_to_append.push(counter);
                }

                // Append to list
                let new_list = if let Some(list) = value.as_list() {
                    let mut list_values: Vec<_> = list.iter().collect();
                    for val in &new_values_to_append {
                        list_values.push(v_int(*val));
                    }
                    v_list(&list_values)
                } else {
                    v_list(
                        &new_values_to_append
                            .iter()
                            .map(|v| v_int(*v))
                            .collect::<Vec<_>>(),
                    )
                };

                tx.update_property(&obj, &obj, prop_sym, &new_list)?;

                match tx.commit()? {
                    moor_common::model::CommitResult::Success { .. } => {
                        workload.push((
                            start,
                            WorkItem::Append(
                                process_id,
                                new_values_to_append
                                    .iter()
                                    .map(|v| (prop_idx, *v))
                                    .collect(),
                            ),
                        ));
                        workload.push((
                            Instant::now(),
                            WorkItem::AppendEnd(
                                process_id,
                                new_values_to_append
                                    .iter()
                                    .map(|v| (prop_idx, *v))
                                    .collect(),
                            ),
                        ));
                        break 'retry; // Success, move to next iteration
                    }
                    moor_common::model::CommitResult::ConflictRetry => {
                        // Retry on conflict with the same values
                        counter -= num_values as i64;
                        continue 'retry;
                    }
                }
            }
        }
    }

    println!(
        "Thread {} completed: {} operations, {} total retries, {} skipped",
        process_id,
        workload.len() / 2,
        total_retries,
        skipped_ops
    );

    Ok(workload)
}

fn calculate_percentile(sorted_durations: &[Duration], percentile: f64) -> Duration {
    if sorted_durations.is_empty() {
        return Duration::ZERO;
    }
    let index = ((sorted_durations.len() as f64 - 1.0) * percentile).round() as usize;
    sorted_durations[index]
}

fn print_performance_metrics(workload_results: &[(Instant, WorkItem)], total_duration: Duration) {
    // Pair up invoke/ok events to calculate operation durations
    let mut read_durations = Vec::new();
    let mut append_durations = Vec::new();
    let mut pending_reads: BTreeMap<(usize, usize), Instant> = BTreeMap::new(); // (process_id, index) -> start_time
    let mut pending_appends: BTreeMap<(usize, usize), Instant> = BTreeMap::new();

    for (timestamp, item) in workload_results.iter() {
        match item {
            WorkItem::Read(process_id, ops) => {
                let key = (*process_id, ops.len());
                pending_reads.insert(key, *timestamp);
            }
            WorkItem::Append(process_id, ops) => {
                let key = (*process_id, ops.len());
                pending_appends.insert(key, *timestamp);
            }
            WorkItem::ReadEnd(process_id, ops) => {
                let key = (*process_id, ops.len());
                if let Some(start_time) = pending_reads.remove(&key) {
                    read_durations.push(timestamp.duration_since(start_time));
                }
            }
            WorkItem::AppendEnd(process_id, ops) => {
                let key = (*process_id, ops.len());
                if let Some(start_time) = pending_appends.remove(&key) {
                    append_durations.push(timestamp.duration_since(start_time));
                }
            }
        }
    }

    // Sort for percentile calculations
    read_durations.sort();
    append_durations.sort();

    let total_ops = read_durations.len() + append_durations.len();
    let throughput = total_ops as f64 / total_duration.as_secs_f64();

    println!("\n════════════════════════════════════════════════════════════");
    println!("Performance Metrics");
    println!("════════════════════════════════════════════════════════════");
    println!("\nOverall:");
    println!("  Total operations:     {}", total_ops);
    println!(
        "  Total duration:       {:.2}s",
        total_duration.as_secs_f64()
    );
    println!("  Throughput:           {:.2} ops/sec", throughput);

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

    if !append_durations.is_empty() {
        let append_mean = append_durations.iter().sum::<Duration>().as_micros() as f64
            / append_durations.len() as f64;
        println!("\nAppend Operations ({} total):", append_durations.len());
        println!("  Mean latency:         {:.2}ms", append_mean / 1000.0);
        println!(
            "  Median (p50):         {:.2}ms",
            calculate_percentile(&append_durations, 0.50).as_micros() as f64 / 1000.0
        );
        println!(
            "  p95:                  {:.2}ms",
            calculate_percentile(&append_durations, 0.95).as_micros() as f64 / 1000.0
        );
        println!(
            "  p99:                  {:.2}ms",
            calculate_percentile(&append_durations, 0.99).as_micros() as f64 / 1000.0
        );
        println!(
            "  Max:                  {:.2}ms",
            append_durations.last().unwrap().as_micros() as f64 / 1000.0
        );
        println!(
            "  Min:                  {:.2}ms",
            append_durations.first().unwrap().as_micros() as f64 / 1000.0
        );
    }

    println!("\n════════════════════════════════════════════════════════════\n");
}

fn write_edn_output(
    workload_results: &[(Instant, WorkItem)],
    output_path: &PathBuf,
) -> Result<(), eyre::Error> {
    let mut output_document = String::new();

    for (i, workload) in workload_results.iter().enumerate() {
        let mut map = BTreeMap::new();
        match &workload.1 {
            WorkItem::Append(process, appends) => {
                if appends.is_empty() {
                    continue;
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("process")),
                    Value::Integer(*process as i64),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("index")),
                    Value::Integer(i as i64),
                );
                let mut append_ops = vec![];
                for (property, value) in appends {
                    append_ops.push(Value::Vector(vec![
                        Value::Keyword(Keyword::from_name("append")),
                        Value::Integer(*property as i64),
                        Value::Integer(*value),
                    ]));
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("value")),
                    Value::Vector(append_ops),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("type")),
                    Value::Keyword(Keyword::from_name("invoke")),
                );
            }
            WorkItem::Read(process, reads) => {
                if reads.is_empty() {
                    continue;
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("process")),
                    Value::Integer(*process as i64),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("index")),
                    Value::Integer(i as i64),
                );
                let mut read_ops = vec![];
                for (property, values) in reads {
                    read_ops.push(Value::Vector(vec![
                        Value::Keyword(Keyword::from_name("r")),
                        Value::Integer(*property as i64),
                        Value::Vector(values.iter().map(|v| Value::Integer(*v)).collect()),
                    ]));
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("value")),
                    Value::Vector(read_ops),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("type")),
                    Value::Keyword(Keyword::from_name("invoke")),
                );
            }
            WorkItem::AppendEnd(process, appends) => {
                map.insert(
                    Value::Keyword(Keyword::from_name("index")),
                    Value::Integer(i as i64),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("process")),
                    Value::Integer(*process as i64),
                );
                let mut append_ops = vec![];
                for (property, value) in appends {
                    append_ops.push(Value::Vector(vec![
                        Value::Keyword(Keyword::from_name("append")),
                        Value::Integer(*property as i64),
                        Value::Integer(*value),
                    ]));
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("value")),
                    Value::Vector(append_ops),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("type")),
                    Value::Keyword(Keyword::from_name("ok")),
                );
            }
            WorkItem::ReadEnd(process, reads) => {
                map.insert(
                    Value::Keyword(Keyword::from_name("index")),
                    Value::Integer(i as i64),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("process")),
                    Value::Integer(*process as i64),
                );
                let mut read_ops = vec![];
                for (property, values) in reads {
                    read_ops.push(Value::Vector(vec![
                        Value::Keyword(Keyword::from_name("r")),
                        Value::Integer(*property as i64),
                        Value::Vector(values.iter().map(|v| Value::Integer(*v)).collect()),
                    ]));
                }
                map.insert(
                    Value::Keyword(Keyword::from_name("value")),
                    Value::Vector(read_ops),
                );
                map.insert(
                    Value::Keyword(Keyword::from_name("type")),
                    Value::Keyword(Keyword::from_name("ok")),
                );
            }
        }
        let edn_value = Value::Map(map);
        output_document.push_str(&format!("{}\n", edn_format::emit_str(&edn_value)));
    }

    std::fs::write(output_path, output_document)?;
    Ok(())
}

fn main() -> Result<(), eyre::Error> {
    color_eyre::install()?;
    let args: Args = Args::parse();

    println!("Direct database list-append workload");
    println!("Configuration:");
    println!("  Properties: {}", args.num_props);
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

    // Setup database
    let (obj, prop_symbols) = setup_database(&db, args.num_props)?;

    println!(
        "Starting {} concurrent workloads",
        args.num_concurrent_workloads
    );

    let workload_start = Instant::now();

    // Spawn workload threads
    let mut handles = vec![];
    for process_id in 0..args.num_concurrent_workloads {
        let db = db.clone();
        let prop_symbols = prop_symbols.clone();
        let num_iterations = args.num_workload_iterations;

        let handle = thread::spawn(move || {
            workload_thread(db, obj, prop_symbols, process_id, num_iterations)
        });
        handles.push(handle);
    }

    // Collect results
    println!("\nWaiting for threads to complete...");
    let mut workload_results = vec![];
    let mut completed = 0;
    for handle in handles {
        let result = handle.join().unwrap()?;
        workload_results.extend(result);
        completed += 1;
        if completed % 5 == 0 {
            println!(
                "Collected results from {}/{} threads",
                completed, args.num_concurrent_workloads
            );
        }
    }

    let total_duration = workload_start.elapsed();

    // Sort by timestamp
    workload_results.sort_by(|a, b| a.0.cmp(&b.0));

    println!("Collected {} operations", workload_results.len());

    // Print performance metrics
    print_performance_metrics(&workload_results, total_duration);

    // Write EDN output
    write_edn_output(&workload_results, &args.output_file)?;

    println!("Wrote workload to {}", args.output_file.display());

    Ok(())
}
