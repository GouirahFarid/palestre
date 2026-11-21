#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use corpus::{
    Answer, Call, Divergence, Expectation, InProcess, Reason, Replay, check_case, compare,
    load_case, load_corpus,
};
use pbc::Fault;
use vm::{ActKind, Action, Env, Report};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn report(gas: u16) -> Report {
    Report {
        mem: [0; 256],
        gas_used: gas,
        fault: None,
        act: None,
        trace: vec![],
    }
}

#[test]
fn comparison_stops_at_the_first_field_in_format_md_order() {
    let expected = report(10);
    let mut actual = report(11);
    actual.mem[200] = 1;
    assert_eq!(
        compare(&[], &expected, &[], &actual),
        Some(Divergence::GasUsed {
            expected: 10,
            actual: 11
        })
    );
    actual.gas_used = 10;
    assert_eq!(
        compare(&[], &expected, &[], &actual),
        Some(Divergence::Mem {
            index: 200,
            expected: 0,
            actual: 1
        })
    );
    let call = Call::Rand { value: 1 };
    assert_eq!(
        compare(&[call], &expected, &[], &expected),
        Some(Divergence::Calls {
            expected: vec![call],
            actual: vec![]
        })
    );
    assert_eq!(compare(&[], &expected, &[], &expected), None);
}

#[test]
fn fault_comes_before_trace() {
    let expected = report(5);
    let mut actual = report(5);
    actual.trace = vec![1];
    actual.fault = Some(Fault::BadJump);
    assert!(matches!(
        compare(&[], &expected, &[], &actual),
        Some(Divergence::Fault { .. })
    ));
    actual.fault = None;
    assert!(matches!(
        compare(&[], &expected, &[], &actual),
        Some(Divergence::Trace { .. })
    ));
}

#[test]
fn replay_answers_in_order_and_records_the_transcript() {
    let answers = [
        Answer::Sense(3),
        Answer::SenseFault(Fault::BadSensor),
        Answer::Act(false),
        Answer::Rand(-1),
    ];
    let mut replay = Replay::new(&answers);
    let action = Action {
        kind: ActKind::Move,
        arg: 1,
    };
    assert_eq!(replay.sense(4), Ok(3));
    assert_eq!(replay.sense(200), Err(Fault::BadSensor));
    assert!(!replay.act(action));
    assert_eq!(replay.rand(), -1);
    assert_eq!(
        replay.transcript(),
        &[
            Call::Sense {
                k: 4,
                answer: Ok(3)
            },
            Call::Sense {
                k: 200,
                answer: Err(Fault::BadSensor)
            },
            Call::Act { action, ok: false },
            Call::Rand { value: -1 },
        ]
    );
}

#[test]
fn unscripted_questions_get_neutral_answers_and_are_recorded() {
    let mut replay = Replay::new(&[Answer::Act(true)]);
    assert_eq!(replay.sense(1), Ok(0)); // le script attendait un ACT
    assert_eq!(replay.rand(), 0); // le script est épuisé
    assert_eq!(replay.transcript().len(), 2);
}

#[test]
fn the_public_corpus_passes_in_process() {
    let cases = load_corpus(&corpus_dir()).unwrap();
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|c| check_case(c, &mut InProcess).err())
        .map(|f| f.to_string())
        .collect();
    assert!(
        failures.is_empty(),
        "{} cas en échec :\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn a_wrong_expectation_reports_the_first_difference() {
    let mut case = load_case(&corpus_dir().join("edge/016-gas-exact-exhaustion")).unwrap();
    let Expectation::Runs(execs) = &mut case.expectation else {
        panic!("exécutions attendues")
    };
    execs[0].expected.gas_used = 11;
    let failure = check_case(&case, &mut InProcess).unwrap_err();
    assert_eq!(failure.exec, Some(0));
    assert!(matches!(
        failure.reason,
        Reason::Diverged(Divergence::GasUsed {
            expected: 11,
            actual: 12
        })
    ));
    assert!(failure.to_string().contains("016-gas-exact-exhaustion"));
}
