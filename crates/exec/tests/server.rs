#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::{Duration, Instant};

use common::{Raw, pbc, start};
use exec::frame::FrameType;
use exec::message::{ErrorCode, valid_session_id};
use exec::server::ServerConfig;
use pbc::Instr as I;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn default_server() -> std::net::SocketAddr {
    start(ServerConfig::default())
}

#[test]
fn open_gets_ready() {
    let mut c = Raw::connect(default_server());
    c.send_json(FrameType::Open, r#"{"proto":1,"client":"test"}"#);
    assert_eq!(
        c.recv_json(FrameType::Ready),
        json!({"proto": 1, "spec": "PBC1", "spec_version": "1.9.0", "sessions_max": 16})
    );
}

#[test]
fn an_unsupported_proto_closes() {
    let mut c = Raw::connect(default_server());
    c.send_json(FrameType::Open, r#"{"proto":2}"#);
    c.expect_error(ErrorCode::BadProto);
    c.assert_closed();
}

#[test]
fn greeting_only_accepts_open() {
    let mut c = Raw::connect(default_server());
    c.send(FrameType::Submit, &pbc(&[I::Halt]));
    c.expect_error(ErrorCode::UnexpectedMessage);
    c.assert_closed();
}

#[test]
fn invalid_json_is_a_bad_frame() {
    let mut c = Raw::connect(default_server());
    c.send_json(FrameType::Open, "{");
    c.expect_error(ErrorCode::BadFrame);
    c.assert_closed();
}

#[test]
fn a_second_open_is_unexpected() {
    let mut c = Raw::open(default_server());
    c.send_json(FrameType::Open, r#"{"proto":1}"#);
    c.expect_error(ErrorCode::UnexpectedMessage);
    c.assert_closed();
}

#[test]
fn submit_answers_with_a_checksum() {
    let bytes = pbc(&[I::Halt]);
    let mut c = Raw::open(default_server());
    c.send(FrameType::Submit, &bytes);
    let session = c.recv_json(FrameType::Session);
    let hex: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(session["pbc_sha256"], hex.as_str());
    assert_eq!(session["code_len"], 1);
    assert!(valid_session_id(session["session"].as_str().unwrap()));
}

#[test]
fn two_submits_get_two_ids() {
    // D-008 : compteur par connexion.
    let mut c = Raw::open(default_server());
    let first = c.submit(&pbc(&[I::Halt]));
    let second = c.submit(&pbc(&[I::Halt]));
    assert_ne!(first, second);
}

#[test]
fn an_unloadable_program_is_bad_program() {
    let mut c = Raw::open(default_server());
    c.send(FrameType::Submit, b"PBC0");
    c.expect_error(ErrorCode::BadProgram);
    c.assert_closed();
}

#[test]
fn exec_returns_the_result() {
    let mut c = Raw::open(default_server());
    let s = c.submit(&pbc(&[I::PushI16(7), I::Trace, I::Halt]));
    c.exec(&s, 1000);
    let r = c.recv_json(FrameType::Result);
    assert_eq!(r["gas_used"], 2);
    assert_eq!(r["trace"], json!([7]));
    assert_eq!(r["fault"], Value::Null);
    assert_eq!(r["act"], Value::Null);
    assert_eq!(r["mem"].as_array().unwrap().len(), 256);
}

#[test]
fn callbacks_round_trip_in_order() {
    let mut c = Raw::open(default_server());
    let s = c.submit(&pbc(&[
        I::Sense(6),
        I::Rand,
        I::Add,
        I::Act(0),
        I::Trace,
        I::Halt,
    ]));
    c.exec(&s, 1000);
    assert_eq!(c.recv_json(FrameType::Sense), json!({"k": 6}));
    c.send_json(FrameType::Sensed, r#"{"value":1}"#);
    assert_eq!(c.recv_json(FrameType::Rand), json!({}));
    c.send_json(FrameType::Drew, r#"{"value":2}"#);
    assert_eq!(
        c.recv_json(FrameType::Act),
        json!({"kind": "MOVE", "arg": 3})
    );
    c.send_json(FrameType::Acted, r#"{"ok":true}"#);
    let r = c.recv_json(FrameType::Result);
    assert_eq!(r["trace"], json!([1]));
    assert_eq!(r["act"], json!({"kind": "MOVE", "arg": 3, "ok": true}));
    assert_eq!(r["gas_used"], 8 + 5 + 2 + 12 + 1);
}

#[test]
fn a_session_runs_many_times_with_identical_results() {
    let mut c = Raw::open(default_server());
    let s = c.submit(&pbc(&[I::PushI16(1), I::Trace, I::Halt]));
    c.exec(&s, 1000);
    let first = c.recv();
    c.exec(&s, 1000);
    assert_eq!(c.recv(), first);
}

#[test]
fn an_unknown_session_keeps_the_connection() {
    let mut c = Raw::open(default_server());
    let s = c.submit(&pbc(&[I::Halt]));
    c.exec("nope", 1000);
    c.expect_error(ErrorCode::NoSuchSession);
    c.exec(&s, 1000);
    c.recv_json(FrameType::Result);
}

#[test]
fn bad_memory_closes() {
    let mut c = Raw::open(default_server());
    let s = c.submit(&pbc(&[I::Halt]));
    let mem = vec!["0"; 255].join(",");
    c.send_json(
        FrameType::Exec,
        &format!(r#"{{"session":"{s}","mem":[{mem}],"budget":1000}}"#),
    );
    c.expect_error(ErrorCode::BadMemory);
    c.assert_closed();
}

#[test]
fn bad_budgets_close() {
    let addr = default_server();
    for budget in [0, 65_536, -1] {
        let mut c = Raw::open(addr);
        let s = c.submit(&pbc(&[I::Halt]));
        c.exec(&s, budget);
        c.expect_error(ErrorCode::BadBudget);
        c.assert_closed();
    }
}

#[test]
fn a_malformed_session_id_is_a_bad_frame() {
    let mut c = Raw::open(default_server());
    c.exec("a b", 1000);
    c.expect_error(ErrorCode::BadFrame);
    c.assert_closed();
}

#[test]
fn exec_validates_the_payload_before_the_session() {
    // D-007 : session inconnue ET mémoire invalide → bad_memory, puis fermeture.
    let mut c = Raw::open(default_server());
    c.send_json(
        FrameType::Exec,
        r#"{"session":"nope","mem":[0],"budget":1000}"#,
    );
    c.expect_error(ErrorCode::BadMemory);
    c.assert_closed();
}

#[test]
fn too_many_sessions_keeps_the_connection() {
    let addr = start(ServerConfig {
        sessions_max: 2,
        ..ServerConfig::default()
    });
    let mut c = Raw::open(addr);
    let first = c.submit(&pbc(&[I::Halt]));
    c.submit(&pbc(&[I::Halt]));
    c.send(FrameType::Submit, &pbc(&[I::Halt]));
    c.expect_error(ErrorCode::TooManySessions);
    c.exec(&first, 1000);
    c.recv_json(FrameType::Result);
}

#[test]
fn executing_accepts_only_the_pending_reply() {
    let mut c = Raw::open(default_server());
    let s = c.submit(&pbc(&[I::Sense(0), I::Halt]));
    c.exec(&s, 1000);
    c.recv_json(FrameType::Sense);
    c.send_json(FrameType::Acted, r#"{"ok":true}"#);
    c.expect_error(ErrorCode::UnexpectedMessage);
    c.assert_closed();
}

#[test]
fn sensed_naming_a_load_fault_is_a_bad_frame() {
    let mut c = Raw::open(default_server());
    let s = c.submit(&pbc(&[I::Sense(0), I::Halt]));
    c.exec(&s, 1000);
    c.recv_json(FrameType::Sense);
    c.send_json(FrameType::Sensed, r#"{"fault":"BadHeader"}"#);
    c.expect_error(ErrorCode::BadFrame);
    c.assert_closed();
}

#[test]
fn a_sensed_fault_ends_the_turn() {
    let mut c = Raw::open(default_server());
    let s = c.submit(&pbc(&[I::Sense(99), I::Halt]));
    c.exec(&s, 1000);
    assert_eq!(c.recv_json(FrameType::Sense), json!({"k": 99}));
    c.send_json(FrameType::Sensed, r#"{"fault":"BadSensor"}"#);
    let r = c.recv_json(FrameType::Result);
    assert_eq!(
        (r["fault"].as_str(), r["gas_used"].as_i64()),
        (Some("BadSensor"), Some(8))
    );
}

#[test]
fn an_unknown_frame_type_is_a_bad_frame() {
    let mut c = Raw::open(default_server());
    c.send_raw(&[0x42, 0, 0, 0, 0]);
    c.expect_error(ErrorCode::BadFrame);
    c.assert_closed();
}

#[test]
fn an_oversized_frame_is_refused_from_its_header() {
    let mut c = Raw::open(default_server());
    c.send_raw(&[0x13, 0x00, 0x01, 0x80, 0x04]); // SUBMIT de 98 308 octets annoncés, aucun envoyé
    c.expect_error(ErrorCode::BadFrame);
    c.assert_closed();
}

#[test]
fn a_cut_connection_does_not_stop_the_server() {
    let addr = default_server();
    {
        let mut c = Raw::connect(addr);
        c.send_raw(&[0x11, 0x00]); // fermée au milieu d'un en-tête
    }
    Raw::open(addr); // le client suivant est servi
}

#[test]
fn a_silent_client_is_dropped_after_the_read_timeout() {
    let addr = start(ServerConfig {
        read_timeout: Some(Duration::from_millis(200)),
        ..ServerConfig::default()
    });
    let _silent = Raw::connect(addr); // ne dit rien, et reste ouvert
    let started = Instant::now();
    Raw::open(addr);
    assert!(started.elapsed() < Duration::from_secs(3));
}
