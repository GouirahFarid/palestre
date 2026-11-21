#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Command, Stdio};

use exec::frame::{FrameType, SERVER_MAX_LEN, read_frame, write_frame};

#[test]
fn exec_subcommand_listens_and_answers() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_palestre"))
        .args(["exec", "--listen", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let addr = line.trim().rsplit(' ').next().unwrap().to_string();
    let mut stream = TcpStream::connect(addr).unwrap();
    write_frame(&mut stream, FrameType::Open, br#"{"proto":1}"#).unwrap();
    assert_eq!(
        read_frame(&mut stream, SERVER_MAX_LEN).unwrap().kind,
        FrameType::Ready
    );
    child.kill().unwrap();
    child.wait().unwrap();
}
