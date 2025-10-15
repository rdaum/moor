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

//! Shared tracing initialization utilities for moor binaries

use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

/// Initialize tracing with environment-based configuration and fallback support
///
/// - Uses `RUST_LOG` environment variable when available
/// - Falls back to provided debug flag when `RUST_LOG` is not set
/// - Uses layered subscriber architecture for flexibility
/// - Provides consistent formatting across all binaries
///
/// # Arguments
/// * `debug_fallback` - If true and `RUST_LOG` is not set, uses DEBUG level; otherwise INFO
///
/// # Returns
/// * `Ok(())` on successful initialization
/// * `Err(eyre::Report)` if tracing initialization fails
pub fn init_tracing(debug_fallback: bool) -> Result<(), eyre::Report> {
    let filter = if let Ok(env_filter) = EnvFilter::try_from_default_env() {
        // User has set RUST_LOG, respect it but still suppress gdt_cpus
        env_filter.add_directive("gdt_cpus=off".parse().unwrap())
    } else {
        // No RUST_LOG set, build filter from scratch with gdt_cpus suppressed
        let level = if debug_fallback { "debug" } else { "info" };
        EnvFilter::new(format!("{level},gdt_cpus=off"))
    };

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .compact()
                .with_ansi(true)
                .with_file(true)
                .with_target(false)
                .with_line_number(true)
                .with_thread_names(true)
                .with_span_events(fmt::format::FmtSpan::NONE),
        )
        .with(filter)
        .init();

    Ok(())
}

/// Initialize tracing with simplified formatting (no file/line info)
///
/// Used for binaries that prefer cleaner output like moorc
///
/// # Arguments
/// * `debug_fallback` - If true and `RUST_LOG` is not set, uses DEBUG level; otherwise INFO
///
/// # Returns
/// * `Ok(())` on successful initialization
/// * `Err(eyre::Report)` if tracing initialization fails
pub fn init_tracing_simple(debug_fallback: bool) -> Result<(), eyre::Report> {
    let filter = if let Ok(env_filter) = EnvFilter::try_from_default_env() {
        // User has set RUST_LOG, respect it but still suppress gdt_cpus
        env_filter.add_directive("gdt_cpus=off".parse().unwrap())
    } else {
        // No RUST_LOG set, build filter from scratch with gdt_cpus suppressed
        let level = if debug_fallback { "debug" } else { "info" };
        EnvFilter::new(format!("{level},gdt_cpus=off"))
    };

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .compact()
                .with_ansi(true)
                .with_file(false)
                .with_target(false)
                .with_line_number(false)
                .with_thread_names(false)
                .with_span_events(fmt::format::FmtSpan::NONE),
        )
        .with(filter)
        .init();

    Ok(())
}
