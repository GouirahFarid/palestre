#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::num::NonZeroU16;

use pbc::{Fault, Op, Operand, Program};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use verify::{Limits, verify_with};
use vm::{Action, Env, Machine, run_with};

/// Fautes indépendantes des valeurs : le vérificateur doit toutes les annoncer.
const STRUCTURAL: [Fault; 10] = [
    Fault::BadOpcode,
    Fault::Truncated,
    Fault::PcOutOfRange,
    Fault::BadJump,
    Fault::StackUnderflow,
    Fault::StackOverflow,
    Fault::CallStackOverflow,
    Fault::CallStackUnderflow,
    Fault::BadAction,
    Fault::AlreadyActed,
];

struct Answers(Vec<i64>, usize);

impl Answers {
    fn next(&mut self) -> i64 {
        let v = self
            .0
            .get(self.1 % self.0.len().max(1))
            .copied()
            .unwrap_or(0);
        self.1 += 1;
        v
    }
}

impl Env for Answers {
    fn sense(&mut self, _k: u8) -> Result<i64, Fault> {
        Ok(self.next())
    }
    fn act(&mut self, _action: Action) -> bool {
        self.next() % 2 == 0
    }
    fn rand(&mut self) -> i64 {
        self.next()
    }
}

/// Une instruction valide aux opérandes petits : les sauts relatifs restent près d'eux, `ACT 4` est inconnu, et
/// `CONST 3` sort de la table de trois constantes. Les instructions qui empilent et celles du flot de contrôle sont
/// plus fréquentes : tirées uniformément, la plupart dépilent, et le tour s'arrête au premier `StackUnderflow`.
fn instr() -> impl Strategy<Value = Vec<u8>> {
    use Op::*;
    let op = prop_oneof![
        4 => select(vec![PushI16, Const, Load, Sense, Rand, Gas, Dup, Over]),
        3 => select(vec![Jmp, Jz, Jnz, JmpX, Call, CallX, Return, JumpDest]),
        1 => Just(Act),
        2 => select(Op::ALL.to_vec()),
    ];
    (op, -8i16..8).prop_map(|(op, n)| {
        let mut bytes = vec![op.byte()];
        match op.operand() {
            Operand::None => {}
            Operand::U8 => bytes.push(n.rem_euclid(5) as u8),
            Operand::U16 => bytes.extend_from_slice(&(n.rem_euclid(4) as u16).to_be_bytes()),
            Operand::I16 => bytes.extend_from_slice(&n.to_be_bytes()),
        }
        bytes
    })
}

/// Du code qui s'exécute assez loin pour que le test morde : des octets tirés uniformément sont presque tous des
/// opcodes inconnus, et le tour s'arrêterait au premier. Quelques valeurs empilées d'abord, des marqueurs fréquents,
/// un `PUSHI16 67` dont l'opérande porte `0x43` (A.3), et quelques octets quelconques pour les erreurs de décodage.
fn code() -> impl Strategy<Value = Vec<u8>> {
    let push =
        (-8i16..8).prop_map(|n| [vec![Op::PushI16.byte()], n.to_be_bytes().to_vec()].concat());
    let piece = prop_oneof![
        8 => instr(),
        2 => Just(vec![Op::JumpDest.byte()]),
        1 => Just(vec![Op::PushI16.byte(), 0x00, 0x43]),
        1 => any::<u8>().prop_map(|byte| vec![byte]),
    ];
    (vec(push, 0..6), vec(piece, 1..32))
        .prop_map(|(prefix, pieces)| [prefix.concat(), pieces.concat()].concat())
}

proptest! {
    #[test]
    fn every_structural_fault_seen_at_runtime_was_announced(
        code in code(),
        answers in vec(prop_oneof![-2i64..12, any::<i64>()], 0..16),
    ) {
        let p = Program::new(vec![0, 1, -1], code).unwrap();
        // Une limite basse : un programme tiré au hasard qui l'atteint est écarté, autant le savoir vite.
        let verdict = verify_with(&p, Limits { max_states: 50_000 });
        prop_assume!(verdict.complete);
        let report = run_with(Machine::new(&p, [0; 256], NonZeroU16::new(1000).unwrap()), &mut Answers(answers, 0));
        if let Some(fault) = report.fault {
            if STRUCTURAL.contains(&fault) {
                prop_assert!(
                    verdict.issues.iter().any(|i| i.fault == fault),
                    "{fault} survenue mais non annoncée : {verdict:?}"
                );
            }
        }
    }
}
