#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::num::NonZeroU16;

use pbc::{Fault, Op, Program};
use proptest::collection::vec;
use proptest::prelude::*;
use vm::{Action, Env, Machine, Memory, Report, run_with};

/// Répond en parcourant une liste de valeurs, et journalise chaque question.
struct Cycle {
    values: Vec<i64>,
    next: usize,
    log: Vec<(u8, i64)>,
}

impl Cycle {
    fn new(values: Vec<i64>) -> Cycle {
        Cycle {
            values,
            next: 0,
            log: Vec::new(),
        }
    }

    fn draw(&mut self, tag: u8) -> i64 {
        let v = if self.values.is_empty() {
            0
        } else {
            self.values[self.next % self.values.len()]
        };
        self.next += 1;
        self.log.push((tag, v));
        v
    }
}

impl Env for Cycle {
    fn sense(&mut self, k: u8) -> Result<i64, Fault> {
        let v = self.draw(k);
        if v % 7 == 0 {
            Err(Fault::BadSensor)
        } else {
            Ok(v)
        }
    }

    fn act(&mut self, action: Action) -> bool {
        self.draw(200 + action.kind as u8) % 2 == 0
    }

    fn rand(&mut self) -> i64 {
        self.draw(255)
    }
}

/// L'octet de `GAS` (B.7).
const GAS: u8 = 0x71;

/// Moitié octets bruts, moitié instructions valides.
///
/// Des octets bruts fautent presque tous dès les premiers octets, sur un opcode inconnu (43 opcodes sur 256 valeurs)
/// ou une pile vide : ils éprouvent le décodage, pas la sémantique. Les instructions valides sont tirées pour moitié
/// parmi les 43 opcodes et pour moitié en `PUSHI16`, pour que la pile ait de quoi nourrir les suivantes.
fn code(max: usize) -> impl Strategy<Value = Vec<u8>> {
    let op = prop_oneof![
        Just(Op::PushI16),
        proptest::sample::select(Op::ALL.to_vec())
    ];
    let instructions = vec((op, any::<[u8; 2]>()), 1..max).prop_map(|instrs| {
        let mut code = Vec::new();
        for (op, operand) in instrs {
            code.push(op.byte());
            code.extend_from_slice(&operand[..op.size() - 1]);
        }
        code
    });
    prop_oneof![vec(any::<u8>(), 1..max), instructions]
}

fn memory() -> impl Strategy<Value = Memory> {
    vec(any::<i64>(), 256).prop_map(|cells| <Memory>::try_from(cells.as_slice()).unwrap())
}

fn execute(p: &Program, mem: Memory, budget: u16, answers: Vec<i64>) -> (Report, Vec<(u8, i64)>) {
    let mut env = Cycle::new(answers);
    let report = run_with(
        Machine::new(p, mem, NonZeroU16::new(budget).unwrap()),
        &mut env,
    );
    (report, env.log)
}

proptest! {
    #[test]
    fn any_program_terminates_within_its_budget(
        constants in vec(any::<i64>(), 0..8),
        code in code(256),
        mem in memory(),
        budget in 1u16..=2000,
        answers in vec(any::<i64>(), 0..16),
    ) {
        let p = Program::new(constants, code).unwrap();
        let (report, calls) = execute(&p, mem, budget, answers);
        prop_assert!(report.gas_used <= budget);
        prop_assert!(report.trace.len() <= usize::from(budget));
        prop_assert!(calls.len() <= usize::from(budget) / 5);
    }

    #[test]
    fn execution_is_deterministic(
        constants in vec(any::<i64>(), 0..8),
        code in code(256),
        mem in memory(),
        budget in 1u16..=2000,
        answers in vec(any::<i64>(), 0..16),
    ) {
        let p = Program::new(constants, code).unwrap();
        let first = execute(&p, mem, budget, answers.clone());
        let second = execute(&p, mem, budget, answers);
        prop_assert_eq!(first, second);
    }

    #[test]
    fn a_larger_budget_never_changes_a_run_that_did_not_run_out(
        code in code(128).prop_filter("sans GAS", |code| !code.contains(&GAS)),
        answers in vec(any::<i64>(), 0..8),
    ) {
        // Invariant 6 : le gaz dépend des seules instructions exécutées.
        // GAS lit le budget restant : un programme qui peut l'exécuter dépend légitimement du budget (B.7). Un tel
        // programme est filtré par la stratégie, et non rejeté par `prop_assume!` : il y en a assez pour épuiser la
        // limite de rejets globale de proptest à 5000 cas.
        let p = Program::new(vec![1, -1], code).unwrap();
        let (small, small_calls) = execute(&p, [0; 256], 500, answers.clone());
        prop_assume!(small.fault != Some(Fault::OutOfGas));
        let (large, large_calls) = execute(&p, [0; 256], 5000, answers);
        prop_assert_eq!((small, small_calls), (large, large_calls));
    }
}
