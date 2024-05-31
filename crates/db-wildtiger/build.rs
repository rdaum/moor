// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::env;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let third_party_dir = path.join("third-party");
    let wiredtiger_dir = third_party_dir.join("wiredtiger");

    // Run cmake to build wiredtiger as a static library
    let dst = cmake::Config::new(wiredtiger_dir)
        .uses_cxx11()
        .build_target("wt")
        .define("ENABLE_STATIC", "1")
        .build();
    let build = dst.join("build");
    let include_dir = build.join("include");

    // Search paths
    println!("cargo:include={}/include", include_dir.display());
    println!("cargo:rustc-link-search=native={}", build.display());
    println!("cargo:rustc-link-lib=static=wiredtiger");

    // Link to the generated wiredtiger library
    println!("cargo:rustc-link-lib=wiredtiger");

    //  Bindings will be built off our wrapper.h file, which includes wiretiger.h
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .clang_arg(format!("-I{}", include_dir.display()))
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
