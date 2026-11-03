#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::VecDeque;

use common::{budget, program, run_default};
use pbc::{Fault, Instr as I, Op, Program};
use vm::{ActKind, ActRecord, Action, Env, Machine, STACK_SIZE, Step, run_with};

#[derive(Default)]
struct Script {
    senses: VecDeque<Result<i64, Fault>>,
    acts: VecDeque<bool>,
    rands: VecDeque<i64>,
    log: Vec<String>,
}

impl Env for Script {
    fn sense(&mut self, k: u8) -> Result<i64, Fault> {
        self.log.push(format!("SENSE {k}"));
        self.senses.pop_front().unwrap_or(Ok(0))
    }
    fn act(&mut self, action: Action) -> bool {
        self.log
            .push(format!("ACT {} {}", action.kind.name(), action.arg));
        self.acts.pop_front().unwrap_or(true)
    }
    fn rand(&mut self) -> i64 {
        self.log.push("RAND".to_string());
        self.rands.pop_front().unwrap_or(0)
    }
}

fn start(p: &Program) -> Step<'_> {
    Machine::new(p, [0; 256], budget(1000)).run()
}

#[test]
fn sense_suspends_the_machine_and_pushes_the_answer() {
    let p = program(&[], &[I::Sense(6), I::Trace, I::Halt]);
    let Step::Sense(call) = start(&p) else {
        panic!("SENSE attendu")
    };
    assert_eq!(call.k(), 6);
    let r = call.answer(Ok(41)).done().unwrap();
    assert_eq!(r.trace, vec![41]);
    assert_eq!(r.gas_used, 8 + 1);
}

#[test]
fn every_sensor_number_is_forwarded() {
    // Le serveur n'a pas de monde : c'est l'environnement qui dit BadSensor (K.3).
    let p = program(&[], &[I::Sense(200), I::Halt]);
    let Step::Sense(call) = start(&p) else {
        panic!("SENSE attendu")
    };
    assert_eq!(call.k(), 200);
    let r = call.answer(Err(Fault::BadSensor)).done().unwrap();
    assert_eq!((r.fault, r.gas_used), (Some(Fault::BadSensor), 8));
}

#[test]
fn rand_pushes_the_drawn_value() {
    let p = program(&[], &[I::Rand, I::Trace, I::Halt]);
    let Step::Rand(call) = start(&p) else {
        panic!("RAND attendu")
    };
    let r = call.answer(-5).done().unwrap();
    assert_eq!((r.trace, r.gas_used), (vec![-5], 5 + 1));
}

#[test]
fn act_pops_its_argument_and_pushes_its_outcome() {
    // Cas 060
    let p = program(&[], &[I::PushI16(0), I::Act(1), I::Trace, I::Halt]);
    for ok in [true, false] {
        let Step::Act(call) = start(&p) else {
            panic!("ACT attendu")
        };
        let action = Action {
            kind: ActKind::Take,
            arg: 0,
        };
        assert_eq!(call.action(), action);
        let r = call.answer(ok).done().unwrap();
        assert_eq!(r.trace, vec![i64::from(ok)]);
        assert_eq!(r.act, Some(ActRecord { action, ok }));
        assert_eq!(r.gas_used, 1 + 12 + 1);
    }
}

#[test]
fn action_codes_map_to_kinds_and_arguments_are_forwarded_untouched() {
    let kinds = [ActKind::Move, ActKind::Take, ActKind::Drop, ActKind::Eat];
    for (k, kind) in (0u8..).zip(kinds) {
        let p = program(&[], &[I::PushI16(-3), I::Act(k), I::Halt]);
        let Step::Act(call) = start(&p) else {
            panic!("ACT attendu")
        };
        assert_eq!(call.action(), Action { kind, arg: -3 });
    }
}

#[test]
fn unknown_action_faults_without_a_callback() {
    let r = start(&program(&[], &[I::PushI16(0), I::Act(4)]))
        .done()
        .unwrap();
    assert_eq!(r.fault, Some(Fault::BadAction));
    assert_eq!(r.act, None);
    assert_eq!(r.gas_used, 1 + 12);
}

#[test]
fn a_second_act_faults_without_a_callback() {
    // Cas 032
    let p = program(&[], &[I::PushI16(0), I::Act(1), I::PushI16(0), I::Act(1)]);
    let Step::Act(call) = start(&p) else {
        panic!("ACT attendu")
    };
    let r = call
        .answer(true)
        .done()
        .expect("le second ACT ne pose pas de rappel");
    assert_eq!(r.fault, Some(Fault::AlreadyActed));
    assert_eq!(
        r.act,
        Some(ActRecord {
            action: Action {
                kind: ActKind::Take,
                arg: 0
            },
            ok: true
        })
    );
    assert_eq!(r.gas_used, 26);
}

#[test]
fn act_on_an_empty_stack_underflows_before_anything_else() {
    for k in [1, 9] {
        let r = run_default(&[I::Act(k)]);
        assert_eq!(
            (r.fault, r.gas_used, r.act),
            (Some(Fault::StackUnderflow), 12, None),
            "ACT {k}"
        );
    }
}

#[test]
fn the_attempted_action_survives_a_later_fault() {
    let p = program(&[], &[I::PushI16(2), I::Act(0), I::Drop, I::Drop]);
    let Step::Act(call) = start(&p) else {
        panic!("ACT attendu")
    };
    let r = call.answer(false).done().unwrap();
    assert_eq!(r.fault, Some(Fault::StackUnderflow));
    assert_eq!(
        r.act,
        Some(ActRecord {
            action: Action {
                kind: ActKind::Move,
                arg: 2
            },
            ok: false
        })
    );
}

#[test]
fn callbacks_on_a_full_stack_ask_then_overflow() {
    // D-005 (à confirmer à J4) : la question est posée, puis l'empilement de la réponse échoue.
    let mut instrs = vec![I::PushI16(0); STACK_SIZE];
    instrs.push(I::Sense(1));
    let p = program(&[], &instrs);
    let Step::Sense(call) = start(&p) else {
        panic!("le rappel précède l'empilement")
    };
    let r = call.answer(Ok(5)).done().unwrap();
    assert_eq!((r.fault, r.gas_used), (Some(Fault::StackOverflow), 64 + 8));

    let mut instrs = vec![I::PushI16(0); STACK_SIZE];
    instrs.push(I::Rand);
    let p = program(&[], &instrs);
    let Step::Rand(call) = start(&p) else {
        panic!("le rappel précède l'empilement")
    };
    assert_eq!(
        call.answer(1).done().unwrap().fault,
        Some(Fault::StackOverflow)
    );
}

#[test]
fn run_with_drives_every_callback_in_order() {
    let p = program(
        &[],
        &[I::Sense(0), I::Rand, I::Add, I::Act(3), I::Trace, I::Halt],
    );
    let mut env = Script {
        senses: [Ok(10)].into(),
        rands: [5].into(),
        acts: [true].into(),
        ..Script::default()
    };
    let r = run_with(Machine::new(&p, [0; 256], budget(1000)), &mut env);
    assert_eq!(env.log, ["SENSE 0", "RAND", "ACT EAT 15"]);
    assert_eq!(r.trace, vec![1]);
    assert_eq!(r.gas_used, 8 + 5 + 2 + 12 + 1);
}

#[test]
fn every_opcode_is_implemented() {
    // Garantit qu'aucun bras « pas encore implémenté » ne subsiste depuis T04.
    for op in Op::ALL {
        let mut code = Vec::new();
        for _ in 0..3 {
            pbc::encode(I::PushI16(1), &mut code);
        }
        code.push(op.byte());
        code.resize(code.len() + op.size() - 1, 0);
        let p = Program::new(vec![7], code).unwrap();
        let r = run_with(
            Machine::new(&p, [0; 256], budget(1000)),
            &mut Script::default(),
        );
        assert_ne!(
            r.fault,
            Some(Fault::BadOpcode),
            "{} n'est pas implémenté",
            op.mnemonic()
        );
    }
}

#[test]
fn action_names_follow_k3() {
    let names = [
        (ActKind::Move, "MOVE"),
        (ActKind::Take, "TAKE"),
        (ActKind::Drop, "DROP"),
        (ActKind::Eat, "EAT"),
    ];
    for (code, (kind, name)) in (0u8..).zip(names) {
        assert_eq!(ActKind::from_code(code), Some(kind));
        assert_eq!(kind.name(), name);
        assert_eq!(ActKind::from_name(name), Some(kind));
    }
    assert_eq!(ActKind::from_code(4), None);
    assert_eq!(ActKind::from_name("move"), None);
}
