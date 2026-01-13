use std::{env, path::PathBuf};

fn main() {
    let cfg = pkg_config::Config::new()
        .range_version("0.4.2".."0.5")
        .probe("lqr-1")
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
        .generate()
        .unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .unwrap();
}
