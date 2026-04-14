use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        return;
    }

    let swift_src = PathBuf::from("macos/ScreenCapture.swift");
    println!("cargo:rerun-if-changed={}", swift_src.display());
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let swift_target = match target_arch.as_str() {
        "aarch64" => "arm64-apple-macos13.0",
        "x86_64" => "x86_64-apple-macos13.0",
        other => panic!("unsupported macOS arch: {other}"),
    };

    let dylib = out_dir.join("libspectra_sc.dylib");

    let status = Command::new("swiftc")
        .args([
            "-parse-as-library",
            "-emit-library",
            "-O",
            "-module-name",
            "SpectraSC",
            "-target",
            swift_target,
            "-Xlinker",
            "-install_name",
            "-Xlinker",
            "@rpath/libspectra_sc.dylib",
            "-o",
        ])
        .arg(&dylib)
        .arg(&swift_src)
        .status()
        .expect("failed to run swiftc — install Xcode Command Line Tools (`xcode-select --install`)");
    assert!(status.success(), "swiftc build failed");

    // Copy the dylib next to the final binary so `cargo run` / `cargo install` work
    // with @executable_path rpath without extra steps.
    if let Some(profile_dir) = out_dir.ancestors().nth(3) {
        let dest = profile_dir.join("libspectra_sc.dylib");
        let _ = std::fs::copy(&dylib, &dest);
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=dylib=spectra_sc");
    println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
    println!("cargo:rustc-link-lib=framework=CoreMedia");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../lib");
}
