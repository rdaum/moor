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

use std::sync::{Arc, Mutex};

use flume::Sender;
use uuid::Uuid;

use moor_common::tasks::{ConnectionDetails, NarrativeEvent, Session, SessionError};
use moor_var::{ByteSized, List, Obj, Symbol, Var};

use crate::{
    event_log::EventLogOps,
    rpc::{session::SessionActions, session_event_buffer::SessionEventBuffer},
};

/// Most events a single captured invocation will accumulate before the task is aborted.
pub const MAX_CAPTURED_EVENTS: usize = 10_000;

/// Most bytes of event payload a single captured invocation will accumulate before the task is
/// aborted. Counted from the same estimate the database uses for a value's size, so it tracks the
/// payload rather than the encoded form exactly.
pub const MAX_CAPTURED_BYTES: usize = 8 * 1024 * 1024;

/// The output a captured invocation has accumulated, shared by the root task's session and by any
/// session standing in for that task after a conflict retry.
#[derive(Default)]
struct CaptureBuffer {
    events: Vec<(Obj, Box<NarrativeEvent>)>,
    bytes: usize,
}

/// What one transaction has spooled towards the limits but not yet committed.
#[derive(Default)]
struct Pending {
    events: usize,
    bytes: usize,
}

/// A shared, bounded accumulator for a captured invocation's output.
#[derive(Default)]
pub struct CaptureAccumulator {
    buffer: Mutex<CaptureBuffer>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureLimit {
    Events(usize),
    Bytes(usize),
}

impl CaptureAccumulator {
    /// Which limit `additional_events`/`additional_bytes` would exceed on top of committed output.
    fn exceeded_limit(&self, pending_events: usize, pending_bytes: usize) -> Option<CaptureLimit> {
        let buffer = self.buffer.lock().unwrap();
        if buffer.events.len() + pending_events > MAX_CAPTURED_EVENTS {
            return Some(CaptureLimit::Events(MAX_CAPTURED_EVENTS));
        }
        if buffer.bytes + pending_bytes > MAX_CAPTURED_BYTES {
            return Some(CaptureLimit::Bytes(MAX_CAPTURED_BYTES));
        }
        None
    }

    fn extend(&self, events: Vec<(Obj, Box<NarrativeEvent>)>, bytes: usize) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.bytes += bytes;
        buffer.events.extend(events);
    }

    /// Take everything accumulated so far.
    pub fn take(&self) -> Vec<(Obj, Box<NarrativeEvent>)> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.bytes = 0;
        std::mem::take(&mut buffer.events)
    }
}

/// A session for a verb invocation that has no connection behind it, whose output is returned to
/// the caller instead of being delivered to a client.
///
/// Only events addressed to the invoking principal are captured. A verb that notifies some *other*
/// player is doing ordinary world-visible work, so those events are published to that player's
/// connections exactly as they would be from a connected session; capture mode changes who is
/// listening for the caller, not the reach of the verb.
///
/// Events are buffered per transaction exactly as they are for `RpcSession`: on commit they are
/// written to the event log and then either captured or published, and on rollback they are
/// discarded.
///
/// A forked task is a new task which may outlive the captured response, so it gets a non-capturing
/// session and its output is published normally. A *retry* of the root task after a commit
/// conflict is the same task running again, so it shares the accumulator (see `fork_retry`).
pub struct OutputCaptureSession {
    client_id: Uuid,
    /// The invoking principal. Events addressed here are captured; others are published.
    player: Obj,
    /// Buffered events and the shared event-log commit rules.
    events: SessionEventBuffer,
    /// The accumulator for the caller's output, or `None` for a session whose output is published
    /// rather than captured (a forked task).
    capture: Option<Arc<CaptureAccumulator>>,
    /// What the current transaction has spooled, counted so the limits can be enforced when an
    /// event is sent rather than only once it commits. A verb that never commits would otherwise
    /// accumulate without bound.
    pending: Mutex<Pending>,
    send: Sender<SessionActions>,
}

impl OutputCaptureSession {
    /// A capturing session for the root task of a captured invocation.
    pub fn new(
        client_id: Uuid,
        player: Obj,
        event_log: Arc<dyn EventLogOps>,
        send: Sender<SessionActions>,
    ) -> Self {
        Self::with_capture(
            client_id,
            player,
            event_log,
            send,
            Some(Arc::new(CaptureAccumulator::default())),
        )
    }

    fn with_capture(
        client_id: Uuid,
        player: Obj,
        event_log: Arc<dyn EventLogOps>,
        send: Sender<SessionActions>,
        capture: Option<Arc<CaptureAccumulator>>,
    ) -> Self {
        Self {
            client_id,
            player,
            events: SessionEventBuffer::new(event_log, player, player),
            capture,
            pending: Mutex::new(Pending::default()),
            send,
        }
    }

    /// The accumulator holding this invocation's output, so the caller can read it once the task
    /// finishes even if the session it started with was replaced by a retry.
    pub fn accumulator(&self) -> Option<Arc<CaptureAccumulator>> {
        self.capture.clone()
    }

    /// Take the events captured for the caller so far. The daemon reads the accumulator directly,
    /// since a retry can replace the session; this is here for tests that hold one session.
    #[cfg(test)]
    pub fn take_captured_events(&self) -> Vec<(Obj, Box<NarrativeEvent>)> {
        self.capture.as_ref().map(|c| c.take()).unwrap_or_default()
    }
}

/// The event payload size used to bound how much a captured invocation may accumulate.
fn event_size_bytes(event: &NarrativeEvent) -> usize {
    event.size_bytes()
}

impl Session for OutputCaptureSession {
    fn commit(&self) -> Result<(), SessionError> {
        // Writes both buffers to the event log and hands back the deliverable events.
        let events = self.events.commit();
        let pending = std::mem::take(&mut *self.pending.lock().unwrap());

        let Some(capture) = self.capture.as_ref() else {
            // Not capturing for anyone: deliver everything the way a connected session would.
            return self.publish(events);
        };

        // Split the caller's own output from anything addressed elsewhere. The former is the
        // response; the latter is ordinary delivery.
        let (captured, to_publish): (Vec<_>, Vec<_>) = events
            .into_iter()
            .partition(|(player, _)| *player == self.player);

        capture.extend(captured, pending.bytes);
        self.publish(to_publish)
    }

    fn rollback(&self) -> Result<(), SessionError> {
        // Events buffered by the rolled-back transaction are dropped: they are neither logged,
        // captured, nor published. Events from earlier committed transactions are kept.
        self.events.rollback();
        *self.pending.lock().unwrap() = Pending::default();
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        // A fork is a new task and may outlive the captured response, so its output is published
        // rather than captured.
        Ok(Arc::new(Self::with_capture(
            self.client_id,
            self.player,
            self.events.event_log(),
            self.send.clone(),
            None,
        )))
    }

    fn fork_retry(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        // The same task running again after a commit conflict. It stands in for the original, so
        // it keeps accumulating into the same buffer; the rolled-back attempt's events were
        // already discarded by `rollback`.
        Ok(Arc::new(Self::with_capture(
            self.client_id,
            self.player,
            self.events.event_log(),
            self.send.clone(),
            self.capture.clone(),
        )))
    }

    fn request_input(
        &self,
        _player: Obj,
        _input_request_id: Uuid,
        _metadata: Option<Vec<(Symbol, Var)>>,
    ) -> Result<(), SessionError> {
        // Captured mode has no input channel, so an input request fails the task.
        Err(SessionError::CommitError(
            "Input requests not supported for output capture sessions".to_string(),
        ))
    }

    fn send_event(&self, player: Obj, event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        if self.capture.is_none() || player != self.player {
            self.events.push_event(player, event);
            return Ok(());
        }

        let size = event_size_bytes(&event);
        let capture = self.capture.as_ref().expect("capture checked above");
        let mut pending = self.pending.lock().unwrap();
        if let Some(limit) = capture.exceeded_limit(pending.events + 1, pending.bytes + size) {
            return Err(match limit {
                CaptureLimit::Events(events) => SessionError::OutputEventLimitExceeded(events),
                CaptureLimit::Bytes(bytes) => SessionError::OutputByteLimitExceeded(bytes),
            });
        }
        pending.events += 1;
        pending.bytes += size;
        self.events.push_event(player, event);
        Ok(())
    }

    fn log_event(&self, player: Obj, event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        // Log-only events go to the event log on commit, but never into the captured response.
        self.events.push_log_only_event(player, event);
        Ok(())
    }

    fn send_system_msg(&self, _player: Obj, _msg: &str) -> Result<(), SessionError> {
        // A system message is addressed to a connection, and this invocation has none; discard.
        Ok(())
    }

    fn notify_shutdown(&self, _msg: Option<String>) -> Result<(), SessionError> {
        // No connected destination; discard.
        Ok(())
    }

    fn connection_name(&self, _player: Obj) -> Result<String, SessionError> {
        Ok("output-capture-session".to_string())
    }

    fn disconnect(&self, _player: Obj) -> Result<(), SessionError> {
        Ok(())
    }

    fn connected_players(&self, _include_all: bool) -> Result<Vec<Obj>, SessionError> {
        Ok(vec![])
    }

    fn connected_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn idle_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn connections(&self, _player: Option<Obj>) -> Result<Vec<Obj>, SessionError> {
        Ok(vec![])
    }

    fn connection_details(
        &self,
        _player: Option<Obj>,
    ) -> Result<Vec<ConnectionDetails>, SessionError> {
        Ok(vec![])
    }

    fn connection_attributes(&self, _obj: Obj) -> Result<Var, SessionError> {
        Ok(Var::from(List::mk_list(&[])))
    }

    fn set_connection_attribute(
        &self,
        _connection_obj: Obj,
        _key: Symbol,
        _value: Var,
    ) -> Result<(), SessionError> {
        Ok(())
    }
}

impl OutputCaptureSession {
    fn publish(&self, events: Vec<(Obj, Box<NarrativeEvent>)>) -> Result<(), SessionError> {
        if events.is_empty() {
            return Ok(());
        }
        self.send
            .send(SessionActions::PublishNarrativeEvents(events))
            .map_err(|e| SessionError::CommitError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::MockEventLog;
    use moor_common::tasks::{Event, Exception, NarrativeEvent};
    use moor_var::{E_INVARG, v_obj, v_str};

    const CALLER: i32 = 2;
    const OTHER: i32 = 3;

    fn event(msg: &str) -> Box<NarrativeEvent> {
        Box::new(NarrativeEvent::notify(
            v_obj(Obj::mk_id(1)),
            v_str(msg),
            None,
            false,
            false,
            None,
        ))
    }

    struct Harness {
        session: Arc<OutputCaptureSession>,
        log: Arc<MockEventLog>,
        published: flume::Receiver<SessionActions>,
    }

    impl Harness {
        fn new() -> Self {
            let log = Arc::new(MockEventLog::new());
            let (tx, rx) = flume::unbounded();
            let caller = Obj::mk_id(CALLER);
            log.set_pubkey(caller, test_pubkey());
            log.set_pubkey(Obj::mk_id(OTHER), test_pubkey());
            let session = Arc::new(OutputCaptureSession::new(
                Uuid::new_v4(),
                caller,
                log.clone(),
                tx,
            ));
            Self {
                session,
                log,
                published: rx,
            }
        }

        /// The events handed to the publishing mailbox, flattened.
        fn published(&self) -> Vec<(Obj, Box<NarrativeEvent>)> {
            let mut all = vec![];
            while let Ok(action) = self.published.try_recv() {
                if let SessionActions::PublishNarrativeEvents(events) = action {
                    all.extend(events);
                }
            }
            all
        }
    }

    /// An age/X25519 recipient key; the event log encrypts every event it stores.
    fn test_pubkey() -> String {
        "age1zvkyg2lqzraa2lnjvqej32nkuu0ues2s82hzrye869xeexvn73equnujwj".to_string()
    }

    fn messages(events: &[(Obj, Box<NarrativeEvent>)]) -> Vec<String> {
        events
            .iter()
            .filter_map(|(_, e)| match &e.event {
                Event::Notify { value, .. } => value.as_string().map(|s| s.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn captures_only_committed_send_events() {
        let h = Harness::new();
        let caller = Obj::mk_id(CALLER);

        h.session.send_event(caller, event("kept")).unwrap();
        h.session.log_event(caller, event("log-only")).unwrap();
        h.session.commit().unwrap();

        assert_eq!(messages(&h.session.take_captured_events()), vec!["kept"]);
        // Both events were written to the event log; only the send_event was captured.
        assert_eq!(h.log.narrative_event_count(), 2);
    }

    #[test]
    fn uncommitted_events_are_not_captured() {
        let h = Harness::new();

        h.session
            .send_event(Obj::mk_id(CALLER), event("pending"))
            .unwrap();

        assert!(h.session.take_captured_events().is_empty());
        assert_eq!(h.log.narrative_event_count(), 0);
    }

    #[test]
    fn rollback_drops_buffered_events() {
        let h = Harness::new();
        let caller = Obj::mk_id(CALLER);

        h.session.send_event(caller, event("doomed")).unwrap();
        h.session.rollback().unwrap();
        h.session.commit().unwrap();

        assert!(h.session.take_captured_events().is_empty());
        assert!(h.published().is_empty());
        assert_eq!(h.log.narrative_event_count(), 0);
    }

    /// A verb invoked in capture mode can still talk to other players; only the caller's own
    /// output is diverted into the response.
    #[test]
    fn output_for_another_player_is_published_not_captured() {
        let h = Harness::new();

        h.session
            .send_event(Obj::mk_id(CALLER), event("for me"))
            .unwrap();
        h.session
            .send_event(Obj::mk_id(OTHER), event("for them"))
            .unwrap();
        h.session.commit().unwrap();

        assert_eq!(messages(&h.session.take_captured_events()), vec!["for me"]);
        let published = h.published();
        assert_eq!(messages(&published), vec!["for them"]);
        assert_eq!(published[0].0, Obj::mk_id(OTHER));
        // Both still reach the event log.
        assert_eq!(h.log.narrative_event_count(), 2);
    }

    /// A forked task is a new task; its output goes to connections the ordinary way rather than
    /// into a response that has likely already been sent.
    #[test]
    fn fork_output_is_published_and_logged_but_not_captured() {
        let h = Harness::new();
        let caller = Obj::mk_id(CALLER);

        let forked = h.session.clone().fork().unwrap();
        forked.send_event(caller, event("from fork")).unwrap();
        forked.commit().unwrap();

        assert!(h.session.take_captured_events().is_empty());
        assert_eq!(messages(&h.published()), vec!["from fork"]);
        assert_eq!(h.log.narrative_event_count(), 1);
    }

    /// A commit conflict re-runs the root task against a forked session. That session stands in
    /// for the original, so output from the successful attempt must still reach the caller.
    #[test]
    fn a_retry_keeps_accumulating_into_the_callers_buffer() {
        let h = Harness::new();
        let caller = Obj::mk_id(CALLER);

        // First attempt commits some output, then a later transaction conflicts and rolls back.
        h.session.send_event(caller, event("first")).unwrap();
        h.session.commit().unwrap();
        h.session.send_event(caller, event("lost")).unwrap();
        h.session.rollback().unwrap();

        // The scheduler re-runs the task with a retry fork.
        let retried = h.session.clone().fork_retry().unwrap();
        retried.send_event(caller, event("second")).unwrap();
        retried.commit().unwrap();

        // The caller sees both committed attempts and not the rolled-back one, whichever session
        // object it happens to ask.
        assert_eq!(
            messages(&h.session.take_captured_events()),
            vec!["first", "second"]
        );
    }

    #[test]
    fn input_requests_fail() {
        let h = Harness::new();

        assert!(
            h.session
                .request_input(Obj::mk_id(CALLER), Uuid::new_v4(), None)
                .is_err()
        );
    }

    #[test]
    fn system_messages_are_discarded() {
        let h = Harness::new();

        h.session
            .send_system_msg(Obj::mk_id(CALLER), "hello")
            .unwrap();
        h.session.notify_shutdown(None).unwrap();
        h.session.commit().unwrap();

        assert!(h.session.take_captured_events().is_empty());
        assert!(h.published().is_empty());
        assert_eq!(h.log.narrative_event_count(), 0);
    }

    /// The response is held in memory with nobody draining it, so a verb that produces output
    /// without limit has to be stopped rather than allowed to exhaust the daemon.
    #[test]
    fn output_past_the_event_limit_fails_the_task() {
        let h = Harness::new();
        let caller = Obj::mk_id(CALLER);

        for i in 0..MAX_CAPTURED_EVENTS {
            h.session
                .send_event(caller, event(&format!("event {i}")))
                .expect("within the limit");
        }

        let err = h
            .session
            .send_event(caller, event("one too many"))
            .expect_err("over the limit");
        assert!(matches!(
            err,
            SessionError::OutputEventLimitExceeded(MAX_CAPTURED_EVENTS)
        ));
    }

    #[test]
    fn output_past_the_byte_limit_fails_the_task() {
        let h = Harness::new();
        let caller = Obj::mk_id(CALLER);
        let big = "x".repeat(1024 * 1024);

        let mut sent = 0;
        loop {
            match h.session.send_event(caller, event(&big)) {
                Ok(()) => sent += 1,
                Err(err) => {
                    assert!(matches!(
                        err,
                        SessionError::OutputByteLimitExceeded(MAX_CAPTURED_BYTES)
                    ));
                    break;
                }
            }
            assert!(sent < MAX_CAPTURED_EVENTS, "byte limit should bite first");
        }
    }

    #[test]
    fn traceback_size_includes_rich_error_and_backtrace_values() {
        let payload = "x".repeat(MAX_CAPTURED_BYTES);
        let traceback = NarrativeEvent {
            event_id: Uuid::now_v7(),
            timestamp: std::time::SystemTime::now(),
            author: v_obj(Obj::mk_id(1)),
            event: Event::Traceback(Exception {
                error: E_INVARG.with_msg(|| payload.clone()),
                stack: vec![],
                backtrace: vec![v_str(&payload)],
            }),
        };

        assert!(event_size_bytes(&traceback) > MAX_CAPTURED_BYTES * 2);
    }

    /// The limit spans the whole invocation, not one transaction of it.
    #[test]
    fn the_event_limit_spans_committed_transactions() {
        let h = Harness::new();
        let caller = Obj::mk_id(CALLER);

        for i in 0..MAX_CAPTURED_EVENTS {
            h.session
                .send_event(caller, event(&format!("event {i}")))
                .unwrap();
            if i % 1000 == 0 {
                h.session.commit().unwrap();
            }
        }
        h.session.commit().unwrap();

        assert!(h.session.send_event(caller, event("over")).is_err());
    }

    /// A fork publishes rather than captures, so it is not subject to the caller's budget.
    #[test]
    fn a_fork_is_not_bounded_by_the_capture_limit() {
        let h = Harness::new();
        let caller = Obj::mk_id(CALLER);

        for i in 0..MAX_CAPTURED_EVENTS {
            h.session
                .send_event(caller, event(&format!("event {i}")))
                .unwrap();
        }
        h.session.commit().unwrap();

        let forked = h.session.clone().fork().unwrap();
        forked.send_event(caller, event("fine")).unwrap();
        forked.commit().unwrap();
    }
}
