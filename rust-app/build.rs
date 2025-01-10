use std::env;

fn main() {
    println!("cargo:rerun-if-changed=script.ld");
    if let Ok(path) = env::var("NEWLIB_LIB_PATH") {
        println!("cargo:rustc-link-search={path}");
    }
}
