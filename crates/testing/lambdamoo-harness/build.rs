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

//! Build script for compiling LambdaMOO C sources into a static library.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Path to LambdaMOO sources (relative to workspace root)
    let workspace_root = Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let lambdamoo_dir = workspace_root.join("lambdamoo");

    if !lambdamoo_dir.join("server.c").exists() {
        panic!(
            "\n\
            LambdaMOO sources not found!\n\n\
            Run the setup script first:\n\
                ./crates/testing/lambdamoo-harness/setup-lambdamoo.sh\n"
        );
    }

    println!("cargo:rerun-if-changed={}", lambdamoo_dir.display());
    println!("cargo:rerun-if-changed=src/net_harness.c");

    // Generate parser.c and y.tab.h from parser.y using bison
    let parser_c = out_dir.join("parser.c");
    let y_tab_h = out_dir.join("y.tab.h");
    let parser_y = lambdamoo_dir.join("parser.y");

    println!("cargo:rerun-if-changed={}", parser_y.display());

    let bison_status = Command::new("bison")
        .arg("-d")
        .arg("-y")
        .arg("-o")
        .arg(&parser_c)
        .arg(format!("--defines={}", y_tab_h.display()))
        .arg(&parser_y)
        .status()
        .expect("Failed to run bison - is it installed?");

    if !bison_status.success() {
        panic!("bison failed to generate parser.c from parser.y");
    }

    // Core source files
    let core_sources = [
        "ast.c",
        "code_gen.c",
        "db_file.c",
        "db_io.c",
        "db_objects.c",
        "db_properties.c",
        "db_verbs.c",
        "decompile.c",
        "disassemble.c",
        "eval_env.c",
        "eval_vm.c",
        "exceptions.c",
        "execute.c",
        "extensions.c",
        "functions.c",
        "keywords.c",
        "list.c",
        "log.c",
        "malloc.c",
        "match.c",
        "md5.c",
        "name_lookup.c",
        "numbers.c",
        "objects.c",
        "parse_cmd.c",
        "pattern.c",
        "program.c",
        "property.c",
        "quota.c",
        "ref_count.c",
        "regexpr.c",
        "server.c",
        "storage.c",
        "streams.c",
        "str_intern.c",
        "sym_table.c",
        "tasks.c",
        "timers.c",
        "unparse.c",
        "utils.c",
        "verbs.c",
        "version.c",
    ];

    let harness_src = Path::new(&manifest_dir).join("src/net_harness.c");
    println!("cargo:rerun-if-changed={}", harness_src.display());

    let mut build = cc::Build::new();

    build.include(&lambdamoo_dir);
    build.include(&out_dir);

    build.flag("-std=gnu99");
    build.flag("-w");
    build.flag("-g");

    build.define("main", "lambdamoo_main");
    build.define("HAVE_SELECT", "1");
    build.define("DEFAULT_FG_TICKS", "100000000");
    build.define("DEFAULT_BG_TICKS", "100000000");

    for src in &core_sources {
        build.file(lambdamoo_dir.join(src));
    }

    build.file(&harness_src);
    build.file(&parser_c);

    build.compile("lambdamoo");

    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=crypt");
}
