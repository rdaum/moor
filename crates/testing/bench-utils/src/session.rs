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

use crate::TableFormatter;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    sync::Mutex,
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

lazy_static! {
    /// Global storage for all benchmark results in this session
    static ref SESSION_RESULTS: Mutex<Vec<BenchmarkResult>> = Mutex::new(Vec::new());
}

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

/// Get the target directory for saving benchmark results, following criterion's approach
fn get_target_directory() -> std::path::PathBuf {
    // Check CARGO_TARGET_DIR environment variable first
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        return std::path::PathBuf::from(target_dir);
    }

    // Try cargo metadata to get target directory
    if let Ok(cargo) = std::env::var("CARGO")
        && let Ok(output) = std::process::Command::new(cargo)
            .args(["metadata", "--format-version", "1"])
            .output()
        && let Ok(metadata_str) = String::from_utf8(output.stdout)
    {
        // Simple JSON parsing to extract target_directory
        if let Some(start) = metadata_str.find("\"target_directory\":\"") {
            let start = start + "\"target_directory\":\"".len();
            if let Some(end) = metadata_str[start..].find('"') {
                let target_dir = &metadata_str[start..start + end];
                return std::path::PathBuf::from(target_dir);
            }
        }
    }

    // Fallback to ./target
    std::path::PathBuf::from("target")
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

    // Get the most recent file (since current session hasn't been saved yet)
    if json_files.is_empty() {
        return None;
    }

    let previous_file = &json_files[0];
    let file = File::open(previous_file.path()).ok()?;
    serde_json::from_reader(file).ok()
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
    let mut groups: HashMap<String, Vec<&BenchmarkResult>> = HashMap::new();
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
                        let mops_change = ((result.mops_per_sec - prev_result.mops_per_sec)
                            / prev_result.mops_per_sec)
                            * 100.0;
                        if mops_change.abs() < 1.0 {
                            "~0%".to_string()
                        } else if mops_change > 0.0 {
                            format!("+{mops_change:.1}% 🚀")
                        } else {
                            format!("{mops_change:.1}% 📉")
                        }
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
        .max_by(|a, b| a.mops_per_sec.partial_cmp(&b.mops_per_sec).unwrap());
    let slowest = current_results
        .iter()
        .min_by(|a, b| a.mops_per_sec.partial_cmp(&b.mops_per_sec).unwrap());

    if let (Some(fast), Some(slow)) = (fastest, slowest) {
        println!(
            "   🏆 Fastest: {} ({:.1} Mops/s)",
            fast.name, fast.mops_per_sec
        );
        println!(
            "   🐌 Slowest: {} ({:.1} Mops/s)",
            slow.name, slow.mops_per_sec
        );
        println!(
            "   📊 Speed difference: {:.1}x",
            fast.mops_per_sec / slow.mops_per_sec
        );
    }

    // Regression analysis summary
    if let Some(ref prev) = previous_session {
        let mut improvements = 0;
        let mut regressions = 0;
        let mut total_change = 0.0;

        for result in &current_results {
            if let Some(prev_result) = prev.results.iter().find(|r| r.name == result.name) {
                let change = ((result.mops_per_sec - prev_result.mops_per_sec)
                    / prev_result.mops_per_sec)
                    * 100.0;
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
        println!(
            "   📈 Average change: {:.1}%",
            total_change / current_results.len() as f64
        );
    }

    // Save results
    match save_session_results() {
        Ok(filename) => println!("\n💾 Results saved to: {filename}"),
        Err(e) => println!("\n⚠️  Failed to save results: {e}"),
    }

    println!("═══════════════════════════════════════════════════════════════════════");
}
