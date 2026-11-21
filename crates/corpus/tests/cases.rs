#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use corpus::{Answer, Call, CaseError, Expectation, comparable, load_case, load_corpus};
use pbc::Fault;
use serde_json::json;
use vm::{ActKind, Action};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

#[test]
fn the_public_corpus_loads() {
    let cases = load_corpus(&corpus_dir()).unwrap();
    assert_eq!(cases.len(), 65);
    let refused = cases
        .iter()
        .filter(|c| matches!(c.expectation, Expectation::Refused { .. }))
        .count();
    assert_eq!(refused, 3);
    assert!(cases.windows(2).all(|w| w[0].name < w[1].name));
}

#[test]
fn a_case_is_fully_decoded() {
    let case = load_case(&corpus_dir().join("edge/016-gas-exact-exhaustion")).unwrap();
    assert_eq!(
        (case.category.as_str(), case.clause.as_str()),
        ("gas", "B.2")
    );
    let Expectation::Runs(execs) = &case.expectation else {
        panic!("exécutions attendues")
    };
    let e = &execs[0];
    assert_eq!(e.budget.get(), 12);
    assert_eq!(e.answers, vec![Answer::Sense(0)]);
    assert_eq!(
        e.calls,
        vec![Call::Sense {
            k: 0,
            answer: Ok(0)
        }]
    );
    assert_eq!(
        (e.expected.gas_used, e.expected.fault),
        (12, Some(Fault::OutOfGas))
    );
    assert_eq!(e.expected.trace, vec![1, 2]);
}

#[test]
fn sparse_memory_and_default_budget() {
    let case = load_case(&corpus_dir().join("nominal/204-stack-memory-within-a-turn")).unwrap();
    let Expectation::Runs(execs) = &case.expectation else {
        panic!("exécutions attendues")
    };
    assert_eq!(execs.len(), 2);
    assert_eq!(execs[0].budget.get(), 1000);
    assert_eq!((execs[1].mem[0], execs[1].mem[18]), (42, 9));
    assert_eq!(execs[1].mem.iter().filter(|v| **v != 0).count(), 2);
}

#[test]
fn load_refusals_have_no_expected_file() {
    let case = load_case(&corpus_dir().join("edge/001-header-bad-magic")).unwrap();
    assert_eq!(
        case.expectation,
        Expectation::Refused {
            fault: "BadHeader".to_string()
        }
    );
}

#[test]
fn versions_compare_on_major_and_minor_only() {
    assert!(comparable("1.9.0", "1.9.3"));
    assert!(!comparable("1.9.0", "1.10.0"));
    assert!(!comparable("1.9.0", "2.0.0"));
    assert!(!comparable("1.9.0", "n'importe quoi"));
}

#[test]
fn an_incomparable_case_is_refused_and_names_both_versions() {
    let source = corpus_dir().join("edge/016-gas-exact-exhaustion");
    let dir = std::env::temp_dir().join(format!("palestre-h1-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::copy(source.join("program.pbc"), dir.join("program.pbc")).unwrap();
    fs::copy(source.join("case.toml"), dir.join("case.toml")).unwrap();
    let expected = fs::read_to_string(source.join("expected.json"))
        .unwrap()
        .replace("1.9.0", "1.8.0");
    fs::write(dir.join("expected.json"), expected).unwrap();
    let err = load_case(&dir).unwrap_err();
    assert!(matches!(err, CaseError::VersionMismatch { .. }));
    let text = err.to_string();
    assert!(text.contains("1.9.0") && text.contains("1.8.0"), "{text}");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn calls_serialize_like_expected_json() {
    assert_eq!(
        Call::Sense {
            k: 6,
            answer: Ok(1)
        }
        .to_json(),
        json!({"call": "SENSE", "k": 6, "value": 1})
    );
    assert_eq!(
        Call::Sense {
            k: 200,
            answer: Err(Fault::BadSensor)
        }
        .to_json(),
        json!({"call": "SENSE", "k": 200, "fault": "BadSensor"})
    );
    assert_eq!(
        Call::Act {
            action: Action {
                kind: ActKind::Take,
                arg: 0
            },
            ok: true
        }
        .to_json(),
        json!({"call": "ACT", "kind": "TAKE", "arg": 0, "ok": true})
    );
    assert_eq!(
        Call::Rand { value: 7 }.to_json(),
        json!({"call": "RAND", "value": 7})
    );
}
