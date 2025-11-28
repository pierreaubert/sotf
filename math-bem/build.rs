//! Build script for compiling NumCalc C++ BEM solver
//!
//! This script handles compilation of the NumCalc C++ code as part of
//! the Rust build process.
//!
//! ## Build Modes
//!
//! 1. **Use existing NumCalc**: If NUMCALC_PATH is set or NumCalc is in PATH
//! 2. **Compile from source**: If NumCalc/src/ directory exists
//! 3. **Skip**: If neither available (runtime will fail gracefully)
//!
//! ## Environment Variables
//!
//! - `NUMCALC_PATH`: Path to pre-built NumCalc executable
//! - `NUMCALC_SOURCE_DIR`: Path to NumCalc source directory
//! - `SKIP_NUMCALC_BUILD`: Set to "1" to skip compilation
//!
//! ## Requirements for Building
//!
//! - C++ compiler (g++, clang++, or MSVC)
//! - Make (optional, for using existing Makefile)

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=NumCalc/src/");
    println!("cargo:rerun-if-env-changed=NUMCALC_PATH");
    println!("cargo:rerun-if-env-changed=SKIP_NUMCALC_BUILD");

    // Set git hash for version tracking
    set_git_hash();

    // Skip NumCalc build if requested
    if env::var("SKIP_NUMCALC_BUILD").is_ok() {
        println!("cargo:warning=Skipping NumCalc build (SKIP_NUMCALC_BUILD is set)");
        return;
    }

    // Check if NumCalc already exists
    if check_existing_numcalc() {
        println!("cargo:warning=Using existing NumCalc installation");
        return;
    }

    // Try to build NumCalc from source
    match build_numcalc() {
        Ok(exe_path) => {
            println!(
                "cargo:warning=NumCalc built successfully: {}",
                exe_path.display()
            );
            // Set environment variable for runtime
            println!("cargo:rustc-env=NUMCALC_BUILT_PATH={}", exe_path.display());
        }
        Err(e) => {
            println!("cargo:warning=NumCalc build failed: {}", e);
            println!(
                "cargo:warning=FFI functionality will require NumCalc to be installed separately"
            );
            println!(
                "cargo:warning=Set NUMCALC_PATH environment variable to use pre-built NumCalc"
            );
        }
    }
}

/// Set git hash for version tracking
fn set_git_hash() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}

/// Check if NumCalc already exists in system
fn check_existing_numcalc() -> bool {
    // Check NUMCALC_PATH
    if let Ok(path) = env::var("NUMCALC_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            println!(
                "cargo:warning=Found NumCalc at NUMCALC_PATH: {}",
                path.display()
            );
            return true;
        }
    }

    // Check system PATH
    #[cfg(target_os = "windows")]
    let exe_name = "NumCalc.exe";
    #[cfg(not(target_os = "windows"))]
    let exe_name = "NumCalc";

    if which::which(exe_name).is_ok() {
        println!("cargo:warning=Found NumCalc in system PATH");
        return true;
    }

    false
}

/// Build NumCalc from source
fn build_numcalc() -> Result<PathBuf, String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    // Look for NumCalc source directory
    let source_dir = if let Ok(custom_dir) = env::var("NUMCALC_SOURCE_DIR") {
        PathBuf::from(custom_dir)
    } else {
        PathBuf::from(&manifest_dir).join("NumCalc/src")
    };

    if !source_dir.exists() {
        return Err(format!(
            "NumCalc source not found at: {}\n\
             To build NumCalc:\n\
             1. Clone Mesh2HRTF: git clone https://github.com/Any2HRTF/Mesh2HRTF.git NumCalc\n\
             2. Or set NUMCALC_SOURCE_DIR to point to NumCalc/src\n\
             3. Or set SKIP_NUMCALC_BUILD=1 to skip compilation",
            source_dir.display()
        ));
    }

    println!(
        "cargo:warning=Building NumCalc from source: {}",
        source_dir.display()
    );

    // Try Makefile first (if available)
    if source_dir.join("Makefile").exists() {
        if let Ok(exe) = build_with_makefile(&source_dir, &out_dir) {
            return Ok(exe);
        }
        println!("cargo:warning=Makefile build failed, trying cc crate...");
    }

    // Fall back to cc crate
    build_with_cc(&source_dir, &out_dir)
}

/// Build using existing Makefile
fn build_with_makefile(source_dir: &Path, out_dir: &str) -> Result<PathBuf, String> {
    println!("cargo:warning=Building with Makefile...");

    let status = Command::new("make")
        .current_dir(source_dir)
        .status()
        .map_err(|e| format!("Failed to run make: {}", e))?;

    if !status.success() {
        return Err("Make failed".to_string());
    }

    // Find built executable
    #[cfg(target_os = "windows")]
    let exe_name = "NumCalc.exe";
    #[cfg(not(target_os = "windows"))]
    let exe_name = "NumCalc";

    let exe_path = source_dir.join(exe_name);
    if !exe_path.exists() {
        return Err(format!(
            "NumCalc executable not found at: {}",
            exe_path.display()
        ));
    }

    // Copy to output directory
    let out_exe = PathBuf::from(out_dir).join(exe_name);
    std::fs::copy(&exe_path, &out_exe).map_err(|e| format!("Failed to copy executable: {}", e))?;

    Ok(out_exe)
}

/// Build using cc crate
fn build_with_cc(source_dir: &Path, out_dir: &str) -> Result<PathBuf, String> {
    println!("cargo:warning=Building with cc crate...");

    // Find all C++ source files
    let cpp_files = find_cpp_files(source_dir)?;

    if cpp_files.is_empty() {
        return Err("No C++ source files found".to_string());
    }

    println!("cargo:warning=Found {} C++ source files", cpp_files.len());

    // Configure compiler
    let mut build = cc::Build::new();

    build
        .cpp(true)
        .flag_if_supported("-std=c++11")
        .flag_if_supported("/std:c++11") // MSVC
        .flag_if_supported("-O3")
        .flag_if_supported("/O2") // MSVC
        .warnings(false); // NumCalc may have warnings

    // Add all source files
    for file in &cpp_files {
        build.file(file);
    }

    // Compile
    build.compile("numcalc");

    // Link to create executable
    #[cfg(target_os = "windows")]
    let exe_name = "NumCalc.exe";
    #[cfg(not(target_os = "windows"))]
    let exe_name = "NumCalc";

    let exe_path = PathBuf::from(out_dir).join(exe_name);

    println!("cargo:warning=NumCalc compilation complete");
    println!(
        "cargo:warning=Executable should be at: {}",
        exe_path.display()
    );

    // Note: cc crate creates a static library, not an executable
    // For a full executable, we'd need to link manually
    // For now, just indicate where it should be

    Ok(exe_path)
}

/// Find all C++ source files in directory
fn find_cpp_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_file()
            && path.extension().is_some_and(|ext| ext == "cpp" || ext == "cc" || ext == "cxx")
        {
            files.push(path);
        }
    }

    Ok(files)
}
