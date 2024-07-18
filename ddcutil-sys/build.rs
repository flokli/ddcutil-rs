use std::env;
use std::path::PathBuf;

fn main() {
    let deps = system_deps::Config::new().probe().unwrap();

    let bindings = bindgen::Builder::default()
        .clang_args(deps.all_include_paths().into_iter().map(|p| format!("-I{0}", p.display())))
        .header("wrapper.h")
        .allowlist_type("DDCA_.*")
        .allowlist_type("ddca_.*")
        .allowlist_var("DDCA_.*")
        .allowlist_var("DDCUTIL_.*")
        .allowlist_function("ddca_.*")
        .opaque_type("FILE")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
