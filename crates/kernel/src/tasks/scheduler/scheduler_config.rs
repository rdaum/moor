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

use super::*;

static SERVER_OPTIONS: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("server_options"));
static BG_SECONDS: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("bg_seconds"));
static BG_TICKS: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("bg_ticks"));
static FG_SECONDS: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("fg_seconds"));
static FG_TICKS: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("fg_ticks"));
static MAX_STACK_DEPTH: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("max_stack_depth"));
static DUMP_INTERVAL: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("dump_interval"));
static GC_INTERVAL: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("gc_interval"));
static MAX_TASK_RETRIES: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("max_task_retries"));
static MAX_TASK_MAILBOX: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("max_task_mailbox"));

fn load_int_sysprop(server_options_obj: &Obj, name: Symbol, tx: &dyn WorldState) -> Option<u64> {
    let Ok(value) = tx.retrieve_property(&SYSTEM_OBJECT, server_options_obj, name) else {
        return None;
    };
    match value.as_integer() {
        Some(i) if i >= 0 => Some(i as u64),
        _ => {
            warn!("${name} is not a non-negative integer");
            None
        }
    }
}

fn load_float_sysprop(server_options_obj: &Obj, name: Symbol, tx: &dyn WorldState) -> Option<f64> {
    let Ok(value) = tx.retrieve_property(&SYSTEM_OBJECT, server_options_obj, name) else {
        return None;
    };
    match value.as_float_numeric() {
        Some(f) if f.is_finite() && f >= 0.0 => Some(f),
        _ => {
            warn!("${name} is not a non-negative number");
            None
        }
    }
}

impl Scheduler {
    pub fn reload_server_options(&self) {
        // Load the server options from the database, if possible.
        let tx = self
            .database
            .new_world_state()
            .expect("Could not open transaction to read server properties");

        let mut lc = self.lifecycle.lock().unwrap();
        let mut so = lc.server_options.clone();

        let Ok(server_options_obj) =
            tx.retrieve_property(&SYSTEM_OBJECT, &SYSTEM_OBJECT, *SERVER_OPTIONS)
        else {
            info!("No server options object found; using defaults");
            tx.rollback().unwrap();
            return;
        };
        let Some(server_options_obj) = server_options_obj.as_object() else {
            info!("Server options property is not an object; using defaults");
            tx.rollback().unwrap();
            return;
        };
        info!("Found server options object: {}", server_options_obj);

        if let Some(bg_seconds) = load_float_sysprop(&server_options_obj, *BG_SECONDS, tx.as_ref())
        {
            so.bg_seconds = bg_seconds;
        }
        if let Some(bg_ticks) = load_int_sysprop(&server_options_obj, *BG_TICKS, tx.as_ref()) {
            so.bg_ticks = bg_ticks as usize;
        }
        if let Some(fg_seconds) = load_float_sysprop(&server_options_obj, *FG_SECONDS, tx.as_ref())
        {
            so.fg_seconds = fg_seconds;
        }
        if let Some(fg_ticks) = load_int_sysprop(&server_options_obj, *FG_TICKS, tx.as_ref()) {
            so.fg_ticks = fg_ticks as usize;
        }
        if let Some(max_stack_depth) =
            load_int_sysprop(&server_options_obj, *MAX_STACK_DEPTH, tx.as_ref())
        {
            so.max_stack_depth = max_stack_depth as usize;
        }
        if let Some(max_task_retries) =
            load_int_sysprop(&server_options_obj, *MAX_TASK_RETRIES, tx.as_ref())
        {
            so.max_task_retries = max_task_retries as u8;
        }
        if let Some(max_task_mailbox) =
            load_int_sysprop(&server_options_obj, *MAX_TASK_MAILBOX, tx.as_ref())
        {
            so.max_task_mailbox = max_task_mailbox as usize;
        }
        if let Some(dump_interval) = load_int_sysprop(&SYSTEM_OBJECT, *DUMP_INTERVAL, tx.as_ref()) {
            info!(
                "Loaded dump_interval from database: {} seconds",
                dump_interval
            );
            so.dump_interval = Some(dump_interval);
        } else {
            info!("No dump_interval found on #0");
        }
        if let Some(gc_interval) = load_int_sysprop(&SYSTEM_OBJECT, *GC_INTERVAL, tx.as_ref()) {
            info!("Loaded gc_interval from database: {} seconds", gc_interval);
            so.gc_interval = Some(gc_interval);
        } else {
            // Check if we have a config override before falling back to default
            if self.config.runtime.gc_interval.is_some() {
                info!("No gc_interval found on #0, will use config override");
                so.gc_interval = None; // Config will take precedence
            } else {
                info!(
                    "No gc_interval found on #0, using default of {} seconds",
                    DEFAULT_GC_INTERVAL_SECONDS
                );
                so.gc_interval = Some(DEFAULT_GC_INTERVAL_SECONDS);
            }
        }
        tx.rollback().unwrap();

        lc.server_options = so;

        info!("Server options refreshed.");
    }

    pub fn get_checkpoint_interval(
        &self,
        cli_checkpoint_interval: Option<Duration>,
    ) -> Option<Duration> {
        // Reload server options to get fresh dump_interval from database
        self.reload_server_options();

        let lc = self.lifecycle.lock().unwrap();

        // Determine the checkpoint interval using the proper precedence:
        // 1. Command-line config overrides all
        // 2. Database dump_interval setting
        // 3. Default disabled
        if let Some(cli_interval) = cli_checkpoint_interval {
            info!(
                "Using checkpoint_interval from command-line config: {:?}",
                cli_interval
            );
            Some(cli_interval)
        } else if let Some(db_secs) = lc.server_options.dump_interval {
            let db_interval = Duration::from_secs(db_secs);
            info!("Using dump_interval from database: {:?}", db_interval);
            Some(db_interval)
        } else {
            None
        }
    }

    pub fn get_gc_interval(&self) -> Option<Duration> {
        // Reload server options to get fresh gc_interval from database
        self.reload_server_options();

        let lc = self.lifecycle.lock().unwrap();

        // Determine the GC interval using the proper precedence:
        // 1. Command-line config overrides all
        // 2. Database gc_interval setting
        // 3. Default of 30 seconds
        if let Some(config_interval) = self.config.runtime.gc_interval {
            info!(
                "Using gc_interval from command-line config: {:?}",
                config_interval
            );
            Some(config_interval)
        } else if let Some(db_secs) = lc.server_options.gc_interval {
            let db_interval = Duration::from_secs(db_secs);
            info!("Using gc_interval from database: {:?}", db_interval);
            Some(db_interval)
        } else {
            // Default to 30 seconds if not configured
            let default_interval = Duration::from_secs(DEFAULT_GC_INTERVAL_SECONDS);
            info!("Using default gc_interval: {:?}", default_interval);
            Some(default_interval)
        }
    }
}
