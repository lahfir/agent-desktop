fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();

    let config = cbindgen::Config::from_file("cbindgen.toml")
        .expect("cbindgen.toml not found");

    let header_path = std::path::Path::new(&out_dir)
        .join("agent_desktop.h");

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
        .expect("cbindgen failed")
        .write_to_file(&header_path);

    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/");
}
