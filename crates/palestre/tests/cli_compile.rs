#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_palestre");

/// Un dossier temporaire propre à ce test.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "palestre-cli-compile-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn compile_writes_a_program_that_verify_accepts() {
    let dir = temp("ok");
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../agents/harvester.agon");
    let output = dir.join("harvester.pbc");
    let out = Command::new(BIN)
        .arg("compile")
        .arg(source)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    pbc::Program::load(&std::fs::read(&output).unwrap()).unwrap();
    let verified = Command::new(BIN)
        .arg("verify")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stdout)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn emit_asm_prints_the_generated_assembly() {
    let dir = temp("asm");
    let source = dir.join("one.agon");
    std::fs::write(&source, "fn main() { trace(1); }\n").unwrap();
    let out = Command::new(BIN)
        .arg("compile")
        .arg(&source)
        .arg("-o")
        .arg(dir.join("one.pbc"))
        .arg("--emit-asm")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("CALL fn_main"), "{stdout}");
    assert!(stdout.contains("; agon:1"), "{stdout}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn errors_are_printed_with_the_file_and_line() {
    let dir = temp("err");
    let source = dir.join("bad.agon");
    std::fs::write(&source, "fn main() {\n  trace(x);\n}\n").unwrap();
    let out = Command::new(BIN)
        .arg("compile")
        .arg(&source)
        .arg("-o")
        .arg(dir.join("bad.pbc"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr.contains("bad.agon:2 : variable inconnue « x »"),
        "{stderr}"
    );
    assert!(!dir.join("bad.pbc").exists());

    std::fs::write(&source, "fn helper() {}\n").unwrap();
    let out = Command::new(BIN)
        .arg("compile")
        .arg(&source)
        .arg("-o")
        .arg(dir.join("bad.pbc"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr.contains("il manque la fonction « main »"),
        "{stderr}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
