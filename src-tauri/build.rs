use std::env;
use std::path::PathBuf;
use std::process::Command;

fn zig_target(target: &str) -> &str {
    match target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
        "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
        "x86_64-apple-darwin" => "x86_64-macos",
        "aarch64-apple-darwin" => "aarch64-macos",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        other => panic!("unsupported target for libghostty-vt: {other}"),
    }
}

fn zig_executable() -> PathBuf {
    if let Some(path) = env::var_os("ZIG") {
        return path.into();
    }
    for path in [
        "/opt/homebrew/opt/zig@0.15/bin/zig",
        "/usr/local/opt/zig@0.15/bin/zig",
    ] {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }
    "zig".into()
}

fn build_libghostty_vt() {
    println!("cargo:rerun-if-changed=vendor/libghostty-vt/build.zig");
    println!("cargo:rerun-if-changed=vendor/libghostty-vt/build.zig.zon");
    println!("cargo:rerun-if-changed=vendor/libghostty-vt/include");
    println!("cargo:rerun-if-changed=vendor/libghostty-vt/pkg");
    println!("cargo:rerun-if-changed=vendor/libghostty-vt/src");
    println!("cargo:rerun-if-changed=vendor/libghostty-vt/VERSION");
    println!("cargo:rerun-if-env-changed=ZIG");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let vendored_dir = manifest_dir.join("vendor/libghostty-vt");
    let target = env::var("TARGET").expect("TARGET");
    let prebuilt = vendored_dir
        .join("prebuilt")
        .join(&target)
        .join("libghostty-vt.a");
    if prebuilt.is_file() {
        println!("cargo:rerun-if-changed={}", prebuilt.display());
        println!("cargo:rustc-link-arg={}", prebuilt.display());
        return;
    }

    let version =
        std::fs::read_to_string(vendored_dir.join("VERSION")).expect("read libghostty-vt VERSION");
    let zig = zig_executable();

    let status = Command::new(&zig)
        .current_dir(&vendored_dir)
        .args([
            "build",
            "-Demit-lib-vt",
            "-Doptimize=ReleaseFast",
            "-Dsimd=true",
            &format!("-Dtarget={}", zig_target(&target)),
            &format!("-Dversion-string={}", version.trim()),
            "-Demit-xcframework=false",
        ])
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run Zig for libghostty-vt ({error}). Install Zig 0.15.2 or set ZIG=/path/to/zig"
            )
        });
    assert!(status.success(), "libghostty-vt Zig build failed: {status}");

    let lib_dir = vendored_dir.join("zig-out/lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    if target.contains("apple-darwin") {
        println!(
            "cargo:rustc-link-arg={}",
            lib_dir.join("libghostty-vt.a").display()
        );
    } else if target.contains("windows-msvc") {
        println!("cargo:rustc-link-lib=static=ghostty-vt-static");
    } else {
        println!("cargo:rustc-link-lib=static=ghostty-vt");
    }
}

fn main() {
    build_libghostty_vt();
    tauri_build::build();
}
