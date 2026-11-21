#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::num::NonZeroU16;

use common::{pbc, start};
use exec::client::{Client, ClientError};
use exec::message::ErrorCode;
use exec::server::ServerConfig;
use pbc::{Fault, Instr as I};
use vm::{ActKind, ActRecord, Action, Env};

struct Echo;

impl Env for Echo {
    fn sense(&mut self, k: u8) -> Result<i64, Fault> {
        Ok(i64::from(k))
    }
    fn act(&mut self, action: Action) -> bool {
        action.arg == 6
    }
    fn rand(&mut self) -> i64 {
        0
    }
}

#[test]
fn the_client_answers_callbacks_through_an_env() {
    let mut client = Client::connect(start(ServerConfig::default())).unwrap();
    assert_eq!(client.ready().spec_version, "1.9.0");
    let session = client
        .submit(&pbc(&[I::Sense(6), I::Act(0), I::Trace, I::Halt]))
        .unwrap();
    let report = client
        .exec(
            &session.session,
            &[0; 256],
            NonZeroU16::new(1000).unwrap(),
            &mut Echo,
        )
        .unwrap();
    assert_eq!(report.trace, vec![1]);
    assert_eq!(
        report.act,
        Some(ActRecord {
            action: Action {
                kind: ActKind::Move,
                arg: 6
            },
            ok: true
        })
    );
}

#[test]
fn server_errors_surface_as_typed_errors() {
    let mut client = Client::connect(start(ServerConfig::default())).unwrap();
    match client.submit(b"garbage") {
        Err(ClientError::Server(err)) => assert_eq!(err.code, ErrorCode::BadProgram),
        other => panic!("attendu bad_program, reçu {other:?}"),
    }
}
