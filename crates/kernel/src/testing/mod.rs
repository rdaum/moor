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

//! Testing utilities and mocks for the kernel crate

pub mod language_test;
pub mod scheduler_test_utils;
pub mod vm_test;
pub mod vm_test_utils;

pub use scheduler_test_utils::{ExecResult as SchedulerExecResult, call_command, call_eval};
pub use vm_test_utils::{
    ActivationAssemblyBenchState, ActivationBenchResult, EnvironmentBenchResult,
    ExecResult as VmExecResult, MooFrameBenchResult, call_eval_builtin, call_fork, call_verb,
    create_activation_assembly_state_for_bench, create_activation_for_bench,
    create_command_activation_for_bench, create_nested_activation_for_bench,
    create_nested_environment_for_bench, create_nested_moo_frame_for_bench,
    create_top_level_environment_for_bench, create_top_level_moo_frame_for_bench,
    run_activation_assembly_cycle_for_bench, run_activation_assembly_cycle_overhead_for_bench,
};
