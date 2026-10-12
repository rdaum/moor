// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com>
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.

//! Mock-session scenarios for Cowbell and Snore cores.
//!
//! Imports an objdef overlay into a temporary database, starts the kernel scheduler, and runs
//! `.moot` scenarios with one `TestSession` per player. Commands go through the real command
//! parser and captured narrative output drives `=` assertions.
//!
//! This harness captures committed output. `tests/wire.py` separately exercises daemon and telnet I/O.
//!
//! usage: core-session-runner --core-dir DIR --moot FILE_OR_DIR [--wizard N] [--programmer N]
//!                       [--nonprogrammer N]

mod scenario;
mod session;

use eyre::{Result, eyre};
use moor_common::model::ObjectRef;
use moor_common::tasks::{
    CommandError, Event, Exception, NarrativeEvent, NoopSystemControl, SchedulerError, Session,
    SessionError, SessionFactory,
};
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_kernel::SchedulerClient;
use moor_kernel::config::{Config, FeaturesConfig};
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::tasks::{NoopTasksDb, TaskHandle, TaskNotification};
use moor_moot::MootRunner;
use moor_objdef::{ObjDefLoaderOptions, ObjDefSet, ObjectDefinitionLoader};
use moor_var::{
    E_VERBNF, List, Obj, SYSTEM_OBJECT, Symbol, Var, v_bool, v_empty_str, v_obj, v_string,
};
use scenario::{Actors, SessionRunner};
use session::{InputRequest, SessionHub, TestSession};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct MockMootRunner {
    scheduler: SchedulerClient,
    features: Arc<FeaturesConfig>,
    sessions: HashMap<Obj, Arc<TestSession>>,
    named_sessions: HashMap<String, (Obj, Arc<TestSession>)>,
    lines: HashMap<Obj, VecDeque<String>>,
    events: HashMap<Obj, VecDeque<(Obj, NarrativeEvent)>>,
    last_result: Option<Var>,
    hub: Arc<SessionHub>,
    input_queue: HashMap<Obj, VecDeque<String>>,
    pending_inputs: VecDeque<InputRequest>,
}

impl MockMootRunner {
    fn new(
        scheduler: SchedulerClient,
        features: Arc<FeaturesConfig>,
        hub: Arc<SessionHub>,
    ) -> Self {
        Self {
            scheduler,
            features,
            sessions: HashMap::new(),
            named_sessions: HashMap::new(),
            lines: HashMap::new(),
            events: HashMap::new(),
            last_result: None,
            hub,
            input_queue: HashMap::new(),
            pending_inputs: VecDeque::new(),
        }
    }

    fn session(&mut self, player: &Obj) -> Arc<TestSession> {
        if !self.sessions.contains_key(player) {
            self.hub.set_connected(*player, true);
            self.sessions.insert(
                *player,
                Arc::new(TestSession::for_player(self.hub.clone(), *player)),
            );
        }
        self.sessions.get(player).expect("session").clone()
    }

    /// Answer pending `read()` requests from the scripted input queues.
    fn pump_input(&mut self) -> Result<()> {
        self.pending_inputs.extend(self.hub.take_input_requests());
        loop {
            let mut submitted = false;
            let pending = self.pending_inputs.len();
            for _ in 0..pending {
                let (request_player, id, meta) =
                    self.pending_inputs.pop_front().expect("pending input");
                let Some(line) = self
                    .input_queue
                    .get_mut(&request_player)
                    .and_then(|queue| queue.pop_front())
                else {
                    self.pending_inputs.push_back((request_player, id, meta));
                    continue;
                };
                self.scheduler
                    .submit_requested_input(&request_player, &request_player, id, v_string(line))
                    .map_err(|err| eyre!("input submission failed: {err}"))?;
                submitted = true;
            }
            if !submitted {
                return Ok(());
            }
        }
    }

    /// Wait for a task to finish, answering `read()` requests from the scripted queues.
    fn run_task(&mut self, handle: &TaskHandle, exception_as_value: bool) -> Result<Var> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if std::time::Instant::now() > deadline {
                return Err(eyre!("task timed out"));
            }
            match handle
                .receiver()
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok((_, Ok(TaskNotification::Suspended))) => continue,
                Ok((_, Ok(TaskNotification::Result(value)))) => return Ok(value),
                Ok((
                    _,
                    Err(SchedulerError::TaskAbortedException(Exception {
                        error, backtrace, ..
                    })),
                )) => {
                    if exception_as_value {
                        return Ok(error.into());
                    }
                    return Err(eyre!("command task aborted: {error:?}; {backtrace:?}"));
                }
                Ok((
                    _,
                    Err(SchedulerError::CommandExecutionError(CommandError::NoCommandMatch)),
                )) => {
                    return Ok(E_VERBNF.msg("No command match").into());
                }
                Ok((_, Err(SchedulerError::TaskAbortedCancelled))) => {
                    // Editor `@abort` and `kill_task` cancel the task on purpose; scenarios
                    // assert the cancellation output. Report it as a false result.
                    return Ok(v_bool(false));
                }
                Ok((_, Err(err))) => return Err(eyre!("task failed: {err}")),
                Err(flume::RecvTimeoutError::Timeout) => self.pump_input()?,
                Err(err) => return Err(eyre!("task channel closed: {err}")),
            }
        }
    }

    /// Route all committed output, including output from background tasks, by recipient.
    fn harvest_all(&mut self) {
        for (recipient, event) in self.hub.take_committed_events() {
            self.events
                .entry(recipient)
                .or_default()
                .push_back((recipient, event.clone()));
            if let Some(text) = render_event(&event) {
                self.lines
                    .entry(recipient)
                    .or_default()
                    .extend(text.split('\n').map(str::to_string));
            }
        }
    }
}

fn render_event(event: &NarrativeEvent) -> Option<String> {
    match &event.event {
        Event::Notify { value, .. } => Some(match value.as_list() {
            Some(lines) => lines
                .iter()
                .map(|line| {
                    line.as_string()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{line:?}"))
                })
                .collect::<Vec<_>>()
                .join("\n"),
            None => value
                .as_string()
                .map(str::to_string)
                .unwrap_or_else(|| format!("{value:?}")),
        }),
        Event::Present(presentation) => Some(presentation.content.clone()),
        _ => None,
    }
}

impl MootRunner for MockMootRunner {
    type Value = Var;

    fn eval<S: Into<String>>(&mut self, player: &Obj, command: S) -> Result<()> {
        self.harvest_all();
        let session = self.session(player);
        let connection = session
            .connection_id()
            .ok_or_else(|| eyre!("selected session has no connection"))?;
        if self.hub.owner_for(connection) != Some(*player) {
            return Err(eyre!(
                "cannot submit a new task for {player} on detached or reassigned connection {connection}"
            ));
        }
        let handle = self
            .scheduler
            .submit_eval_task(
                player,
                player,
                command.into(),
                None,
                session,
                self.features.clone(),
            )
            .map_err(|err| eyre!("eval submit failed for {player}: {err}"))?;
        let result = self.run_task(&handle, true)?;
        self.last_result = Some(result);
        self.harvest_all();
        Ok(())
    }

    fn command<S: AsRef<str>>(&mut self, player: &Obj, command: S) -> Result<()> {
        self.harvest_all();
        let session = self.session(player);
        let connection = session
            .connection_id()
            .ok_or_else(|| eyre!("selected session has no connection"))?;
        if self.hub.owner_for(connection) != Some(*player) {
            return Err(eyre!(
                "cannot submit a new task for {player} on detached or reassigned connection {connection}"
            ));
        }
        let handle = self
            .scheduler
            .submit_command_task(&SYSTEM_OBJECT, player, command.as_ref(), session)
            .map_err(|err| eyre!("command submit failed for {player}: {err}"))?;
        let result = self.run_task(&handle, false)?;
        self.last_result = Some(result);
        self.harvest_all();
        for events in self.events.values() {
            for (recipient, event) in events {
                if let Event::Notify {
                    metadata: Some(metadata),
                    ..
                } = &event.event
                    && metadata
                        .iter()
                        .any(|(key, value)| key.as_str() == "command_exception" && value.is_true())
                {
                    return Err(eyre!(
                        "caught command exception for connection {recipient}: {event:?}"
                    ));
                }
            }
        }
        Ok(())
    }

    fn read_line(&mut self, player: &Obj) -> Result<Option<String>> {
        let recipient = self.session(player).connection_id().unwrap_or(*player);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            self.harvest_all();
            if let Some(line) = self
                .lines
                .get_mut(&recipient)
                .and_then(|lines| lines.pop_front())
            {
                return Ok(Some(line));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            self.pump_input()?;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn read_eval_result(&mut self, _player: &Obj) -> Result<Option<Var>> {
        Ok(self.last_result.take())
    }

    fn read_command_result(&mut self, _player: &Obj) -> Result<Option<Var>> {
        Ok(self.last_result.take())
    }

    fn none(&self) -> Var {
        v_bool(false)
    }
}

/// Match an event contract without imposing order on independent committed tasks.
fn event_matches(event: &Event, content_type: &str, audience: &str) -> bool {
    if let Some(mime) = content_type.strip_prefix("present:") {
        return matches!(event, Event::Present(presentation)
            if presentation.content_type == mime && presentation.target == audience);
    }
    if let Some(namespace) = content_type.strip_prefix("data:") {
        return matches!(event, Event::Data { namespace: actual, kind, .. }
            if actual.as_str() == namespace && kind.as_str() == audience);
    }
    let Event::Notify {
        content_type: actual,
        metadata,
        ..
    } = event
    else {
        return false;
    };
    actual.is_some_and(|actual| actual.as_str() == content_type)
        && metadata.as_ref().is_some_and(|metadata| {
            metadata.iter().any(|(key, value)| {
                key.as_str() == "audience"
                    && value
                        .as_symbol()
                        .is_ok_and(|value| value.as_str() == audience)
            })
        })
}

impl SessionRunner for MockMootRunner {
    fn select_connection(&mut self, name: &str) -> Result<Obj> {
        let (player, session) = self
            .named_sessions
            .get(name)
            .cloned()
            .ok_or_else(|| eyre!("unknown connection: {name}"))?;
        self.sessions.insert(player, session);
        Ok(player)
    }
    fn attach_connection(&mut self, name: &str, player: Obj) -> Result<()> {
        if self.named_sessions.contains_key(name) {
            return Err(eyre!("duplicate connection name: {name}"));
        }
        let connection = self.hub.attach(player);
        self.named_sessions.insert(
            name.into(),
            (
                player,
                Arc::new(TestSession::for_connection(self.hub.clone(), connection)),
            ),
        );
        Ok(())
    }
    fn detach_connection(&mut self, name: &str) -> Result<()> {
        let (_, session) = self
            .named_sessions
            .get(name)
            .ok_or_else(|| eyre!("unknown connection: {name}"))?;
        if !self.hub.detach(session.connection_id().unwrap()) {
            return Err(eyre!("connection already detached: {name}"));
        }
        Ok(())
    }
    fn reassign_connection(&mut self, name: &str, player: Obj) -> Result<()> {
        let endpoint = self
            .named_sessions
            .get_mut(name)
            .ok_or_else(|| eyre!("unknown connection: {name}"))?;
        if !self
            .hub
            .reassign(endpoint.1.connection_id().unwrap(), player)
        {
            return Err(eyre!("connection detached: {name}"));
        }
        endpoint.0 = player;
        Ok(())
    }

    fn expect_event(&mut self, player: &Obj, content_type: &str, audience: &str) -> Result<()> {
        let recipient = self.session(player).connection_id().unwrap_or(*player);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            self.harvest_all();
            for events in self.events.values() {
                for (connection, event) in events {
                    if let Event::Notify {
                        metadata: Some(metadata),
                        ..
                    } = &event.event
                        && metadata.iter().any(|(key, value)| {
                            key.as_str() == "command_exception" && value.is_true()
                        })
                    {
                        return Err(eyre!(
                            "caught command exception for connection {connection}: {event:?}"
                        ));
                    }
                }
            }
            if let Some(kind) = content_type.strip_prefix("history:") {
                if self.hub.consume_logged(*player, kind, audience) {
                    return Ok(());
                }
                if std::time::Instant::now() >= deadline {
                    return Err(eyre!(
                        "missing committed history for {player}: {kind} {audience}"
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            let events = self.events.entry(recipient).or_default();
            if content_type == "none" && audience == "none" {
                if let Some((connection, event)) = events.front() {
                    return Err(eyre!(
                        "unexpected committed event for {player}, connection {connection}: {event:?}"
                    ));
                }
                return Ok(());
            }
            if let Some(index) = events
                .iter()
                .position(|(_, event)| event_matches(&event.event, content_type, audience))
            {
                events.remove(index);
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(eyre!(
                    "event contract failed for {player}, connection {recipient}: no committed {content_type} event with audience/kind {audience}; queued events: {events:?}"
                ));
            }
            self.pump_input()?;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn provide_input(&mut self, player: &Obj, text: &str) -> Result<()> {
        self.input_queue
            .entry(*player)
            .or_default()
            .push_back(text.to_string());
        Ok(())
    }

    fn system_hook(&mut self, player: &Obj, verb: &str, object: Option<&Obj>) -> Result<()> {
        // Mirror the connection host: submit the hook as a root task on #0 under system
        // authority, with the target object as both player and argument.
        let target = object.copied().unwrap_or(*player);
        // The target counts as connected while its connect/reconnect/disconnect hook runs.
        self.hub.set_connected(target, true);
        let session = Arc::new(TestSession::for_player(self.hub.clone(), target));
        self.sessions.insert(target, session.clone());
        let handle = self
            .scheduler
            .submit_verb_task(
                &target,
                &ObjectRef::Id(SYSTEM_OBJECT),
                Symbol::mk(verb),
                List::mk_list(&[v_obj(target)]),
                v_empty_str(),
                &SYSTEM_OBJECT,
                session,
            )
            .map_err(|err| eyre!("system hook {verb} submit failed: {err}"))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(eyre!("system hook {verb} timed out"));
            }

            match handle
                .receiver()
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok((_, Ok(TaskNotification::Result(_)))) => break,
                Ok((_, Ok(TaskNotification::Suspended))) => continue,
                Ok((_, Err(err))) => return Err(eyre!("system hook {verb} failed: {err:?}")),
                Err(flume::RecvTimeoutError::Timeout) => self.pump_input()?,
                Err(err) => return Err(eyre!("system hook {verb} timed out: {err}")),
            }
        }
        if matches!(verb, "user_disconnected" | "user_client_disconnected") {
            self.hub.set_connected(target, false);
        }
        self.harvest_all();
        Ok(())
    }
}

struct MockSessionFactory {
    hub: Arc<SessionHub>,
}
impl SessionFactory for MockSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        _player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(TestSession::new(self.hub.clone())))
    }
}

fn collect_moot_files(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if path.is_file() {
        files.push(path.to_path_buf());
        return Ok(files);
    }
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                files.extend(collect_moot_files(&entry_path)?);
            } else if entry_path.extension().is_some_and(|ext| ext == "moot") {
                files.push(entry_path);
            }
        }
    }
    files.sort();
    Ok(files)
}

struct Args {
    core_dir: PathBuf,
    moot: PathBuf,
    wizard: i32,
    programmer: i32,
    nonprogrammer: i32,
    cowbell: bool,
}

fn parse_args() -> Result<Args> {
    let mut cowbell = false;
    let mut core_dir = None;
    let mut moot = None;
    let mut wizard = 2;
    let mut programmer = 102;
    let mut nonprogrammer = 101;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cowbell" => cowbell = true,
            "--core-dir" => core_dir = args.next().map(PathBuf::from),
            "--moot" => moot = args.next().map(PathBuf::from),
            "--wizard" => {
                wizard = args
                    .next()
                    .ok_or_else(|| eyre!("--wizard needs a value"))?
                    .parse()?
            }
            "--programmer" => {
                programmer = args
                    .next()
                    .ok_or_else(|| eyre!("--programmer needs a value"))?
                    .parse()?
            }
            "--nonprogrammer" => {
                nonprogrammer = args
                    .next()
                    .ok_or_else(|| eyre!("--nonprogrammer needs a value"))?
                    .parse()?
            }
            other => return Err(eyre!("unexpected argument: {other}")),
        }
    }
    Ok(Args {
        core_dir: core_dir.ok_or_else(|| eyre!("missing --core-dir"))?,
        moot: moot.ok_or_else(|| eyre!("missing --moot"))?,
        wizard,
        programmer,
        nonprogrammer,
        cowbell,
    })
}

fn run() -> Result<()> {
    let args = parse_args()?;
    // Room tasks use server-signed capabilities. Keep the harness key ephemeral
    // and confined to this process, as in an isolated server instance.
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    key[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    moor_kernel::initialize_server_symmetric_key(key).map_err(|error| eyre!(error))?;
    let moot_files = collect_moot_files(&args.moot)?;
    if moot_files.is_empty() {
        return Err(eyre!(
            "no .moot scenarios found under {}",
            args.moot.display()
        ));
    }

    let features = Arc::new(FeaturesConfig {
        anonymous_objects: args.cowbell,
        bool_type: true,
        symbol_type: true,
        custom_errors: true,
        use_uuobjids: true,
        flyweight_type: args.cowbell,
        rich_notify: args.cowbell,
        use_boolean_returns: true,
        use_symbols_in_builtins: true,
        ..Default::default()
    });
    let core = ObjDefSet::read_directory(&features.compile_options(), &args.core_dir)
        .map_err(|err| eyre!("objdef compile failed: {err}"))?;

    let mut failures = Vec::new();
    for path in &moot_files {
        match run_scenario(&args, path, &core, features.clone()) {
            Ok(()) => println!("PASS {}", path.display()),
            Err(err) => {
                eprintln!("FAIL {}: {err:?}", path.display());
                failures.push(path.clone());
            }
        }
    }

    if failures.is_empty() {
        println!("session scenarios passed: {}", moot_files.len());
        Ok(())
    } else {
        Err(eyre!(
            "{} of {} session scenarios failed",
            failures.len(),
            moot_files.len()
        ))
    }
}

/// Reuse compiled definitions, but give every scenario an independent database.
fn load_database(core: &ObjDefSet) -> Result<TxDB> {
    let (database, _) = TxDB::try_open(None, DatabaseConfig::default())
        .map_err(|err| eyre!("unable to open temporary database: {err}"))?;
    let mut loader = database
        .loader_client()
        .map_err(|err| eyre!("unable to open loader: {err}"))?;
    {
        let mut object_loader = ObjectDefinitionLoader::new(loader.as_mut());
        object_loader
            .load_objdef_set(core.clone(), ObjDefLoaderOptions::default())
            .map_err(|err| eyre!("objdef import failed: {err}"))?;
    }
    loader
        .commit()
        .map_err(|err| eyre!("import commit failed: {err:?}"))?;
    Ok(database)
}

/// Each scenario gets its own scheduler and output queues so state cannot leak.
fn run_scenario(
    args: &Args,
    path: &Path,
    core: &ObjDefSet,
    features: Arc<FeaturesConfig>,
) -> Result<()> {
    let database = load_database(core)?;
    let config = Config {
        features: features.clone(),
        ..Default::default()
    };
    let scheduler = Scheduler::new(
        semver::Version::new(0, 1, 0),
        Box::new(database),
        Box::new(NoopTasksDb {}),
        Arc::new(config),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );
    let scheduler_client = scheduler
        .client()
        .map_err(|err| eyre!("unable to create scheduler client: {err:?}"))?;
    let hub = Arc::new(SessionHub::default());
    let scheduler_thread = scheduler
        .start(Arc::new(MockSessionFactory { hub: hub.clone() }))
        .map_err(|err| eyre!("unable to start scheduler: {err:?}"))?;

    let actors = Actors {
        wizard: Obj::mk_id(args.wizard),
        programmer: Obj::mk_id(args.programmer),
        nonprogrammer: Obj::mk_id(args.nonprogrammer),
    };
    let mut runner = MockMootRunner::new(scheduler_client.clone(), features.clone(), hub);
    // Register all fixture principals up front so the shared connection view is complete.
    for (name, player) in [
        ("wizard", args.wizard),
        ("programmer", args.programmer),
        ("nonprogrammer", args.nonprogrammer),
    ] {
        let player = Obj::mk_id(player);
        let session = runner.session(&player);
        runner.named_sessions.insert(name.into(), (player, session));
    }

    let result = scenario::run(&mut runner, &actors, path);

    scheduler_client
        .submit_shutdown("session scenario complete")
        .map_err(|err| eyre!("unable to stop scheduler: {err:?}"))?;
    let _ = scheduler_thread.join();

    result
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("session runner failed: {err:?}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::model::{TaskPermissions, WorldStateSource};
    use moor_common::util::BitEnum;
    use moor_objdef::ObjDefSource;

    #[test]
    fn compiled_core_does_not_share_scenario_mutations() {
        let core = ObjDefSet::parse_sources(
            &FeaturesConfig::default().compile_options(),
            None,
            None,
            [ObjDefSource::new(
                "fixture",
                "object #1\nname: \"Original\"\nowner: #-1\nreadable: true\nendobject\n",
            )],
        )
        .unwrap();
        let first = load_database(&core).unwrap();
        let mut transaction = first.loader_client().unwrap();
        transaction
            .set_object_name(&Obj::mk_id(1), "Changed".to_owned())
            .unwrap();
        transaction.commit().unwrap();

        let second = load_database(&core).unwrap();
        let permissions = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
        for (database, expected) in [(first, "Changed"), (second, "Original")] {
            let transaction = database.new_world_state().unwrap();
            assert_eq!(
                transaction.name_of(&permissions, &Obj::mk_id(1)).unwrap(),
                expected
            );
        }
    }
}
