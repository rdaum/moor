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

//! Mock scheduler with declarative scenarios for testing daemon components

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, SystemTime},
};
use uuid::Uuid;

use flume::{Receiver, Sender};
use moor_common::tasks::SchedulerError;
use moor_var::{Obj, Var, v_obj, v_str};

#[cfg(test)]
use moor_common::model::ObjectRef;
#[cfg(test)]
use moor_var::Symbol;

use crate::tasks::{
    TaskHandle, TaskNotification,
    scheduler_client::{SchedulerClient, SchedulerClientMsg},
    workers::{WorkerRequest, WorkerResponse},
};

#[derive(Debug, Clone, PartialEq)]
pub enum MockScenario {
    /// Normal operation - fast responses, everything works
    NormalOperation,
    /// System under load - higher latency but operations succeed
    OverloadedSystem,
    /// Authentication issues - login attempts fail
    LoginFailures,
    /// Worker shortage - no workers available for requests
    WorkerShortage,
    /// Partial system outage - some operations fail randomly
    PartialOutage,
    /// Database connectivity issues - property/verb operations fail
    DatabaseIssues,
    /// Network partition simulation - intermittent failures
    NetworkPartition,
    /// System degrading over time - performance gets worse
    GradualDegradation,
    /// System recovering - performance improving over time
    RecoveryMode,
    /// Custom scenario with user-defined behavior
    Custom(ScenarioConfig),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioConfig {
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub success_rate: f32, // 0.0 to 1.0
    pub login_success_rate: f32,
    pub worker_availability_rate: f32,
    pub property_lookup_success_rate: f32,
    pub verb_lookup_success_rate: f32,
    pub task_completion_rate: f32,
    pub degradation_factor: f32, // For scenarios that change over time
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            success_rate: 1.0,
            login_success_rate: 1.0,
            worker_availability_rate: 1.0,
            property_lookup_success_rate: 1.0,
            verb_lookup_success_rate: 1.0,
            task_completion_rate: 1.0,
            degradation_factor: 1.0,
        }
    }
}

impl MockScenario {
    pub fn config(&self) -> ScenarioConfig {
        match self {
            MockScenario::NormalOperation => ScenarioConfig::default(),

            MockScenario::OverloadedSystem => ScenarioConfig {
                base_delay: Duration::from_millis(100),
                max_delay: Duration::from_millis(2000),
                success_rate: 0.95,
                ..Default::default()
            },

            MockScenario::LoginFailures => ScenarioConfig {
                login_success_rate: 0.2,
                ..Default::default()
            },

            MockScenario::WorkerShortage => ScenarioConfig {
                worker_availability_rate: 0.3,
                base_delay: Duration::from_millis(500),
                max_delay: Duration::from_millis(5000),
                ..Default::default()
            },

            MockScenario::PartialOutage => ScenarioConfig {
                success_rate: 0.7,
                login_success_rate: 0.8,
                property_lookup_success_rate: 0.6,
                verb_lookup_success_rate: 0.6,
                task_completion_rate: 0.75,
                base_delay: Duration::from_millis(200),
                max_delay: Duration::from_millis(3000),
                ..Default::default()
            },

            MockScenario::DatabaseIssues => ScenarioConfig {
                property_lookup_success_rate: 0.4,
                verb_lookup_success_rate: 0.4,
                base_delay: Duration::from_millis(50),
                max_delay: Duration::from_millis(1000),
                ..Default::default()
            },

            MockScenario::NetworkPartition => ScenarioConfig {
                success_rate: 0.6,
                base_delay: Duration::from_millis(100),
                max_delay: Duration::from_millis(8000),
                ..Default::default()
            },

            MockScenario::GradualDegradation => ScenarioConfig {
                base_delay: Duration::from_millis(20),
                max_delay: Duration::from_millis(500),
                success_rate: 0.9,
                degradation_factor: 1.1, // 10% worse each operation
                ..Default::default()
            },

            MockScenario::RecoveryMode => ScenarioConfig {
                base_delay: Duration::from_millis(200),
                max_delay: Duration::from_millis(1000),
                success_rate: 0.8,
                degradation_factor: 0.95, // 5% better each operation
                ..Default::default()
            },

            MockScenario::Custom(config) => config.clone(),
        }
    }
}

pub struct MockScheduler {
    scheduler_sender: Sender<SchedulerClientMsg>,
    scheduler_receiver: Receiver<SchedulerClientMsg>,

    // Scenario configuration
    current_scenario: Arc<RwLock<MockScenario>>,
    scenario_start_time: Arc<RwLock<SystemTime>>,
    operation_count: Arc<Mutex<u64>>,
    next_task_id: Arc<Mutex<usize>>,

    // Worker management
    worker_sender: Sender<WorkerResponse>,
    worker_receiver: Option<Receiver<WorkerResponse>>,
    pending_worker_requests: Arc<Mutex<Vec<WorkerRequest>>>,

    // Default values
    default_login_player: Obj,

    // Custom overrides (still available for fine-grained control)
    custom_responses: Arc<RwLock<HashMap<String, Var>>>,
}

impl Default for MockScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl MockScheduler {
    pub fn new() -> Self {
        let (scheduler_sender, scheduler_receiver) = flume::unbounded();
        let (worker_sender, worker_receiver) = flume::unbounded();

        Self {
            scheduler_sender,
            scheduler_receiver,
            current_scenario: Arc::new(RwLock::new(MockScenario::NormalOperation)),
            scenario_start_time: Arc::new(RwLock::new(SystemTime::now())),
            operation_count: Arc::new(Mutex::new(0)),
            next_task_id: Arc::new(Mutex::new(1)),
            worker_sender,
            worker_receiver: Some(worker_receiver),
            pending_worker_requests: Arc::new(Mutex::new(Vec::new())),
            default_login_player: Obj::mk_id(1),
            custom_responses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn client(&self) -> SchedulerClient {
        SchedulerClient::new(self.scheduler_sender.clone())
    }

    pub fn worker_sender(&self) -> Sender<WorkerResponse> {
        self.worker_sender.clone()
    }

    pub fn take_worker_receiver(&mut self) -> Option<Receiver<WorkerResponse>> {
        self.worker_receiver.take()
    }

    // Scenario management

    pub fn set_scenario(&self, scenario: MockScenario) {
        *self.current_scenario.write().unwrap() = scenario;
        *self.scenario_start_time.write().unwrap() = SystemTime::now();
        *self.operation_count.lock().unwrap() = 0;
        *self.next_task_id.lock().unwrap() = 1;
    }

    pub fn get_current_scenario(&self) -> MockScenario {
        self.current_scenario.read().unwrap().clone()
    }

    pub fn set_default_login_player(&mut self, player: Obj) {
        self.default_login_player = player;
    }

    // Override specific responses (for fine-grained test control)
    pub fn set_custom_response(&self, key: &str, value: Var) {
        self.custom_responses
            .write()
            .unwrap()
            .insert(key.to_string(), value);
    }

    // Query methods for testing

    pub fn get_operation_count(&self) -> u64 {
        *self.operation_count.lock().unwrap()
    }

    pub fn get_scenario_duration(&self) -> Duration {
        self.scenario_start_time
            .read()
            .unwrap()
            .elapsed()
            .unwrap_or(Duration::ZERO)
    }

    pub fn get_pending_worker_requests(&self) -> usize {
        self.pending_worker_requests.lock().unwrap().len()
    }

    // Simulation control

    pub fn simulate_worker_request(&self, request: WorkerRequest) {
        self.pending_worker_requests.lock().unwrap().push(request);
    }

    pub fn simulate_worker_response(&self, request_id: Uuid, response: Var) {
        let worker_response = WorkerResponse::Response {
            request_id,
            response,
        };
        self.worker_sender.send(worker_response).ok();
    }

    pub fn simulate_worker_error(&self, request_id: Uuid, error: moor_common::tasks::WorkerError) {
        let worker_response = WorkerResponse::Error { request_id, error };
        self.worker_sender.send(worker_response).ok();
    }

    // Main processing loop

    pub fn run(&self) {
        while let Ok(msg) = self.scheduler_receiver.recv() {
            self.handle_scheduler_message(msg);
        }
    }

    pub fn run_with_timeout(&self, timeout: Duration) -> Result<(), flume::RecvTimeoutError> {
        match self.scheduler_receiver.recv_timeout(timeout) {
            Ok(msg) => {
                self.handle_scheduler_message(msg);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn increment_operation_count(&self) {
        *self.operation_count.lock().unwrap() += 1;
    }

    fn next_task_id(&self) -> usize {
        let mut id = self.next_task_id.lock().unwrap();
        let current = *id;
        *id += 1;
        current
    }

    fn should_succeed(&self, base_rate: f32) -> bool {
        #[cfg(test)]
        {
            use rand::Rng;
            let config = self.current_scenario.read().unwrap().config();
            let mut rate = base_rate;

            // Apply degradation/improvement factor for time-based scenarios
            if config.degradation_factor != 1.0 {
                let operations = *self.operation_count.lock().unwrap() as f32;
                rate *= config.degradation_factor.powf(operations / 10.0); // Apply every 10 operations
            }

            rand::thread_rng().r#gen::<f32>() < rate.clamp(0.0, 1.0)
        }
        #[cfg(not(test))]
        {
            // Simple deterministic behavior for non-test builds
            base_rate >= 0.5
        }
    }

    fn get_delay(&self) -> Duration {
        #[cfg(test)]
        {
            use rand::Rng;
            let config = self.current_scenario.read().unwrap().config();
            let range = config.max_delay.as_millis() - config.base_delay.as_millis();
            if range > 0 {
                let random_delay = rand::thread_rng().gen_range(0..=range);
                config.base_delay + Duration::from_millis(random_delay as u64)
            } else {
                config.base_delay
            }
        }
        #[cfg(not(test))]
        {
            let config = self.current_scenario.read().unwrap().config();
            config.base_delay
        }
    }

    fn create_mock_task_handle(
        &self,
        task_id: usize,
        delay: Duration,
        result: Result<TaskNotification, SchedulerError>,
    ) -> TaskHandle {
        let (sender, receiver) = flume::unbounded();

        // Simulate delayed completion
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            sender.send((task_id, result)).ok();
        });

        TaskHandle::new_mock(task_id, receiver)
    }

    fn handle_scheduler_message(&self, msg: SchedulerClientMsg) {
        self.increment_operation_count();
        let config = self.current_scenario.read().unwrap().config();

        match msg {
            SchedulerClientMsg::SubmitCommandTask {
                handler_object: _,
                player: _,
                command,
                session: _,
                reply,
            } => {
                let task_id = self.next_task_id();

                if !self.should_succeed(config.success_rate) {
                    reply
                        .send(Err(SchedulerError::CommandExecutionError(
                            moor_common::tasks::CommandError::NoCommandMatch,
                        )))
                        .ok();
                    return;
                }

                let delay = self.get_delay();
                let result = if self.should_succeed(config.task_completion_rate) {
                    Ok(TaskNotification::Result(v_str(&format!(
                        "Command '{command}' executed"
                    ))))
                } else {
                    Err(SchedulerError::CommandExecutionError(
                        moor_common::tasks::CommandError::PermissionDenied,
                    ))
                };

                let task_handle = self.create_mock_task_handle(task_id, delay, result);
                reply.send(Ok(task_handle)).ok();
            }

            SchedulerClientMsg::SubmitVerbTask {
                player: _,
                vloc: _,
                verb,
                args: _,
                argstr: _,
                perms: _,
                session: _,
                reply,
            } => {
                let task_id = self.next_task_id();
                let delay = self.get_delay();

                // Special handling for login
                if verb.as_string() == "do_login_command" {
                    let result = if self.should_succeed(config.login_success_rate) {
                        Ok(TaskNotification::Result(v_obj(self.default_login_player)))
                    } else {
                        Ok(TaskNotification::Result(v_str("Invalid credentials")))
                    };

                    let task_handle = self.create_mock_task_handle(task_id, delay, result);
                    reply.send(Ok(task_handle)).ok();
                    return;
                }

                if !self.should_succeed(config.verb_lookup_success_rate) {
                    reply.send(Err(SchedulerError::TaskNotFound(task_id))).ok();
                    return;
                }

                let result = Ok(TaskNotification::Result(v_str(&format!(
                    "Verb '{verb}' executed"
                ))));
                let task_handle = self.create_mock_task_handle(task_id, delay, result);
                reply.send(Ok(task_handle)).ok();
            }

            SchedulerClientMsg::ExecuteWorldStateActions {
                actions,
                rollback: _,
                reply,
            } => {
                use crate::tasks::world_state_action::{WorldStateResponse, WorldStateResult};

                let mut responses = Vec::new();

                for request in actions {
                    // For now, simulate simple success responses for the mock
                    let result = match &request.action {
                        crate::tasks::world_state_action::WorldStateAction::RequestSystemProperty { property, .. } => {
                            let value = self
                                .custom_responses
                                .read()
                                .unwrap()
                                .get(&format!("sysprop_{property}"))
                                .cloned()
                                .unwrap_or(v_str(&format!("system_prop_{property}")));
                            WorldStateResult::SystemProperty(value)
                        }
                        _ => {
                            // For other actions, return a simple success
                            WorldStateResult::SystemProperty(v_str("mock_response"))
                        }
                    };

                    responses.push(WorldStateResponse::Success {
                        id: request.id,
                        result,
                    });
                }

                reply.send(Ok(responses)).ok();
            }

            SchedulerClientMsg::Checkpoint(_, reply) => {
                reply.send(Ok(())).ok();
            }

            SchedulerClientMsg::Shutdown(_, reply) => {
                reply.send(Ok(())).ok();
            }

            SchedulerClientMsg::CheckStatus(reply) => {
                reply.send(Ok(())).ok();
            }

            // For simplicity, handle remaining message types with basic success responses
            _ => {
                // Most other operations just succeed in this simplified mock
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::tasks::Session;
    use std::{thread, time::Duration};

    // Simple test session implementation
    struct TestSession;

    impl Session for TestSession {
        fn send_event(
            &self,
            _player: Obj,
            _event: Box<moor_common::tasks::NarrativeEvent>,
        ) -> Result<(), moor_common::tasks::SessionError> {
            Ok(())
        }

        fn request_input(
            &self,
            _player: Obj,
            _request_id: Uuid,
        ) -> Result<(), moor_common::tasks::SessionError> {
            Ok(())
        }

        fn commit(&self) -> Result<(), moor_common::tasks::SessionError> {
            Ok(())
        }

        fn rollback(&self) -> Result<(), moor_common::tasks::SessionError> {
            Ok(())
        }

        fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, moor_common::tasks::SessionError> {
            Ok(self)
        }

        fn send_system_msg(
            &self,
            _player: Obj,
            _msg: &str,
        ) -> Result<(), moor_common::tasks::SessionError> {
            Ok(())
        }

        fn notify_shutdown(
            &self,
            _msg: Option<String>,
        ) -> Result<(), moor_common::tasks::SessionError> {
            Ok(())
        }

        fn connection_name(
            &self,
            _player: Obj,
        ) -> Result<String, moor_common::tasks::SessionError> {
            Ok("test_connection".to_string())
        }

        fn disconnect(&self, _player: Obj) -> Result<(), moor_common::tasks::SessionError> {
            Ok(())
        }

        fn connected_players(&self) -> Result<Vec<Obj>, moor_common::tasks::SessionError> {
            Ok(vec![])
        }

        fn connected_seconds(&self, _player: Obj) -> Result<f64, moor_common::tasks::SessionError> {
            Ok(0.0)
        }

        fn idle_seconds(&self, _player: Obj) -> Result<f64, moor_common::tasks::SessionError> {
            Ok(0.0)
        }

        fn connections(
            &self,
            _player: Option<Obj>,
        ) -> Result<Vec<Obj>, moor_common::tasks::SessionError> {
            Ok(vec![])
        }

        fn connection_details(
            &self,
            _player: Option<Obj>,
        ) -> Result<Vec<moor_common::tasks::ConnectionDetails>, moor_common::tasks::SessionError>
        {
            Ok(vec![])
        }

        fn connection_attributes(
            &self,
            _player: Obj,
        ) -> Result<Var, moor_common::tasks::SessionError> {
            use moor_var::v_list;
            Ok(v_list(&[]))
        }

        fn set_connection_attribute(
            &self,
            _connection_obj: Obj,
            _key: Symbol,
            _value: Var,
        ) -> Result<(), moor_common::tasks::SessionError> {
            Ok(())
        }
    }

    #[test]
    fn test_mock_scheduler_basic_operation() {
        let scheduler = Arc::new(MockScheduler::new());
        let client = scheduler.client();

        // Start scheduler processing in background
        let scheduler_clone = scheduler.clone();
        let handle = thread::spawn(move || {
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_secs(2) {
                if scheduler_clone
                    .run_with_timeout(Duration::from_millis(100))
                    .is_err()
                {
                    break;
                }
            }
        });

        // Test command submission
        let session = Arc::new(TestSession);
        let task_handle = client
            .submit_command_task(&Obj::mk_id(1), &Obj::mk_id(2), "test command", session)
            .expect("Command submission should succeed");

        // Task should complete
        let result = task_handle
            .into_receiver()
            .recv_timeout(Duration::from_millis(500))
            .expect("Task should complete within timeout");

        assert!(result.1.is_ok(), "Task should succeed in normal operation");

        handle.join().ok();
    }

    #[test]
    fn test_scenario_switching() {
        let scheduler = MockScheduler::new();

        assert_eq!(
            scheduler.get_current_scenario(),
            MockScenario::NormalOperation
        );

        scheduler.set_scenario(MockScenario::OverloadedSystem);
        assert_eq!(
            scheduler.get_current_scenario(),
            MockScenario::OverloadedSystem
        );

        scheduler.set_scenario(MockScenario::LoginFailures);
        assert_eq!(
            scheduler.get_current_scenario(),
            MockScenario::LoginFailures
        );
    }

    #[test]
    fn test_property_requests() {
        let scheduler = Arc::new(MockScheduler::new());
        let client = scheduler.client();

        // Start scheduler processing
        let scheduler_clone = scheduler.clone();
        let handle = thread::spawn(move || {
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_secs(3) {
                if scheduler_clone
                    .run_with_timeout(Duration::from_millis(100))
                    .is_err()
                {
                    break;
                }
            }
        });

        // Give scheduler a moment to start
        std::thread::sleep(Duration::from_millis(10));

        // Test property request
        let result = client.request_system_property(
            &Obj::mk_id(1),
            &ObjectRef::Id(Obj::mk_id(1)),
            Symbol::mk("test_prop"),
        );

        // Should succeed in normal operation
        assert!(
            result.is_ok(),
            "Property request should succeed: {result:?}"
        );

        handle.join().ok();
    }

    #[test]
    fn test_worker_simulation() {
        let mut scheduler = MockScheduler::new();
        let worker_receiver = scheduler.take_worker_receiver().unwrap();

        let request_id = Uuid::new_v4();
        let expected_response = v_str("worker computation result");

        // Simulate worker responding to a request
        scheduler.simulate_worker_response(request_id, expected_response.clone());

        // Verify the response was sent
        let response = worker_receiver
            .recv_timeout(Duration::from_millis(100))
            .expect("Should receive worker response");

        match response {
            WorkerResponse::Response {
                request_id: resp_id,
                response,
            } => {
                assert_eq!(resp_id, request_id);
                assert_eq!(response, expected_response);
            }
            _ => panic!("Expected WorkerResponse::Response"),
        }
    }
}
