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

fn main() {
    println!("cargo:rerun-if-changed=schema/values.fbs");

    // Find the flatc binary.
    let flatc_path = flatc::flatc();

    // Emit the version # by executing the binary with --version
    let version = std::process::Command::new(flatc_path)
        .arg("--version")
        .output()
        .expect("failed to get flatc version")
        .stdout;

    println!(
        "cargo:warning=Compiling flatbuffers with {}",
        String::from_utf8(version).unwrap()
    );

    // Invoke flatc to generate Rust code
    std::process::Command::new(flatc_path)
        // Rust output
        .arg("-r")
        // My output directory
        .arg("-o")
        .arg("../target/flatbuffers/")
        // My schema
        .arg("schema/values.fbs")
        .output()
        .expect("failed to run flatc");
}
