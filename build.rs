use std::env;
use std::path::{Path, PathBuf};

fn main() {
    const MIN_VERSION: &str = "2.12";

    println!("cargo:rerun-if-changed=build.rs");

    // Try to find system liburing via pkg-config
    if try_pkg_config(MIN_VERSION) {
        println!("cargo:rustc-link-lib=uring");
        return;
    }

    // Fallback: clone and build liburing from source
    // When building from source, link with liburing-ffi which contains the inline functions
    println!("cargo:rustc-link-lib=static=uring-ffi");
    build_from_source(MIN_VERSION);
}

fn try_pkg_config(min_version: &str) -> bool {
    let mut cfg = pkg_config::Config::new();
    cfg.cargo_metadata(false);
    cfg.atleast_version(min_version);

    let lib = match cfg.probe("liburing") {
        Ok(lib) => lib,
        Err(e) => {
            println!(
                "cargo:warning=Couldn't find liburing from pkg-config ({:?})",
                e
            );
            return false;
        }
    };

    // Re-probe with metadata enabled
    let mut cfg = pkg_config::Config::new();
    cfg.atleast_version(min_version);
    cfg.probe("liburing").unwrap();

    // Generate bindings from system headers
    if let Some(include_path) = lib.include_paths.first() {
        let header = include_path.join("liburing.h");
        generate_bindings(&header, include_path);
    }

    true
}

fn generate_bindings(header_path: &Path, include_dir: &Path) {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let bindings = bindgen::Builder::default()
        .header(header_path.to_str().unwrap())
        .clang_arg(format!("-I{}", include_dir.display()))
        // Generate bindings for liburing types and functions
        .allowlist_type("io_uring.*")
        .allowlist_type("__kernel_timespec")
        .allowlist_function("io_uring.*")
        .allowlist_var("IORING_.*")
        .allowlist_var("IOSQE_.*")
        .allowlist_var("IO_URING_.*")
        // Generate rust enum types for C enums
        .rustified_enum("io_uring_op")
        .rustified_enum("io_uring_register_op")
        .rustified_enum("io_uring_msg_ring_flags")
        // Derive common traits
        .derive_debug(true)
        .derive_default(true)
        // Layout tests
        .layout_tests(false)
        // Generate comments from headers
        .generate_comments(true)
        // Parse callbacks
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Tell cargo to rerun if headers change
    println!("cargo:rerun-if-changed={}", header_path.display());
}

fn build_from_source(min_version: &str) {
    use std::process::Command;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let liburing_dir = out_dir.join("liburing-src");

    // Clone liburing if not already present
    if !liburing_dir.exists() {
        println!(
            "cargo:warning=Cloning liburing {} from GitHub...",
            min_version
        );

        let status = Command::new("git")
            .args([
                "clone",
                "--depth=1",
                "--branch",
                &format!("liburing-{}", min_version),
                "https://github.com/axboe/liburing.git",
                liburing_dir.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to execute git clone");

        if !status.success() {
            panic!("Failed to clone liburing from GitHub");
        }
    }

    // Build liburing
    println!("cargo:warning=Building liburing from source...");

    let configure_status = Command::new("./configure")
        .current_dir(&liburing_dir)
        .status()
        .expect("Failed to run ./configure");

    if !configure_status.success() {
        panic!("Failed to configure liburing");
    }

    let make_status = Command::new("make")
        .current_dir(&liburing_dir)
        .arg("-j")
        .status()
        .expect("Failed to run make");

    if !make_status.success() {
        panic!("Failed to build liburing");
    }

    // Set up link paths
    let lib_path = liburing_dir.join("src");
    println!("cargo:rustc-link-search=native={}", lib_path.display());

    // Generate bindings
    let header_path = liburing_dir.join("src/include/liburing.h");
    let include_dir = liburing_dir.join("src/include");
    generate_bindings(&header_path, &include_dir);

    println!(
        "cargo:warning=Successfully built liburing {} from source",
        min_version
    );
}
