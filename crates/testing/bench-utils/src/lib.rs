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

//! Benchmark utilities for the MOO system
//!
//! This crate provides a reusable microbenchmark framework with:
//! - Performance counter integration (Linux only)
//! - Console output with Unicode tables
//! - JSON result persistence and regression analysis  
//! - Warm-up and calibration phases
//! - Progress indicators
//! - Generic table formatting

pub mod bench;
pub mod session;
pub mod table;

pub use bench::{
    BenchContext, BenchmarkDef, NoContext, op_bench, op_bench_with_factory,
    op_bench_with_factory_filtered, run_benchmark, run_benchmark_group,
};
pub use table::TableFormatter;

#[cfg(target_os = "linux")]
pub use bench::PerfCounters;
pub use session::{
    BenchmarkResult, BenchmarkSession, add_session_result, generate_session_summary,
    get_session_results,
};

// Re-export key types for convenience
pub use minstant;
pub use std::hint::black_box;

#[cfg(target_os = "linux")]
pub use perf_event;
