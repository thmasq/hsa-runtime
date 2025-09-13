use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to link HSA runtime library
    println!("cargo:rustc-link-lib=hsa-runtime64");
    println!("cargo:rustc-link-search=native=/opt/rocm/lib");

    // Generate bindings
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg("-I/opt/rocm/include")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("hsa_.*")
        .allowlist_type("hsa_.*")
        .allowlist_var("HSA_.*")
        .derive_default(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
