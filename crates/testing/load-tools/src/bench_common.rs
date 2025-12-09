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

//! Shared utilities for performance benchmarks

use std::time::Duration;

/// Calculate percentiles from a list of latencies.
/// Returns (p50, p95, p99, max).
pub fn calculate_percentiles(mut latencies: Vec<Duration>) -> (Duration, Duration, Duration, Duration) {
    if latencies.is_empty() {
        return (
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
        );
    }

    latencies.sort();
    let len = latencies.len();

    let p50 = latencies[len / 2];
    let p95 = latencies[(len * 95) / 100];
    let p99 = latencies[(len * 99) / 100];
    let max = latencies[len - 1];

    (p50, p95, p99, max)
}

/// Format a duration for display in benchmark tables.
pub fn format_duration(d: Duration) -> String {
    if d.as_nanos() < 1000 {
        format!("{}ns", d.as_nanos())
    } else if d.as_micros() < 1000 {
        format!("{:.1}µs", d.as_nanos() as f64 / 1000.0)
    } else if d.as_millis() < 1000 {
        format!("{:.2}ms", d.as_micros() as f64 / 1000.0)
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

/// Format a throughput value for display.
pub fn format_throughput(ops_per_sec: f64) -> String {
    if ops_per_sec >= 1_000_000.0 {
        format!("{:.2}M/s", ops_per_sec / 1_000_000.0)
    } else if ops_per_sec >= 1_000.0 {
        format!("{:.2}K/s", ops_per_sec / 1_000.0)
    } else {
        format!("{:.2}/s", ops_per_sec)
    }
}

/// Creates a temporary database directory or uses the provided path.
pub fn setup_db_path(
    db_path: &std::path::PathBuf,
    default_name: &str,
) -> Result<(std::path::PathBuf, Option<tempfile::TempDir>), eyre::Error> {
    let temp_dir = if db_path == &std::path::PathBuf::from(default_name) {
        Some(tempfile::tempdir()?)
    } else {
        None
    };

    let actual_path = if let Some(ref temp_dir) = temp_dir {
        temp_dir.path().join(default_name)
    } else {
        db_path.clone()
    };

    Ok((actual_path, temp_dir))
}

/// Spinner animation characters for progress display.
pub const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Update spinner display during benchmark execution.
pub fn update_spinner(spinner_idx: &mut usize, message: &str) {
    *spinner_idx = (*spinner_idx + 1) % SPINNER.len();
    eprint!("\r  {} {}", SPINNER[*spinner_idx], message);
    std::io::Write::flush(&mut std::io::stderr()).ok();
}

/// Clear screen and move cursor to top (ANSI escape codes).
pub fn clear_screen() {
    eprint!("\x1B[2J\x1B[1;1H");
}
