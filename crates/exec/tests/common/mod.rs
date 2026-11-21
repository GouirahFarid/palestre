#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use exec::frame::{Frame, FrameType, SERVER_MAX_LEN, read_frame, write_frame};
use exec::message::{self, ErrorCode, ErrorMsg};
use exec::server::{ServerConfig, serve};
use pbc::{Instr, Program};

pub fn start(config: ServerConfig) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || serve(listener, &config));
    addr
}

pub fn pbc(instrs: &[Instr]) -> Vec<u8> {
    let mut code = Vec::new();
    for instr in instrs {
        pbc::encode(*instr, &mut code);
    }
    Program::new(vec![], code).unwrap().to_bytes()
}

pub struct Raw(pub TcpStream);

impl Raw {
    pub fn connect(addr: SocketAddr) -> Raw {
        let stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Raw(stream)
    }

    /// Se connecte et passe l'accueil (OPEN → READY).
    pub fn open(addr: SocketAddr) -> Raw {
        let mut raw = Raw::connect(addr);
        raw.send_json(FrameType::Open, r#"{"proto":1}"#);
        raw.recv_json(FrameType::Ready);
        raw
    }

    pub fn send(&mut self, kind: FrameType, payload: &[u8]) {
        write_frame(&mut self.0, kind, payload).unwrap();
    }

    pub fn send_json(&mut self, kind: FrameType, json: &str) {
        self.send(kind, json.as_bytes());
    }

    pub fn send_raw(&mut self, bytes: &[u8]) {
        self.0.write_all(bytes).unwrap();
    }

    pub fn recv(&mut self) -> Frame {
        read_frame(&mut self.0, SERVER_MAX_LEN).unwrap()
    }

    pub fn recv_json(&mut self, kind: FrameType) -> serde_json::Value {
        let frame = self.recv();
        assert_eq!(
            frame.kind,
            kind,
            "reçu : {}",
            String::from_utf8_lossy(&frame.payload)
        );
        serde_json::from_slice(&frame.payload).unwrap()
    }

    pub fn expect_error(&mut self, code: ErrorCode) {
        let frame = self.recv();
        assert_eq!(
            frame.kind,
            FrameType::Error,
            "reçu : {}",
            String::from_utf8_lossy(&frame.payload)
        );
        let err: ErrorMsg = message::from_json(&frame.payload).unwrap();
        assert_eq!(err.code, code, "{}", err.message);
    }

    pub fn assert_closed(&mut self) {
        let mut byte = [0u8; 1];
        match self.0.read(&mut byte) {
            Ok(0) | Err(_) => {}
            Ok(_) => panic!("la connexion aurait dû être fermée"),
        }
    }

    pub fn submit(&mut self, bytes: &[u8]) -> String {
        self.send(FrameType::Submit, bytes);
        self.recv_json(FrameType::Session)["session"]
            .as_str()
            .unwrap()
            .to_string()
    }

    pub fn exec(&mut self, session: &str, budget: i64) {
        let mem = vec!["0"; 256].join(",");
        self.send_json(
            FrameType::Exec,
            &format!(r#"{{"session":"{session}","mem":[{mem}],"budget":{budget}}}"#),
        );
    }
}
