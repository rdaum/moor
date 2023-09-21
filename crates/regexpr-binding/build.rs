use std::env;
use std::path::PathBuf;

fn main() {
    // first compile the C library
    cc::Build::new()
        .file("c_src/regexpr.c")
        .flag("-Wno-sign-compare")
        .flag("-Wno-implicit-function-declaration")
        .flag("-Wno-implicit-fallthrough")
        .include("c_src")
        .compile("regexpr");

    // Then build the Rust bindings

    println!("cargo:rerun-if-changed=c_src/regexpr.h");
    let bindings = bindgen::Builder::default()
        .header("c_src/regexpr.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
