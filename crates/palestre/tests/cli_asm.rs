#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::Command;

use serde_json::json;

const BIN: &str = env!("CARGO_BIN_EXE_palestre");

#[test]
fn asm_then_disasm_round_trips_through_files() {
    let dir = std::env::temp_dir().join(format!("palestre-cli-asm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("agent.asm");
    let pbc = dir.join("agent.pbc");
    std::fs::write(&src, "PUSHI16 7\nTRACE\nHALT\n").unwrap();
    assert!(
        Command::new(BIN)
            .arg("asm")
            .arg(&src)
            .arg("-o")
            .arg(&pbc)
            .status()
            .unwrap()
            .success()
    );
    let out = Command::new(BIN).arg("disasm").arg(&pbc).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("PUSHI16 7") && text.contains("TRACE"),
        "{text}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn run_prints_an_expected_json_execution() {
    let program = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/nominal/207-jump-if-else-on-the-agent-id/program.pbc"
    );
    let out = Command::new(BIN)
        .args(["run", program, "--sense", "6=1"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["calls"], json!([{"call": "SENSE", "k": 6, "value": 1}]));
    assert_eq!(v["result"]["trace"], json!([200]));
    assert_eq!(v["result"]["gas_used"], 12);
}

#[test]
fn asm_errors_name_the_line() {
    let dir = std::env::temp_dir().join(format!("palestre-cli-asm-err-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("faux.asm");
    std::fs::write(&src, "HALT\nFLY\n").unwrap();
    let out = Command::new(BIN)
        .arg("asm")
        .arg(&src)
        .arg("-o")
        .arg(dir.join("x.pbc"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains(":2"));
    std::fs::remove_dir_all(&dir).unwrap();
}
