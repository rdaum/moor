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

//! Standalone activation/frame allocation workload driver for `perf`.
//!
//! Focuses only on `Activation::for_call` construction paths so hardware counter
//! profiles are not polluted by Criterion harness overhead.

use std::hint::black_box;
use std::process::ExitCode;

use uuid::Uuid;

use moor_common::{
    model::{VerbArgsSpec, VerbDef, VerbFlag},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_kernel::testing::{create_activation_for_bench, create_nested_activation_for_bench};
use moor_var::{
    List, SYSTEM_OBJECT, Symbol, program::ProgramType, v_empty_str, v_int, v_obj, v_str,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Simple,
    Medium,
    Complex,
    WithArgs,
    WithArgstr,
    NestedSimple,
    Mixed,
}

impl Scenario {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "simple" => Some(Self::Simple),
            "medium" => Some(Self::Medium),
            "complex" => Some(Self::Complex),
            "with_args" => Some(Self::WithArgs),
            "with_argstr" => Some(Self::WithArgstr),
            "nested_simple" => Some(Self::NestedSimple),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Config {
    scenario: Scenario,
    warmup: u64,
    iters: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scenario: Scenario::NestedSimple,
            warmup: 100_000,
            iters: 1_500_000,
        }
    }
}

fn parse_u64(value: &str, name: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|e| format!("invalid {name} value '{value}': {e}"))
}

fn parse_config(args: &[String]) -> Result<Config, String> {
    let mut config = Config::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scenario" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --scenario".to_string());
                }
                config.scenario = Scenario::parse(args[i].as_str())
                    .ok_or_else(|| format!("unknown scenario '{}'", args[i]))?;
            }
            "--warmup" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --warmup".to_string());
                }
                config.warmup = parse_u64(args[i].as_str(), "warmup")?;
            }
            "--iters" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for --iters".to_string());
                }
                config.iters = parse_u64(args[i].as_str(), "iters")?;
            }
            "--help" | "-h" => {
                return Err(usage());
            }
            unknown => {
                return Err(format!("unknown argument '{unknown}'\n\n{}", usage()));
            }
        }
        i += 1;
    }
    Ok(config)
}

fn usage() -> String {
    let text = r#"activation_profile
Standalone activation/frame allocation workload for perf analysis.

USAGE:
  cargo run --release -p moor-kernel --bin activation_profile -- [OPTIONS]

OPTIONS:
  --scenario <name>  simple | medium | complex | with_args | with_argstr | nested_simple | mixed
  --warmup <n>       Warmup iterations (default: 100000)
  --iters <n>        Measured iterations (default: 1500000)
"#;
    text.to_string()
}

fn make_verbdef(verb_name: Symbol) -> VerbDef {
    VerbDef::new(
        Uuid::new_v4(),
        SYSTEM_OBJECT,
        SYSTEM_OBJECT,
        &[verb_name],
        BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Debug,
        VerbArgsSpec::this_none_this(),
    )
}

struct Workload {
    verb_name: Symbol,
    verbdef: VerbDef,
    this: moor_var::Var,
    caller: moor_var::Var,
    empty_args: List,
    small_args: List,
    empty_argstr: moor_var::Var,
    short_argstr: moor_var::Var,
    simple_program: ProgramType,
    medium_program: ProgramType,
    complex_program: ProgramType,
}

impl Workload {
    fn new() -> Self {
        let simple_program =
            ProgramType::MooR(compile("return 1;", CompileOptions::default()).unwrap());
        let medium_program = ProgramType::MooR(
            compile(
                r#"
                x = 1;
                y = 2;
                z = x + y;
                return z;
                "#,
                CompileOptions::default(),
            )
            .unwrap(),
        );
        let complex_program = ProgramType::MooR(
            compile(
                r#"
                result = {};
                for i in [1..10]
                    result = {@result, i * 2};
                endfor
                return result;
                "#,
                CompileOptions::default(),
            )
            .unwrap(),
        );

        Self {
            verb_name: Symbol::mk("test"),
            verbdef: make_verbdef(Symbol::mk("test")),
            this: v_obj(SYSTEM_OBJECT),
            caller: v_obj(SYSTEM_OBJECT),
            empty_args: List::mk_list(&[]),
            small_args: List::mk_list(&[v_int(1), v_int(2), v_str("hello")]),
            empty_argstr: v_empty_str(),
            short_argstr: v_str("some argument string"),
            simple_program,
            medium_program,
            complex_program,
        }
    }

    fn create_top_level(
        &self,
        program: ProgramType,
        args: List,
        argstr: moor_var::Var,
    ) -> moor_kernel::testing::ActivationBenchResult {
        create_activation_for_bench(
            SYSTEM_OBJECT,
            self.verbdef.clone(),
            self.verb_name,
            self.this.clone(),
            SYSTEM_OBJECT,
            args,
            self.caller.clone(),
            argstr,
            program,
        )
    }

    #[inline]
    fn cycle_simple(&self) {
        let activation = self.create_top_level(
            self.simple_program.clone(),
            self.empty_args.clone(),
            self.empty_argstr.clone(),
        );
        black_box(&activation);
        drop(activation);
    }

    #[inline]
    fn cycle_medium(&self) {
        let activation = self.create_top_level(
            self.medium_program.clone(),
            self.empty_args.clone(),
            self.empty_argstr.clone(),
        );
        black_box(&activation);
        drop(activation);
    }

    #[inline]
    fn cycle_complex(&self) {
        let activation = self.create_top_level(
            self.complex_program.clone(),
            self.empty_args.clone(),
            self.empty_argstr.clone(),
        );
        black_box(&activation);
        drop(activation);
    }

    #[inline]
    fn cycle_with_args(&self) {
        let activation = self.create_top_level(
            self.simple_program.clone(),
            self.small_args.clone(),
            self.empty_argstr.clone(),
        );
        black_box(&activation);
        drop(activation);
    }

    #[inline]
    fn cycle_with_argstr(&self) {
        let activation = self.create_top_level(
            self.simple_program.clone(),
            self.empty_args.clone(),
            self.short_argstr.clone(),
        );
        black_box(&activation);
        drop(activation);
    }

    #[inline]
    fn cycle_nested_simple(&self, parent: &moor_kernel::testing::ActivationBenchResult) {
        let activation = create_nested_activation_for_bench(
            SYSTEM_OBJECT,
            self.verbdef.clone(),
            self.verb_name,
            self.this.clone(),
            SYSTEM_OBJECT,
            self.empty_args.clone(),
            self.caller.clone(),
            self.empty_argstr.clone(),
            parent,
            self.simple_program.clone(),
        );
        black_box(&activation);
        drop(activation);
    }
}

fn run_loop(config: Config) {
    let workload = Workload::new();
    let parent = workload.create_top_level(
        workload.simple_program.clone(),
        workload.empty_args.clone(),
        workload.empty_argstr.clone(),
    );
    match config.scenario {
        Scenario::Simple => {
            for _ in 0..config.warmup {
                workload.cycle_simple();
            }
            for _ in 0..config.iters {
                workload.cycle_simple();
            }
        }
        Scenario::Medium => {
            for _ in 0..config.warmup {
                workload.cycle_medium();
            }
            for _ in 0..config.iters {
                workload.cycle_medium();
            }
        }
        Scenario::Complex => {
            for _ in 0..config.warmup {
                workload.cycle_complex();
            }
            for _ in 0..config.iters {
                workload.cycle_complex();
            }
        }
        Scenario::WithArgs => {
            for _ in 0..config.warmup {
                workload.cycle_with_args();
            }
            for _ in 0..config.iters {
                workload.cycle_with_args();
            }
        }
        Scenario::WithArgstr => {
            for _ in 0..config.warmup {
                workload.cycle_with_argstr();
            }
            for _ in 0..config.iters {
                workload.cycle_with_argstr();
            }
        }
        Scenario::NestedSimple => {
            for _ in 0..config.warmup {
                workload.cycle_nested_simple(&parent);
            }
            for _ in 0..config.iters {
                workload.cycle_nested_simple(&parent);
            }
        }
        Scenario::Mixed => {
            for i in 0..config.warmup {
                match i % 6 {
                    0 => workload.cycle_simple(),
                    1 => workload.cycle_with_args(),
                    2 => workload.cycle_with_argstr(),
                    3 => workload.cycle_nested_simple(&parent),
                    4 => workload.cycle_medium(),
                    _ => workload.cycle_complex(),
                }
            }
            for i in 0..config.iters {
                match i % 6 {
                    0 => workload.cycle_simple(),
                    1 => workload.cycle_with_args(),
                    2 => workload.cycle_with_argstr(),
                    3 => workload.cycle_nested_simple(&parent),
                    4 => workload.cycle_medium(),
                    _ => workload.cycle_complex(),
                }
            }
        }
    }
}

fn main() -> ExitCode {
    let args = std::env::args().collect::<Vec<_>>();
    let config = match parse_config(&args) {
        Ok(cfg) => cfg,
        Err(message) => {
            eprintln!("{message}");
            if message.starts_with("activation_profile") {
                return ExitCode::SUCCESS;
            }
            return ExitCode::FAILURE;
        }
    };

    eprintln!(
        "activation_profile: scenario={:?} warmup={} iters={}",
        config.scenario, config.warmup, config.iters
    );
    run_loop(config);
    ExitCode::SUCCESS
}
