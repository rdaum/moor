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

use super::*;

struct FailingDatabase {
    inner: TxDB,
    successful_world_states_before_failure: Arc<AtomicUsize>,
}

impl WorldStateSource for FailingDatabase {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let remaining = self
            .successful_world_states_before_failure
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                Some(match remaining {
                    usize::MAX | 0 => usize::MAX,
                    _ => remaining - 1,
                })
            })
            .unwrap();

        if remaining == 0 {
            return Err(WorldStateError::DatabaseError(
                "injected new world state failure".to_string(),
            ));
        }

        self.inner.new_world_state()
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        self.inner.checkpoint()
    }
}

impl Database for FailingDatabase {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        self.inner.loader_client()
    }

    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError> {
        self.inner.create_snapshot()
    }

    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError> {
        self.inner.create_snapshot_async(callback)
    }

    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError> {
        self.inner.gc_interface()
    }
}

struct GatedConflictDatabase {
    inner: TxDB,
    gate_next_world_state: Arc<AtomicBool>,
    started: flume::Sender<()>,
    release: flume::Receiver<()>,
}

impl WorldStateSource for GatedConflictDatabase {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let world_state = self.inner.new_world_state()?;
        if self.gate_next_world_state.swap(false, Ordering::SeqCst) {
            self.started.send(()).unwrap();
            self.release.recv().unwrap();
        }
        Ok(world_state)
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        self.inner.checkpoint()
    }
}

impl Database for GatedConflictDatabase {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        self.inner.loader_client()
    }

    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError> {
        self.inner.create_snapshot()
    }

    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError> {
        self.inner.create_snapshot_async(callback)
    }

    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError> {
        self.inner.gc_interface()
    }
}

fn setup_failing_scheduler(
    verbs: &[TestVerb],
) -> (SchedulerClient, RunningScheduler, Arc<AtomicUsize>) {
    let successful_world_states_before_failure = Arc::new(AtomicUsize::new(usize::MAX));
    let database = FailingDatabase {
        inner: setup_database(verbs),
        successful_world_states_before_failure: successful_world_states_before_failure.clone(),
    };
    let (client, scheduler) = start_scheduler(Box::new(database));
    (client, scheduler, successful_world_states_before_failure)
}

#[test]
fn captured_output_survives_conflict_retry() {
    let verb = Symbol::mk("capture_retry");
    let database = setup_database(&[TestVerb {
        name: verb,
        program: compile(
            "name = this.name; notify(player, \"successful attempt\"); this.name = name; return 42;",
            CompileOptions::default(),
        )
        .unwrap(),
        argspec: VerbArgsSpec::this_none_this(),
    }]);
    let database_probe = database.clone();
    let gate_next_world_state = Arc::new(AtomicBool::new(false));
    let (started_send, started_recv) = flume::bounded(1);
    let (release_send, release_recv) = flume::bounded(1);
    let (client, _scheduler) = start_scheduler(Box::new(GatedConflictDatabase {
        inner: database,
        gate_next_world_state: gate_next_world_state.clone(),
        started: started_send,
        release: release_recv,
    }));

    gate_next_world_state.store(true, Ordering::SeqCst);
    let session = Arc::new(MockClientSession::new());
    let submit_client = client.clone();
    let submit_session = session.clone();
    let submit = std::thread::spawn(move || {
        submit_client.submit_verb_task(
            &SYSTEM_OBJECT,
            &ObjectRef::Id(SYSTEM_OBJECT),
            verb,
            List::mk_list(&[]),
            v_empty_str(),
            &SYSTEM_OBJECT,
            submit_session,
        )
    });

    started_recv
        .recv_timeout(Duration::from_secs(1))
        .expect("task transaction should reach the conflict gate");
    let mut competing = database_probe.new_world_state().unwrap();
    competing
        .update_property(
            &system_permissions(),
            &SYSTEM_OBJECT,
            Symbol::mk("name"),
            &v_str("concurrent writer"),
        )
        .unwrap();
    assert!(matches!(
        competing.commit().unwrap(),
        CommitResult::Success { .. }
    ));
    release_send.send(()).unwrap();

    let handle = submit.join().unwrap().unwrap();
    assert_eq!(wait_result(&handle), Ok(v_int(42)));
    let committed = session.committed();
    assert_eq!(committed.len(), 1);
    let moor_common::tasks::Event::Notify { value, .. } = committed[0].event() else {
        panic!("expected a notify event");
    };
    assert_eq!(value, v_str("successful attempt"));
}

/// A database that counts `new_world_state()` calls, so a test can wait
/// for a specific transaction (identified by ordinal) to open without
/// blocking it.
/// A database whose `new_world_state()` parks on a gate exactly once,
/// after the world state's snapshot has been taken, so a competing write
/// landed while parked is guaranteed to be invisible to it (and so
/// guaranteed to conflict when the parked transaction later commits a
/// write of its own). This is `GatedConflictDatabase` generalised to
/// gate an arbitrary sequence of opens, not just the first.
struct SequencedGateDatabase {
    inner: TxDB,
    /// Remaining gate arm-counts, one entry consumed per `new_world_state`
    /// call while the queue is non-empty. `0` means "don't park"; `1+`
    /// means "park, then decrement" -- kept simple as a shared counter of
    /// *how many future opens should park*, driven externally by storing
    /// `1` before a release and `0` otherwise via `armed`.
    armed: Arc<AtomicBool>,
    started: flume::Sender<()>,
    release: flume::Receiver<()>,
}

impl WorldStateSource for SequencedGateDatabase {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let world_state = self.inner.new_world_state()?;
        if self.armed.swap(false, Ordering::SeqCst) {
            self.started.send(()).unwrap();
            self.release.recv().unwrap();
        }
        Ok(world_state)
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        self.inner.checkpoint()
    }
}

impl Database for SequencedGateDatabase {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        self.inner.loader_client()
    }
    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError> {
        self.inner.create_snapshot()
    }
    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError> {
        self.inner.create_snapshot_async(callback)
    }
    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError> {
        self.inner.gc_interface()
    }
}

/// Drive a task through `conflicts_per_boundary.len()` `commit()`
/// boundaries, injecting `conflicts_per_boundary[i]` write-write
/// conflicts on `name` before the i-th boundary's write is allowed to
/// succeed.
///
/// Every transaction the task opens is parked, snapshot already taken,
/// exactly like `captured_output_survives_conflict_retry`'s single gate,
/// but re-armed before every release so a whole schedule of conflicts
/// can be driven deterministically: while a transaction is parked, its
/// snapshot cannot see a write landed after that point, so releasing it
/// straight into a commit is guaranteed to conflict against a write the
/// driver lands first.
///
/// Returns the task's result and, per injected conflict, the wall time
/// from releasing the conflicted attempt to the retry's transaction
/// reaching the gate (the scheduler's backoff delay plus scheduling
/// slack).
fn run_with_conflict_schedule(
    conflicts_per_boundary: &[u32],
) -> (Result<Var, SchedulerError>, Vec<Duration>) {
    let verb = Symbol::mk("conflict_schedule");
    let mut program = String::new();
    for _ in conflicts_per_boundary {
        // `name` is a special built-in property (object metadata) with
        // its own storage, not the ordinary `object_propvalues` relation
        // the conflict detector tracks -- writing it never conflicts
        // with anything. Use a real user-defined property instead.
        program.push_str("v = this.conflict_counter; this.conflict_counter = v; commit(); ");
    }
    program.push_str("return 7;");
    let database = setup_database(&[TestVerb {
        name: verb,
        program: compile(&program, CompileOptions::default()).unwrap(),
        argspec: VerbArgsSpec::this_none_this(),
    }]);
    {
        let mut tx = database.new_world_state().unwrap();
        tx.define_property(
            &system_permissions(),
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            Symbol::mk("conflict_counter"),
            &SYSTEM_OBJECT,
            PropFlag::all_flags(),
            Some(v_int(0)),
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let database_probe = database.clone();
    let armed = Arc::new(AtomicBool::new(false));
    let (started_send, started_recv) = flume::bounded(1);
    let (release_send, release_recv) = flume::bounded(1);
    let (client, _scheduler) = start_scheduler(Box::new(SequencedGateDatabase {
        inner: database,
        armed: armed.clone(),
        started: started_send,
        release: release_recv,
    }));

    armed.store(true, Ordering::SeqCst);
    let session = Arc::new(NoopClientSession::new());
    let submit_client = client.clone();
    let submit = std::thread::spawn(move || {
        submit_client.submit_verb_task(
            &SYSTEM_OBJECT,
            &ObjectRef::Id(SYSTEM_OBJECT),
            verb,
            List::mk_list(&[]),
            v_empty_str(),
            &SYSTEM_OBJECT,
            session,
        )
    });

    let mut retry_durations = Vec::new();
    // Invariant at the top of each loop body below: the task has a
    // transaction open and parked at the gate, snapshot already taken.
    started_recv
        .recv_timeout(Duration::from_secs(5))
        .expect("task's first transaction should reach the gate");
    for (i, &conflicts) in conflicts_per_boundary.iter().enumerate() {
        for c in 0..conflicts {
            // Land a competing write invisible to the parked snapshot.
            let mut competing = database_probe.new_world_state().unwrap();
            competing
                .update_property(
                    &system_permissions(),
                    &SYSTEM_OBJECT,
                    Symbol::mk("conflict_counter"),
                    &v_int(1000 + (i as i64) * 100 + c as i64),
                )
                .unwrap();
            assert!(matches!(
                competing.commit().unwrap(),
                CommitResult::Success { .. }
            ));
            // Arm the gate for the retry's transaction, then release
            // this parked one into its (now-conflicting) commit.
            armed.store(true, Ordering::SeqCst);
            let released_at = std::time::Instant::now();
            release_send.send(()).unwrap();
            started_recv
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| {
                    panic!("boundary {i} conflict {c}: retry never reached the gate")
                });
            retry_durations.push(released_at.elapsed());
        }
        // Arm the gate for the next boundary's transaction (unless this
        // was the last boundary), then release this attempt, which
        // commits successfully.
        let is_last_boundary = i + 1 == conflicts_per_boundary.len();
        if !is_last_boundary {
            armed.store(true, Ordering::SeqCst);
        }
        release_send.send(()).unwrap();
        if !is_last_boundary {
            started_recv
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|_| panic!("boundary {i}: next boundary never reached the gate"));
        }
    }

    let handle = submit.join().unwrap().unwrap();
    (wait_result(&handle), retry_durations)
}

/// A successful transaction boundary resets the consecutive-retry
/// counter. Three conflicts before the first boundary push `retries` to
/// 3; after that boundary succeeds, one conflict before the second
/// boundary must be retry #1 (backoff shift 0: 10-50ms), not retry #4
/// (shift 3: 80-400ms).
#[test]
fn retries_reset_after_successful_boundary() {
    let (result, retries) = run_with_conflict_schedule(&[3, 1]);
    assert_eq!(result, Ok(v_int(7)));
    assert_eq!(retries.len(), 4, "expected 4 injected conflicts total");
    // First conflict on the second boundary (overall retry #4): shift 0.
    // Without the counter reset this would be retry #4, shift 3
    // (>= 80ms base backoff alone, before scheduling slack).
    assert!(
        retries[3] < Duration::from_millis(70),
        "retry after a successful boundary should use shift 0 (10-50ms base, \
         no shift), got {:?}; without the counter reset this would be \
         shift 3 (>= 80ms)",
        retries[3]
    );
}

/// One conflict per boundary, across more boundaries than
/// `max_task_retries` (10 by default), never aborts: the limit bounds
/// consecutive retries against one piece of work, not lifetime retries
/// of a long-lived task. Before the fix in `refresh_retry_state`, this
/// task would abort with `TaskAbortedError` on its 10th lifetime
/// conflict even though no boundary ever saw more than one.
#[test]
fn one_retry_per_boundary_never_aborts() {
    let boundaries = 12;
    let schedule = vec![1u32; boundaries];
    let (result, retries) = run_with_conflict_schedule(&schedule);
    assert_eq!(
        result,
        Ok(v_int(7)),
        "task should complete despite 12 lifetime conflicts (> max_task_retries \
         of 10), since only one conflict ever occurs per boundary"
    );
    assert_eq!(retries.len(), boundaries);
}

/// A schedule created in a task that then rolls back must never exist:
/// creation is buffered until commit, exactly like `task_send`.
#[test]
fn schedule_creation_is_discarded_on_rollback() {
    let (client, scheduler) = setup_scheduler(&[]);
    let session = Arc::new(NoopClientSession::new());
    let handle = client
        .submit_eval_task(
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            "add_verb(#0, {#0, \"rxd\", \"sched_probe\"}, {\"this\", \"none\", \"this\"}); \
             set_verb_code(#0, \"sched_probe\", {\"return 0;\"}); commit(); \
             id = schedule_at(#0, \"sched_probe\", time() + 3600); rollback(); return id;"
                .to_string(),
            None,
            session,
            Arc::new(FeaturesConfig::default()),
        )
        .unwrap();
    // rollback() ends the task without a normal result.
    let _ = wait_result(&handle);
    let lc = scheduler.client.scheduler_for_test().lifecycle.lock();
    assert!(
        lc.schedule_q.all_ids().is_empty(),
        "rolled-back schedule_at must not leave an entry: {:?}",
        lc.schedule_q.all_ids()
    );
    assert!(lc.task_q.active.is_empty());
}

#[test]
fn test_suspend_zero_transaction_renewal_failure_aborts_cleanly() {
    let (client, _sched, successful_world_states_before_failure) = setup_failing_scheduler(&[]);

    // Allow the initial task transaction, then fail the replacement requested by suspend(0).
    successful_world_states_before_failure.store(1, Ordering::SeqCst);

    let session = Arc::new(NoopClientSession::new());
    let handle = client
        .submit_eval_task(
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            "suspend(0); return 123;".to_string(),
            None,
            session,
            Arc::new(FeaturesConfig::default()),
        )
        .unwrap();

    let error = wait_result(&handle).unwrap_err();
    assert_eq!(error, SchedulerError::CouldNotStartTask);
}

#[test]
fn resumed_input_survives_conflict_retry() {
    let verb = Symbol::mk("resume_retry");
    let database = setup_database(&[TestVerb {
        name: verb,
        program: compile(
            "value = read(); this.name = this.name + \" resumed\"; return value;",
            CompileOptions::default(),
        )
        .unwrap(),
        argspec: VerbArgsSpec::this_none_this(),
    }]);
    let database_probe = database.clone();
    let gate_next_world_state = Arc::new(AtomicBool::new(false));
    let (started_send, started_recv) = flume::bounded(1);
    let (release_send, release_recv) = flume::bounded(1);
    let (client, _scheduler) = start_scheduler(Box::new(GatedConflictDatabase {
        inner: database,
        gate_next_world_state: gate_next_world_state.clone(),
        started: started_send,
        release: release_recv,
    }));
    let session = Arc::new(MockClientSession::new());
    let handle = client
        .submit_verb_task(
            &SYSTEM_OBJECT,
            &ObjectRef::Id(SYSTEM_OBJECT),
            verb,
            List::mk_list(&[]),
            v_empty_str(),
            &SYSTEM_OBJECT,
            session.clone(),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let input_id = loop {
        if let Some((_, input_id, _)) = session.input_requests().into_iter().next() {
            break input_id;
        }
        assert!(Instant::now() < deadline, "read did not suspend");
        std::thread::yield_now();
    };
    gate_next_world_state.store(true, Ordering::SeqCst);
    let resume = std::thread::spawn(move || {
        client.submit_requested_input(
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            input_id,
            v_str("delivered once"),
        )
    });
    started_recv
        .recv_timeout(Duration::from_secs(5))
        .expect("resumed transaction did not reach the gate");
    let mut competing = database_probe.new_world_state().unwrap();
    competing
        .update_property(
            &system_permissions(),
            &SYSTEM_OBJECT,
            Symbol::mk("name"),
            &v_str("concurrent writer"),
        )
        .unwrap();
    assert!(matches!(
        competing.commit().unwrap(),
        CommitResult::Success { .. }
    ));
    release_send.send(()).unwrap();
    resume.join().unwrap().unwrap();
    assert_eq!(wait_result(&handle), Ok(v_str("delivered once")));
    assert_eq!(session.input_requests().len(), 1);
}
