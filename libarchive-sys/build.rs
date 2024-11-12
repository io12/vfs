use bindgen::MacroTypeVariation::Signed;
use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rustc-link-lib=archive");
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .default_macro_constant_type(Signed)
        .generate()
        .unwrap();
    bindings
        .write_to_file(PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs"))
        .unwrap();
}
