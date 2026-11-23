#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use pbc::{Fault, Instr as I, Op, Program};
use verify::{Issue, Limits, Verdict, verify, verify_with};

fn program(constants: &[i64], instrs: &[I]) -> Program {
    let mut code = Vec::new();
    for instr in instrs {
        pbc::encode(*instr, &mut code);
    }
    Program::new(constants.to_vec(), code).unwrap()
}

fn v(instrs: &[I]) -> Verdict {
    verify(&program(&[], instrs))
}

fn issues(verdict: &Verdict) -> Vec<(usize, Fault)> {
    verdict.issues.iter().map(|i| (i.pc, i.fault)).collect()
}

#[test]
fn stack_effects_cover_every_opcode() {
    assert_eq!(Op::Over.stack_effect(), (2, 3));
    assert_eq!(Op::Act.stack_effect(), (1, 1));
    assert_eq!(Op::CallX.stack_effect(), (1, 0));
    assert!(Op::ALL.iter().all(|op| op.stack_effect().0 <= 3));
}

#[test]
fn a_straight_line_program_is_safe() {
    let verdict = v(&[I::PushI16(1), I::PushI16(2), I::Add, I::Trace, I::Halt]);
    assert!(verdict.is_safe());
    assert_eq!(verdict.max_stack, 2);
}

#[test]
fn a_static_jump_without_marker_is_reported() {
    assert_eq!(issues(&v(&[I::Jmp(0), I::Halt])), vec![(0, Fault::BadJump)]);
}

#[test]
fn a_marker_inside_an_operand_is_a_valid_target() {
    assert!(v(&[I::Jmp(2), I::PushI16(0x43), I::Halt]).is_safe());
}

#[test]
fn an_underflow_on_one_path_only() {
    // 0 SENSE 0 | 2 DUP | 3 JZ +1 (→ 7) | 6 DROP | 7 JUMPDEST | 8 DROP | 9 HALT
    let verdict = v(&[
        I::Sense(0),
        I::Dup,
        I::Jz(1),
        I::Drop,
        I::JumpDest,
        I::Drop,
        I::Halt,
    ]);
    assert_eq!(issues(&verdict), vec![(8, Fault::StackUnderflow)]);
    assert_eq!(verdict.max_stack, 2);
}

#[test]
fn a_loop_that_pushes_overflows() {
    // 0 JUMPDEST | 1 PUSHI16 1 | 4 JMP -7 (→ 0)
    let verdict = v(&[I::JumpDest, I::PushI16(1), I::Jmp(-7)]);
    assert_eq!(issues(&verdict), vec![(1, Fault::StackOverflow)]);
    assert_eq!(verdict.max_stack, 64);
}

#[test]
fn a_balanced_loop_is_bounded() {
    let verdict = v(&[
        I::PushI16(3),
        I::JumpDest,
        I::Dup,
        I::Trace,
        I::PushI16(1),
        I::Sub,
        I::Dup,
        I::Jnz(-11),
        I::Halt,
    ]);
    assert!(verdict.is_safe());
    assert_eq!(verdict.max_stack, 2);
}

#[test]
fn calls_are_followed_and_returns_resolved() {
    let verdict = v(&[
        I::Call(2),
        I::Trace,
        I::Halt,
        I::JumpDest,
        I::PushI16(9),
        I::Return,
    ]);
    assert!(verdict.is_safe());
    assert_eq!((verdict.max_stack, verdict.max_frames), (1, 1));
}

#[test]
fn a_fault_right_after_a_return_is_reported() {
    // 0 CALL +2 (→ 5) | 3 DROP | 4 HALT | 5 JUMPDEST | 6 RETURN (→ 3, pile vide)
    let verdict = v(&[I::Call(2), I::Drop, I::Halt, I::JumpDest, I::Return]);
    assert_eq!(issues(&verdict), vec![(3, Fault::StackUnderflow)]);
}

#[test]
fn a_dynamic_jump_reaches_a_marker_inside_an_operand() {
    // 0 SENSE 0 | 2 JMPX | 3 HALT | 4 HALT | 5 PUSHI16 0x43 (marqueur en 7) | 8 DROP | 9 HALT
    // Chercher les marqueurs aux seules frontières d'instructions, comme l'EVM, manquerait le DROP en 8.
    let verdict = v(&[
        I::Sense(0),
        I::JmpX,
        I::Halt,
        I::Halt,
        I::PushI16(0x43),
        I::Drop,
        I::Halt,
    ]);
    assert_eq!(
        issues(&verdict),
        vec![(2, Fault::BadJump), (8, Fault::StackUnderflow)]
    );
}

#[test]
fn unbounded_recursion_overflows_the_call_stack() {
    let verdict = v(&[I::JumpDest, I::Call(-4)]);
    assert_eq!(issues(&verdict), vec![(1, Fault::CallStackOverflow)]);
    assert_eq!(verdict.max_frames, 16);
}

#[test]
fn a_return_without_a_call_is_reported() {
    assert_eq!(
        issues(&v(&[I::Return])),
        vec![(0, Fault::CallStackUnderflow)]
    );
}

#[test]
fn decoding_problems_on_reachable_paths() {
    let p = Program::new(vec![], vec![0x02, 0x00, 0x01, 0xFF]).unwrap();
    assert_eq!(issues(&verify(&p)), vec![(3, Fault::BadOpcode)]);
    let p = Program::new(vec![], vec![0x02, 0x00]).unwrap();
    assert_eq!(issues(&verify(&p)), vec![(0, Fault::Truncated)]);
    assert_eq!(issues(&v(&[I::PushI16(1)])), vec![(3, Fault::PcOutOfRange)]);
}

#[test]
fn unreachable_garbage_is_ignored() {
    assert!(verify(&Program::new(vec![], vec![0x00, 0xFF]).unwrap()).is_safe());
}

#[test]
fn static_operands_are_checked() {
    assert_eq!(
        issues(&v(&[I::Const(0), I::Halt])),
        vec![(0, Fault::BadConst)]
    );
    assert_eq!(
        issues(&v(&[I::PushI16(0), I::Act(4), I::Halt])),
        vec![(3, Fault::BadAction)]
    );
    let twice = [I::PushI16(0), I::Act(1), I::PushI16(0), I::Act(1), I::Halt];
    assert_eq!(issues(&v(&twice)), vec![(8, Fault::AlreadyActed)]);
}

#[test]
fn a_dynamic_jump_may_reach_every_marker() {
    // 0 SENSE 0 | 2 JMPX | 3 HALT | 4 JUMPDEST | 5 HALT | 6 JUMPDEST | 7 PUSHI16 1 | 10 HALT
    let verdict = v(&[
        I::Sense(0),
        I::JmpX,
        I::Halt,
        I::JumpDest,
        I::Halt,
        I::JumpDest,
        I::PushI16(1),
        I::Halt,
    ]);
    assert_eq!(issues(&verdict), vec![(2, Fault::BadJump)]);
    assert_eq!(verdict.max_stack, 1);
}

#[test]
fn the_state_limit_keeps_the_analysis_bounded() {
    let p = program(&[], &[I::JumpDest, I::PushI16(1), I::Jmp(-7)]);
    let verdict = verify_with(&p, Limits { max_states: 10 });
    assert!(!verdict.complete);
    assert!(!verdict.is_safe());
}

#[test]
fn issues_are_sorted_and_unique() {
    let verdict = v(&[
        I::Sense(0),
        I::Dup,
        I::Jz(1),
        I::Drop,
        I::JumpDest,
        I::Drop,
        I::Halt,
    ]);
    let mut sorted = verdict.issues.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted, verdict.issues);
    assert!(verdict.issues.iter().all(|i: &Issue| i.pc < 10));
}
