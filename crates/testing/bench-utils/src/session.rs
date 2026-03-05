// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::TableFormatter;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::PathBuf,
    sync::{LazyLock, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

/// Collected benchmark result for session summary and JSON export
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub group: String,
    pub benchmark_type: String, // "standard" or "timed"
    pub mops_per_sec: f64,
    pub ns_per_op: f64,
    pub instructions_per_op: f64,
    pub branches_per_op: f64,
    pub branch_miss_rate: f64,     // percentage of branches mispredicted
    pub branch_misses_per_op: f64, // branch misses per operation
    pub cache_miss_rate: f64,
    pub cv_percent: f64,
    pub samples: usize,
    pub operations: u64,
    pub total_duration_sec: f64,
}

/// Complete benchmark session data
#[derive(Serialize, Deserialize)]
pub struct BenchmarkSession {
    pub timestamp: String,
    pub hostname: String,
    pub git_commit: Option<String>,
    pub results: Vec<BenchmarkResult>,
}

/// Global storage for all benchmark results in this session
static SESSION_RESULTS: LazyLock<Mutex<Vec<BenchmarkResult>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Add a benchmark result to the session collection
pub fn add_session_result(result: BenchmarkResult) {
    if let Ok(mut results) = SESSION_RESULTS.lock() {
        results.push(result);
    }
}

/// Get all collected results from this session
pub fn get_session_results() -> Vec<BenchmarkResult> {
    SESSION_RESULTS
        .lock()
        .map(|results| results.clone())
        .unwrap_or_default()
}

/// Clear all collected benchmark results from this process.
pub fn clear_session_results() {
    if let Ok(mut results) = SESSION_RESULTS.lock() {
        results.clear();
    }
}

fn safe_percent_change(current: f64, previous: f64) -> Option<f64> {
    if !current.is_finite() || !previous.is_finite() || previous.abs() <= f64::EPSILON {
        return None;
    }

    Some(((current - previous) / previous) * 100.0)
}

fn format_percent_change(change: Option<f64>) -> String {
    let Some(change) = change else {
        return "n/a".to_string();
    };

    if change.abs() < 1.0 {
        "~0%".to_string()
    } else if change > 0.0 {
        format!("+{change:.1}% 🚀")
    } else {
        format!("{change:.1}% 📉")
    }
}

fn finite_mops(result: &BenchmarkResult) -> Option<f64> {
    let mops = result.mops_per_sec;
    if mops.is_finite() && mops >= 0.0 {
        Some(mops)
    } else {
        None
    }
}

/// Get the target directory for saving benchmark results, following criterion's approach
fn get_target_directory() -> PathBuf {
    // Check CARGO_TARGET_DIR environment variable first
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(target_dir);
    }

    // Try cargo metadata to get target directory
    if let Ok(cargo) = std::env::var("CARGO")
        && let Ok(output) = std::process::Command::new(cargo)
            .args(["metadata", "--format-version", "1"])
            .output()
        && output.status.success()
        && let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        && let Some(target_dir) = metadata_json
            .get("target_directory")
            .and_then(serde_json::Value::as_str)
    {
        return PathBuf::from(target_dir);
    }

    // Fallback to ./target
    PathBuf::from("target")
}

/// Save current session results to JSON file
fn save_session_results() -> Result<String, Box<dyn std::error::Error>> {
    let results = get_session_results();
    if results.is_empty() {
        return Ok("No results to save".to_string());
    }

    // Get proper target directory
    let target_dir = get_target_directory();
    fs::create_dir_all(&target_dir)?;

    // Generate timestamp for filename
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let filename = target_dir.join(format!("benchmark_results_{timestamp}.json"));

    // Get git commit if available
    let git_commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    // Get hostname
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    let session = BenchmarkSession {
        timestamp: format!("{timestamp}"), // Unix timestamp as string
        hostname,
        git_commit,
        results,
    };

    // Write JSON file
    let file = File::create(&filename)?;
    serde_json::to_writer_pretty(file, &session)?;

    Ok(filename.to_string_lossy().to_string())
}

/// Load previous benchmark results for comparison
fn load_previous_results() -> Option<BenchmarkSession> {
    // Find the most recent benchmark file using proper target directory
    let target_dir = get_target_directory();
    if !target_dir.exists() {
        return None;
    }

    let mut json_files: Vec<_> = fs::read_dir(target_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("benchmark_results_")
                && entry.file_name().to_string_lossy().ends_with(".json")
        })
        .collect();

    // Sort by modification time, newest first
    json_files.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });
    json_files.reverse();

    // Return the most recent parseable session.
    for entry in json_files {
        let Ok(file) = File::open(entry.path()) else {
            continue;
        };
        if let Ok(session) = serde_json::from_reader(file) {
            return Some(session);
        }
    }

    None
}

/// Generate final session summary with regression analysis
pub fn generate_session_summary() {
    let current_results = get_session_results();
    if current_results.is_empty() {
        return;
    }

    println!("\n🎯 BENCHMARK SESSION SUMMARY");
    println!("═══════════════════════════════════════════════════════════════════════");

    // Load previous results for comparison
    let previous_session = load_previous_results();

    if let Some(ref prev) = previous_session {
        println!("📊 Comparing with previous run from {}", prev.timestamp);
        if let Some(ref commit) = prev.git_commit {
            println!("   Previous commit: {commit}");
        }
        println!();
    }

    // Group results by category
    let mut groups: BTreeMap<String, Vec<&BenchmarkResult>> = BTreeMap::new();
    for result in &current_results {
        groups.entry(result.group.clone()).or_default().push(result);
    }

    // Display results by group with regression analysis
    for (group_name, group_results) in groups {
        println!(
            "📈 {} ({} benchmarks)",
            group_name.to_uppercase(),
            group_results.len()
        );

        let mut table = TableFormatter::new(
            vec!["Benchmark", "Mops/s", "ns/op", "Change"],
            vec![25, 13, 13, 16], // Change column width
        );

        for result in group_results {
            let change_info = if let Some(ref prev) = previous_session {
                // Find matching benchmark from previous run
                prev.results
                    .iter()
                    .find(|r| r.name == result.name)
                    .map(|prev_result| {
                        let change =
                            safe_percent_change(result.mops_per_sec, prev_result.mops_per_sec);
                        format_percent_change(change)
                    })
                    .unwrap_or_else(|| "NEW".to_string())
            } else {
                "-".to_string()
            };

            table.add_row(vec![
                &result.name,
                &format!("{:.1}", result.mops_per_sec),
                &format!("{:.2}", result.ns_per_op),
                &change_info,
            ]);
        }

        table.print();
        println!();
    }

    // Key insights
    println!("🔍 KEY INSIGHTS:");
    let fastest = current_results
        .iter()
        .filter_map(|result| finite_mops(result).map(|mops| (result, mops)))
        .max_by(|a, b| a.1.total_cmp(&b.1));
    let slowest = current_results
        .iter()
        .filter_map(|result| finite_mops(result).map(|mops| (result, mops)))
        .min_by(|a, b| a.1.total_cmp(&b.1));

    if let (Some((fast, fast_mops)), Some((slow, slow_mops))) = (fastest, slowest) {
        println!("   🏆 Fastest: {} ({:.1} Mops/s)", fast.name, fast_mops);
        println!("   🐌 Slowest: {} ({:.1} Mops/s)", slow.name, slow_mops);
        if slow_mops > f64::EPSILON {
            println!("   📊 Speed difference: {:.1}x", fast_mops / slow_mops);
        } else {
            println!("   📊 Speed difference: n/a");
        }
    } else {
        println!("   No finite throughput values available for insights.");
    }

    // Regression analysis summary
    if let Some(ref prev) = previous_session {
        let mut improvements = 0;
        let mut regressions = 0;
        let mut total_change = 0.0;
        let mut comparable_count = 0;

        for result in &current_results {
            if let Some(prev_result) = prev.results.iter().find(|r| r.name == result.name)
                && let Some(change) =
                    safe_percent_change(result.mops_per_sec, prev_result.mops_per_sec)
            {
                comparable_count += 1;
                total_change += change;
                if change > 1.0 {
                    improvements += 1;
                } else if change < -1.0 {
                    regressions += 1;
                }
            }
        }

        println!();
        println!("📊 REGRESSION ANALYSIS:");
        println!("   ✅ Improvements: {improvements} benchmarks");
        println!("   ❌ Regressions: {regressions} benchmarks");
        if comparable_count > 0 {
            println!(
                "   📈 Average change: {:.1}%",
                total_change / comparable_count as f64
            );
        } else {
            println!("   📈 Average change: n/a");
        }
    }

    // Save results
    match save_session_results() {
        Ok(filename) => println!("\n💾 Results saved to: {filename}"),
        Err(e) => println!("\n⚠️  Failed to save results: {e}"),
    }

    println!("═══════════════════════════════════════════════════════════════════════");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(name: &str, mops_per_sec: f64) -> BenchmarkResult {
        BenchmarkResult {
            name: name.to_string(),
            group: "test".to_string(),
            benchmark_type: "standard".to_string(),
            mops_per_sec,
            ns_per_op: 1.0,
            instructions_per_op: 1.0,
            branches_per_op: 1.0,
            branch_miss_rate: 0.0,
            branch_misses_per_op: 0.0,
            cache_miss_rate: 0.0,
            cv_percent: 0.0,
            samples: 1,
            operations: 1,
            total_duration_sec: 1.0,
        }
    }

    #[test]
    fn safe_percent_change_handles_invalid_input() {
        assert_eq!(safe_percent_change(10.0, 0.0), None);
        assert_eq!(safe_percent_change(f64::NAN, 10.0), None);
        assert_eq!(safe_percent_change(10.0, f64::INFINITY), None);
        assert_eq!(safe_percent_change(12.0, 10.0), Some(20.0));
    }

    #[test]
    fn format_percent_change_handles_none_and_threshold() {
        assert_eq!(format_percent_change(None), "n/a");
        assert_eq!(format_percent_change(Some(0.3)), "~0%");
        assert_eq!(format_percent_change(Some(3.2)), "+3.2% 🚀");
        assert_eq!(format_percent_change(Some(-3.2)), "-3.2% 📉");
    }

    #[test]
    fn finite_mops_filters_out_non_finite_values() {
        assert_eq!(finite_mops(&make_result("ok", 1.0)), Some(1.0));
        assert_eq!(finite_mops(&make_result("nan", f64::NAN)), None);
        assert_eq!(finite_mops(&make_result("neg", -1.0)), None);
    }

    #[test]
    fn clear_session_results_clears_global_state() {
        clear_session_results();
        add_session_result(make_result("bench_a", 1.0));
        assert_eq!(get_session_results().len(), 1);

        clear_session_results();
        assert!(get_session_results().is_empty());
    }
}
