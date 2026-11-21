#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::Command;

#[test]
fn corpus_subcommand_runs_the_public_cases_in_process() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    let out = Command::new(env!("CARGO_BIN_EXE_palestre"))
        .args(["corpus", root])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("65/65 cas conformes"), "{stdout}");
}
