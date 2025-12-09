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

//! Testing utilities and mocks for the kernel crate

pub mod mock_scheduler;
pub mod scheduler_test_utils;
pub mod vm_test;
pub mod vm_test_utils;

pub use mock_scheduler::{MockScenario, MockScheduler};
pub use scheduler_test_utils::{ExecResult as SchedulerExecResult, call_command, call_eval};
pub use vm_test_utils::{
    ActivationBenchResult, ExecResult as VmExecResult, call_eval_builtin, call_fork, call_verb,
    create_activation_for_bench, create_nested_activation_for_bench,
};
