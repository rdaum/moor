use std::path::Path;
use std::process::Command;

fn main() {
    let antlr_path = "./antlr4-4.8-2-SNAPSHOT-complete.jar";
    if !Path::new(antlr_path).exists() {
        panic!("Latest custom ANTLR build does not exist at {antlr_path}. Please download it as described at README.md");
    }

    // println!("cargo:rerun-if-changed=grammar/moo.g4");

    let _output = Command::new("java")
        .arg("-jar")
        .arg(antlr_path)
        .arg("-Dlanguage=Rust")
        .arg("-visitor")
        .arg("grammar/moo.g4")
        .arg("-o")
        .arg("src/grammar")
        .output()
        .expect("antlr tool failed to start");

    println!("cargo:rerun-if-changed=grammar/moo.g4");
}
