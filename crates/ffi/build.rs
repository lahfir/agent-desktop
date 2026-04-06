fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();

    let config = cbindgen::Config::from_file("cbindgen.toml").expect("cbindgen.toml not found");

    let out_path = std::path::Path::new(&out_dir).join("agent_desktop.h");

    if let Ok(bindings) = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        bindings.write_to_file(&out_path);

        let include_dir = std::path::Path::new(&crate_dir).join("include");
        std::fs::create_dir_all(&include_dir).ok();
        std::fs::copy(&out_path, include_dir.join("agent_desktop.h")).ok();
    }

    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/");
}
