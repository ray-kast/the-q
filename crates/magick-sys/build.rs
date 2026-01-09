use std::{env, path::PathBuf};

fn main() {
    let cfg = pkg_config::Config::new()
        .range_version("7.1.1".."8")
        .probe("MagickCore-7.Q16HDRI")
        .unwrap();

    for path in cfg.link_paths {
        println!("cargo:rustc-link-search={}", path.display());
    }

    for lib in cfg.link_files {
        println!("cargo:rustc-link-lib={}", lib.display());
    }

    let bindings = bindgen::Builder::default()
        .clang_args(
            cfg.include_paths
                .iter()
                .map(|p| format!("-I{}", p.to_str().unwrap())),
        )
        .header("src/wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .blocklist_var("FP_INT_UPWARD")
        .blocklist_var("FP_INT_DOWNWARD")
        .blocklist_var("FP_INT_TOWARDZERO")
        .blocklist_var("FP_INT_TONEARESTFROMZERO")
        .blocklist_var("FP_INT_TONEAREST")
        .blocklist_var("FP_NAN")
        .blocklist_var("FP_INFINITE")
        .blocklist_var("FP_ZERO")
        .blocklist_var("FP_SUBNORMAL")
        .blocklist_var("FP_NORMAL")
        .generate()
        .unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .unwrap();
}
