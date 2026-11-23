#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::Command;

use pbc::Program;

const BIN: &str = env!("CARGO_BIN_EXE_palestre");

#[test]
fn verify_announces_the_call_stack_overflow_of_case_023() {
    let program = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/edge/023-call-stack-overflow/program.pbc"
    );
    let out = Command::new(BIN)
        .args(["verify", program])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("CallStackOverflow"), "{stdout}");
    assert!(stdout.contains("appels max : 16"), "{stdout}");
}

#[test]
fn verify_accepts_a_safe_program() {
    let path = std::env::temp_dir().join(format!("palestre-cli-verify-{}.pbc", std::process::id()));
    // PUSHI16 7 | TRACE | HALT
    let program = Program::new(vec![], vec![0x02, 0x00, 0x07, 0xF0, 0x00]).unwrap();
    std::fs::write(&path, program.to_bytes()).unwrap();
    let out = Command::new(BIN).arg("verify").arg(&path).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    std::fs::remove_file(&path).unwrap();
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("pile max : 1, appels max : 0, états : 3"),
        "{stdout}"
    );
}
