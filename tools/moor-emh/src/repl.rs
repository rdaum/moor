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

use crate::{
    AdminSessionFactory, MooAdminHelper,
    commands::{
        cmd_dump, cmd_get, cmd_list, cmd_load, cmd_prog, cmd_props, cmd_reload, cmd_set, cmd_su,
        cmd_verbs, eval_expression, exec_code_block, print_help,
    },
    parse::{ReplCommand, ensure_args, parse_repl_command},
};
use eyre::Report;
use moor_kernel::{SchedulerClient, config::FeaturesConfig};
use moor_var::Obj;
use rustyline::Editor;
use rustyline::error::ReadlineError;
use std::sync::Arc;
use tracing::error;

pub(crate) fn repl(
    scheduler_client: SchedulerClient,
    session_factory: Arc<AdminSessionFactory>,
    features: Arc<FeaturesConfig>,
    wizard: Obj,
    mut rl: Editor<MooAdminHelper, rustyline::history::DefaultHistory>,
) -> Result<(), Report> {
    let mut current_wizard = wizard;
    let logo = r#"                     ███▀▀███▄   ▄█████   ▄██████▄
███▄███▄ ▄███▄ ▄███▄ ███▄▄███▀      ███   ███  ███
██ ██ ██ ██ ██ ██ ██ ███▀▀██▄       ███   ███▄▄███
██ ██ ██ ▀███▀ ▀███▀ ███  ▀███      ███ ██ ▀████▀



         ▄▄▄▄▄▄▄ ▄▄▄      ▄▄▄ ▄▄▄   ▄▄▄
        ███▀▀▀▀▀ ████▄  ▄████ ███   ███
        ███▄▄    ███▀████▀███ █████████
        ███      ███  ▀▀  ███ ███▀▀▀███
        ▀███████ ███      ███ ███   ███            "#;

    let intro = format!(
        "{logo}\nEmergency Medical Hologram - Database Administration Subroutine\n\nPlease state the nature of the database emergency.\n\nRunning as wizard: {}\n\nType 'help' for available commands or 'quit' to deactivate.",
        current_wizard.to_literal()
    );
    println!("{intro}");

    loop {
        let prompt = format!("({}): ", current_wizard.to_literal());
        let readline = rl.readline(&prompt);

        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(line)?;

                match parse_repl_command(line) {
                    ReplCommand::Quit => {
                        println!("Emergency Medical Hologram deactivated.");
                        println!("Database will be saved on shutdown.");
                        break;
                    }
                    ReplCommand::Help => print_help(),
                    ReplCommand::EvalExpr(expr) => {
                        if let Err(e) = eval_expression(
                            &scheduler_client,
                            &session_factory,
                            features.clone(),
                            &current_wizard,
                            &expr,
                        ) {
                            error!("Eval failed: {}", e);
                        }
                    }
                    ReplCommand::ExecCode(code) => {
                        if let Err(e) = exec_code_block(
                            &scheduler_client,
                            &session_factory,
                            features.clone(),
                            &current_wizard,
                            &code,
                        ) {
                            error!("{e}");
                        }
                    }
                    ReplCommand::Get(args) => {
                        if !ensure_args(&args, "get $property or get #OBJ.PROP") {
                            continue;
                        }
                        if let Err(e) = cmd_get(&scheduler_client, &current_wizard, &args) {
                            error!("Get failed: {}", e);
                        }
                    }
                    ReplCommand::Set(args) => {
                        if !ensure_args(&args, "set #OBJ.PROP VALUE") {
                            continue;
                        }
                        if let Err(e) = cmd_set(&scheduler_client, &current_wizard, &args) {
                            error!("Set failed: {}", e);
                        }
                    }
                    ReplCommand::Props(args) => {
                        if !ensure_args(&args, "props #OBJ") {
                            continue;
                        }
                        if let Err(e) = cmd_props(&scheduler_client, &current_wizard, &args) {
                            error!("Props failed: {}", e);
                        }
                    }
                    ReplCommand::Verbs(args) => {
                        if !ensure_args(&args, "verbs #OBJ") {
                            continue;
                        }
                        if let Err(e) = cmd_verbs(&scheduler_client, &current_wizard, &args) {
                            error!("Verbs failed: {}", e);
                        }
                    }
                    ReplCommand::Prog(args) => {
                        if !ensure_args(&args, "prog #OBJ:VERB") {
                            continue;
                        }
                        if let Err(e) = cmd_prog(&scheduler_client, &current_wizard, &args, &mut rl)
                        {
                            error!("Prog failed: {}", e);
                        }
                    }
                    ReplCommand::List(args) => {
                        if !ensure_args(&args, "list #OBJ:VERB") {
                            continue;
                        }
                        if let Err(e) = cmd_list(&scheduler_client, &current_wizard, &args) {
                            error!("List failed: {}", e);
                        }
                    }
                    ReplCommand::Dump(args) => {
                        if !ensure_args(&args, "dump #OBJ [--file FILENAME]") {
                            continue;
                        }
                        if let Err(e) = cmd_dump(&scheduler_client, &current_wizard, &args) {
                            error!("Dump failed: {}", e);
                        }
                    }
                    ReplCommand::Load(args) => {
                        if let Err(e) = cmd_load(&scheduler_client, &current_wizard, &args, &mut rl)
                        {
                            error!("Load failed: {}", e);
                        }
                    }
                    ReplCommand::Reload(args) => {
                        if let Err(e) =
                            cmd_reload(&scheduler_client, &current_wizard, &args, &mut rl)
                        {
                            error!("Reload failed: {}", e);
                        }
                    }
                    ReplCommand::Su(args) => {
                        if !ensure_args(&args, "su #OBJ") {
                            continue;
                        }
                        match cmd_su(&scheduler_client, &current_wizard, &args) {
                            Ok(new_wizard) => current_wizard = new_wizard,
                            Err(e) => error!("Su failed: {}", e),
                        }
                    }
                    ReplCommand::Unknown => {
                        error!("Unknown command. Type 'help' for available commands.");
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C - use 'quit' to exit");
            }
            Err(ReadlineError::Eof) => {
                println!("EOF - exiting");
                break;
            }
            Err(err) => {
                error!("Error reading line: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
