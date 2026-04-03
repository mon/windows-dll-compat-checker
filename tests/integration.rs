use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn cmd() -> Command {
    Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}

fn test_libs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test_libs")
}

fn winxp_ini() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("premade_ini/winxp_x86_64_32bit_dlls.ini")
}

fn extend_ini(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test_libs/extend").join(name)
}

#[test]
fn valid_imports_pass() {
    let libs = test_libs();
    cmd()
        .arg(libs.join("test_input.dll"))
        .args(["--system", libs.join("test_system.dll").to_str().unwrap()])
        .args(["--system", libs.join("test_transitive.dll").to_str().unwrap()])
        .args(["--system", winxp_ini().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn missing_transitive_dll_fails() {
    let libs = test_libs();
    cmd()
        .arg(libs.join("test_input.dll"))
        .args(["--system", libs.join("test_system.dll").to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn transitive_import_from_input_dll_passes() {
    let libs = test_libs();
    cmd()
        .arg(libs.join("test_input.dll"))
        .arg(libs.join("test_transitive.dll"))
        .args(["--system", libs.join("test_system.dll").to_str().unwrap()])
        .args(["--system", winxp_ini().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn partial_system_dll_fails() {
    let libs = test_libs();
    cmd()
        .arg(libs.join("test_input.dll"))
        .args(["--system", libs.join("partial").join("test_system.dll").to_str().unwrap()])
        .args(["--system", libs.join("test_transitive.dll").to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn two_layer_extend_resolves_all_imports() {
    let libs = test_libs();
    cmd()
        .arg(libs.join("test_input.dll"))
        .args(["--system", extend_ini("top.ini").to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn two_layer_extend_missing_dll_fails() {
    let libs = test_libs();
    cmd()
        .arg(libs.join("test_input.dll"))
        .args(["--system", extend_ini("base.ini").to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn os_version_at_limit_passes() {
    let libs = test_libs();
    cmd()
        .arg(libs.join("test_input.exe"))
        .args(["--system", libs.join("test_system.dll").to_str().unwrap()])
        .args(["--system", libs.join("test_transitive.dll").to_str().unwrap()])
        .args(["--system", winxp_ini().to_str().unwrap()])
        .args(["--os-version", "5,1"])
        .assert()
        .success();
}

#[test]
fn os_version_exceeded_fails() {
    let libs = test_libs();
    cmd()
        .arg(libs.join("test_input.exe"))
        .args(["--system", libs.join("test_system.dll").to_str().unwrap()])
        .args(["--system", libs.join("test_transitive.dll").to_str().unwrap()])
        .args(["--system", winxp_ini().to_str().unwrap()])
        .args(["--os-version", "5,0"])
        .assert()
        .failure();
}
