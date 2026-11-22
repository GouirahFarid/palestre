#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_palestre");

#[test]
fn match_subcommand_reports_the_winner_and_writes_a_log() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let dir = std::env::temp_dir().join(format!("palestre-cli-match-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["harvester", "idler"] {
        let ok = Command::new(BIN)
            .arg("asm")
            .arg(format!("{root}/agents/{name}.asm"))
            .arg("-o")
            .arg(dir.join(format!("{name}.pbc")))
            .status()
            .unwrap()
            .success();
        assert!(ok);
    }
    let log = dir.join("match.json");
    let out = Command::new(BIN)
        .arg("match")
        .arg(dir.join("harvester.pbc"))
        .arg(dir.join("idler.pbc"))
        .args(["--seed", "1", "--log"])
        .arg(&log)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("vainqueur : agent 0"), "{stdout}");
    assert!(arena::MatchLog::from_json(&std::fs::read_to_string(&log).unwrap()).is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}
