#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use exec::frame::{FrameType, SERVER_MAX_LEN, read_frame, write_frame};

/// Lance `palestre exec` sur un port libre et rend le processus et l'adresse annoncée.
fn spawn(extra: &[&str]) -> (Child, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_palestre"))
        .args(["exec", "--listen", "127.0.0.1:0"])
        .args(extra)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let addr = line.trim().rsplit(' ').next().unwrap().to_string();
    (child, addr)
}

/// `OPEN`, puis le type de la trame reçue en réponse ; échoue au bout de deux secondes sans réponse.
fn open(addr: &str) -> FrameType {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write_frame(&mut stream, FrameType::Open, br#"{"proto":1}"#).unwrap();
    read_frame(&mut stream, SERVER_MAX_LEN).unwrap().kind
}

#[test]
fn exec_subcommand_listens_and_answers() {
    let (mut child, addr) = spawn(&[]);
    assert_eq!(open(&addr), FrameType::Ready);
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn concurrent_flag_serves_a_client_while_another_is_silent() {
    let (mut child, addr) = spawn(&["--concurrent", "--read-timeout", "0"]);
    let _silent = TcpStream::connect(&addr).unwrap();
    assert_eq!(open(&addr), FrameType::Ready);
    child.kill().unwrap();
    child.wait().unwrap();
}
