// Build script to generate C header file (macOS only)

fn main() {
    #[cfg(target_os = "macos")]
    {
        use std::env;
        use std::path::PathBuf;

        let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let output_file = PathBuf::from(&crate_dir).join("sotf_audio_plugin_ffi.h");

        cbindgen::Builder::new()
            .with_crate(crate_dir)
            .with_config(cbindgen::Config::from_file("cbindgen.toml").unwrap())
            .generate()
            .expect("Unable to generate bindings")
            .write_to_file(&output_file);

        println!("cargo:rerun-if-changed=src/lib.rs");
        println!("cargo:rerun-if-changed=src/plugin_factory.rs");
        println!("cargo:rerun-if-changed=src/parameter_map.rs");
        println!("cargo:rerun-if-changed=cbindgen.toml");
    }
}
