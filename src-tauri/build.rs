use std::env;
use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // Copy steam_api64.dll to the output directory
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Source: steam_api64.dll in src-tauri directory
    let dll_source = manifest_dir.join("steam_api64.dll");

    // Destination: target/debug or target/release directory
    let target_dir = out_dir
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    let dll_dest = target_dir.join("steam_api64.dll");

    // Copy the DLL if it exists
    if dll_source.exists() {
        std::fs::copy(&dll_source, &dll_dest).expect("Failed to copy steam_api64.dll");
        println!("cargo:rerun-if-changed={}", dll_source.display());
    }
}
