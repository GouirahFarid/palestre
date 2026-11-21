#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use exec::message::{
    self, ActReq, ErrorCode, ErrorMsg, Exec, Open, RandReq, Ready, ResultMsg, Sensed,
    valid_session_id,
};
use pbc::Fault;
use vm::{ActKind, ActRecord, Action, Report};

fn json<T: serde::Serialize>(msg: &T) -> String {
    String::from_utf8(message::to_json(msg).unwrap()).unwrap()
}

#[test]
fn ready_is_compact_and_in_k3_order() {
    let ready = Ready {
        proto: 1,
        spec: "PBC1".to_string(),
        spec_version: pbc::SPEC_VERSION.to_string(),
        sessions_max: 16,
    };
    assert_eq!(
        json(&ready),
        r#"{"proto":1,"spec":"PBC1","spec_version":"1.9.0","sessions_max":16}"#
    );
}

#[test]
fn result_round_trips_through_a_report() {
    let mut mem = [0; 256];
    mem[2] = 98;
    let report = Report {
        mem,
        gas_used: 137,
        fault: None,
        act: Some(ActRecord {
            action: Action {
                kind: ActKind::Move,
                arg: 0,
            },
            ok: true,
        }),
        trace: vec![1, 7],
    };
    let text = json(&ResultMsg::from_report(&report));
    assert!(text.starts_with(r#"{"mem":[0,0,98,0,"#), "{text}");
    assert!(
        text.ends_with(
            r#"],"gas_used":137,"fault":null,"act":{"kind":"MOVE","arg":0,"ok":true},"trace":[1,7]}"#
        ),
        "{text}"
    );
    let back = message::from_json::<ResultMsg>(text.as_bytes())
        .unwrap()
        .to_report()
        .unwrap();
    assert_eq!(back, report);
}

#[test]
fn a_faulted_result_without_action() {
    let report = Report {
        mem: [0; 256],
        gas_used: 3,
        fault: Some(Fault::BadJump),
        act: None,
        trace: vec![],
    };
    let text = json(&ResultMsg::from_report(&report));
    assert!(
        text.ends_with(r#""gas_used":3,"fault":"BadJump","act":null,"trace":[]}"#),
        "{text}"
    );
}

#[test]
fn sensed_carries_a_value_or_a_fault_never_both() {
    assert_eq!(json(&Sensed::from_answer(Ok(-1))), r#"{"value":-1}"#);
    assert_eq!(
        json(&Sensed::from_answer(Err(Fault::BadSensor))),
        r#"{"fault":"BadSensor"}"#
    );
    let parse = |text: &str| {
        message::from_json::<Sensed>(text.as_bytes())
            .unwrap()
            .to_answer()
    };
    assert_eq!(parse(r#"{"value":7}"#).unwrap(), Ok(7));
    assert_eq!(
        parse(r#"{"fault":"DivByZero"}"#).unwrap(),
        Err(Fault::DivByZero)
    );
    assert!(parse(r#"{"value":1,"fault":"BadSensor"}"#).is_err());
    assert!(parse("{}").is_err());
    assert!(parse(r#"{"fault":"BadHeader"}"#).is_err());
    assert!(parse(r#"{"fault":"Nope"}"#).is_err());
}

#[test]
fn act_requests_name_the_kind() {
    assert_eq!(
        json(&ActReq::from_action(Action {
            kind: ActKind::Eat,
            arg: -3
        })),
        r#"{"kind":"EAT","arg":-3}"#
    );
    let req: ActReq = message::from_json(br#"{"kind":"DROP","arg":2}"#).unwrap();
    assert_eq!(
        req.to_action().unwrap(),
        Action {
            kind: ActKind::Drop,
            arg: 2
        }
    );
    let req: ActReq = message::from_json(br#"{"kind":"FLY","arg":0}"#).unwrap();
    assert!(req.to_action().is_err());
}

#[test]
fn rand_and_error_shapes() {
    assert_eq!(json(&RandReq {}), "{}");
    let err = ErrorMsg {
        code: ErrorCode::NoSuchSession,
        message: "inconnue".to_string(),
    };
    assert_eq!(
        json(&err),
        r#"{"code":"no_such_session","message":"inconnue"}"#
    );
}

#[test]
fn only_two_error_codes_keep_the_connection() {
    use ErrorCode::*;
    for code in [
        BadFrame,
        BadProto,
        BadProgram,
        BadMemory,
        BadBudget,
        UnexpectedMessage,
    ] {
        assert!(code.closes_connection(), "{code:?}");
    }
    for code in [TooManySessions, NoSuchSession] {
        assert!(!code.closes_connection(), "{code:?}");
    }
}

#[test]
fn parsing_tolerates_unknown_fields() {
    let open: Open = message::from_json(br#"{"proto":1,"client":"ref","extra":true}"#).unwrap();
    assert_eq!(
        open,
        Open {
            proto: 1,
            client: Some("ref".to_string())
        }
    );
    let open: Open = message::from_json(br#"{"proto":1}"#).unwrap();
    assert_eq!(open.client, None);
}

#[test]
fn exec_payloads_parse() {
    let mut text = String::from(r#"{"session":"1","mem":["#);
    text.push_str(&vec!["0"; 256].join(","));
    text.push_str(r#"],"budget":1000}"#);
    let exec: Exec = message::from_json(text.as_bytes()).unwrap();
    assert_eq!(
        (exec.session.as_str(), exec.mem.len(), exec.budget),
        ("1", 256, 1000)
    );
    assert!(message::from_json::<Exec>(br#"{"session":"1","mem":[1.5],"budget":1}"#).is_err());
}

#[test]
fn session_ids_follow_k4() {
    for ok in ["1", "abc-DEF_9", &"a".repeat(64)] {
        assert!(valid_session_id(ok), "{ok}");
    }
    for bad in ["", &"a".repeat(65), "a b", "é", "a/b"] {
        assert!(!valid_session_id(bad), "{bad}");
    }
}
