fn main() {
    #[cfg(unix)]
    build_qrun_detect();
}

#[cfg(unix)]
fn build_qrun_detect() {
    use std::path::PathBuf;
    use std::process::Command;

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let detect_manifest = PathBuf::from(&manifest_dir)
        .parent()
        .expect("qrun has no parent dir")
        .join("qrun-detect")
        .join("Cargo.toml");

    // Use a sub-directory of OUT_DIR as target to avoid locking the outer target dir
    let detect_target = PathBuf::from(&out_dir).join("detect_target");

    let mut cmd = Command::new(&cargo);
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(&detect_manifest)
        .arg("--target-dir")
        .arg(&detect_target);
    if profile == "release" {
        cmd.arg("--release");
    }
    // Suppress cargo output noise in build logs
    cmd.env("CARGO_TERM_COLOR", "never");

    let status = cmd.status().expect("Failed to run cargo to build qrun-detect");
    assert!(status.success(), "qrun-detect build failed");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let lib_name = if target_os == "macos" {
        "libqrun_detect.dylib"
    } else {
        "libqrun_detect.so"
    };

    let so_path = detect_target.join(&profile).join(lib_name);

    println!("cargo:rustc-env=QRUN_DETECT_SO={}", so_path.display());
    println!(
        "cargo:rerun-if-changed={}",
        detect_manifest.parent().unwrap().join("src/lib.rs").display()
    );
    println!("cargo:rerun-if-changed={}", detect_manifest.display());
}
