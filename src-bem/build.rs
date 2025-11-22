//! Build script for compiling NumCalc C++ code

use std::env;

fn main() {
    // Set git hash for version tracking
    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", git_hash.trim());

    // TODO: Compile NumCalc C++ code
    // This will be implemented when we integrate the NumCalc source
}
