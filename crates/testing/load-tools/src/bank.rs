//! Elle bank consistency checker workload generator
//! Tests bank account transfers for serializability anomalies

use clap::Parser;
use clap_derive::Parser;
use edn_format::{Keyword, Value};
use moor_common::model::{CommitResult, WorldStateSource};
use moor_db::TxDB;
use moor_model_checker::elle_common::{self, EdnEvent};
use moor_var::{Obj, Symbol, v_int};
use rand::Rng;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long, help = "Database path", default_value = "test_db")]
    db_path: PathBuf,
    #[arg(long, default_value = "5")]
    num_accounts: usize,
    #[arg(long, default_value = "100")]
    initial_balance: i64,
    #[arg(long, default_value = "20")]
    max_transfer: i64,
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
    account_symbols: Vec<Symbol>,
    process_id: usize,
    num_iterations: usize,
    max_transfer: i64,
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

        let is_transfer = rng.gen_bool(0.8); // 80% transfers, 20% reads

        if is_transfer {
            // Transfer operation
            let from_idx = rng.gen_range(0..account_symbols.len());
            let mut to_idx = rng.gen_range(0..account_symbols.len());
            // Ensure from != to
            while to_idx == from_idx {
                to_idx = rng.gen_range(0..account_symbols.len());
            }
            let amount = rng.gen_range(1..=max_transfer);

            let mut retry_count = 0;
            loop {
                if retry_count >= MAX_RETRIES {
                    skipped_ops += 1;
                    break;
                }
                retry_count += 1;

                let start = Instant::now();
                let mut tx = db.new_world_state()?;

                // Read both accounts
                let from_balance = tx
                    .retrieve_property(&obj, &obj, account_symbols[from_idx])?
                    .as_integer()
                    .unwrap_or(0);
                let to_balance = tx
                    .retrieve_property(&obj, &obj, account_symbols[to_idx])?
                    .as_integer()
                    .unwrap_or(0);

                // Check if transfer would cause negative balance
                if from_balance - amount < 0 {
                    // Transfer would fail - skip this operation and record failure
                    tx.commit()?;
                    let negative_vec = Value::Vector(vec![
                        Value::Keyword(Keyword::from_name("negative")),
                        Value::Integer(from_idx as i64),
                        Value::Integer(from_balance - amount),
                    ]);
                    events.push(EdnEvent::invoke(
                        start,
                        process_id,
                        "transfer".to_string(),
                        Value::Map({
                            let mut m = BTreeMap::new();
                            m.insert(
                                Value::Keyword(Keyword::from_name("from")),
                                Value::Integer(from_idx as i64),
                            );
                            m.insert(
                                Value::Keyword(Keyword::from_name("to")),
                                Value::Integer(to_idx as i64),
                            );
                            m.insert(
                                Value::Keyword(Keyword::from_name("amount")),
                                Value::Integer(amount),
                            );
                            m
                        }),
                    ));
                    // For failed transfers, Elle expects :fail type with [:negative ...] value
                    events.push(EdnEvent::fail(
                        Instant::now(),
                        process_id,
                        "transfer".to_string(),
                        negative_vec,
                    ));
                    break;
                }

                // Perform transfer
                tx.update_property(
                    &obj,
                    &obj,
                    account_symbols[from_idx],
                    &v_int(from_balance - amount),
                )?;
                tx.update_property(
                    &obj,
                    &obj,
                    account_symbols[to_idx],
                    &v_int(to_balance + amount),
                )?;

                match tx.commit()? {
                    CommitResult::Success { .. } => {
                        let mut transfer_map = BTreeMap::new();
                        transfer_map.insert(
                            Value::Keyword(Keyword::from_name("from")),
                            Value::Integer(from_idx as i64),
                        );
                        transfer_map.insert(
                            Value::Keyword(Keyword::from_name("to")),
                            Value::Integer(to_idx as i64),
                        );
                        transfer_map.insert(
                            Value::Keyword(Keyword::from_name("amount")),
                            Value::Integer(amount),
                        );
                        let value = Value::Map(transfer_map);

                        events.push(EdnEvent::invoke(
                            start,
                            process_id,
                            "transfer".to_string(),
                            value.clone(),
                        ));
                        events.push(EdnEvent::ok(
                            Instant::now(),
                            process_id,
                            "transfer".to_string(),
                            value,
                        ));
                        break;
                    }
                    CommitResult::ConflictRetry => continue,
                }
            }
        } else {
            // Read all accounts
            let mut retry_count = 0;
            loop {
                if retry_count >= MAX_RETRIES {
                    skipped_ops += 1;
                    break;
                }
                retry_count += 1;

                let start = Instant::now();
                let tx = db.new_world_state()?;

                let mut balances = BTreeMap::new();
                for (idx, &sym) in account_symbols.iter().enumerate() {
                    let balance = tx
                        .retrieve_property(&obj, &obj, sym)?
                        .as_integer()
                        .unwrap_or(0);
                    balances.insert(Value::Integer(idx as i64), Value::Integer(balance));
                }

                match tx.commit()? {
                    CommitResult::Success { .. } => {
                        let value = Value::Map(balances);
                        events.push(EdnEvent::invoke(
                            start,
                            process_id,
                            "read".to_string(),
                            Value::Nil,
                        ));
                        events.push(EdnEvent::ok(
                            Instant::now(),
                            process_id,
                            "read".to_string(),
                            value,
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

    println!("Bank Elle workload");
    println!("Configuration:");
    println!("  Accounts: {}", args.num_accounts);
    println!("  Initial balance per account: {}", args.initial_balance);
    println!("  Max transfer amount: {}", args.max_transfer);
    println!("  Concurrent workloads: {}", args.num_concurrent_workloads);
    println!(
        "  Iterations per workload: {}",
        args.num_workload_iterations
    );
    println!("  Output file: {}", args.output_file.display());
    println!(
        "  Total initial balance: {}",
        args.num_accounts as i64 * args.initial_balance
    );
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

    // Setup database - only account 0 gets initial_balance, rest start at 0
    let (obj, account_symbols) =
        elle_common::setup_test_database(&db, args.num_accounts, "account", |i| {
            if i == 0 {
                v_int(args.initial_balance)
            } else {
                v_int(0)
            }
        })?;

    // Create initial read to establish the account universe for Elle
    let mut initial_events = Vec::new();
    let tx = db.new_world_state()?;
    let mut balances = BTreeMap::new();
    for (idx, &sym) in account_symbols.iter().enumerate() {
        let balance = tx
            .retrieve_property(&obj, &obj, sym)?
            .as_integer()
            .unwrap_or(0);
        balances.insert(Value::Integer(idx as i64), Value::Integer(balance));
    }
    tx.commit()?;

    let start = Instant::now();
    let value = Value::Map(balances);
    initial_events.push(EdnEvent::invoke(start, 0, "read".to_string(), Value::Nil));
    initial_events.push(EdnEvent::ok(Instant::now(), 0, "read".to_string(), value));

    // Run workloads
    let max_transfer = args.max_transfer;
    let workload_fn = move |db: Arc<TxDB>,
                            obj: Obj,
                            account_symbols: Vec<Symbol>,
                            process_id: usize,
                            num_iterations: usize|
          -> Result<Vec<EdnEvent>, eyre::Error> {
        workload_thread(
            db,
            obj,
            account_symbols,
            process_id,
            num_iterations,
            max_transfer,
        )
    };

    let mut all_events = elle_common::run_concurrent_workloads(
        db,
        obj,
        account_symbols,
        args.num_concurrent_workloads,
        args.num_workload_iterations,
        workload_fn,
    )?;

    // Prepend initial read and sort by timestamp
    initial_events.extend(all_events);
    let mut all_events = initial_events;
    all_events.sort_by_key(|e| e.timestamp);

    println!(
        "Collected {} events (including initial read)",
        all_events.len()
    );

    // Write EDN output
    elle_common::write_edn_history(&all_events, &args.output_file)?;

    println!("Wrote workload to {}", args.output_file.display());

    Ok(())
}
