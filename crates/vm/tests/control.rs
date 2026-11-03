#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::run_default;
use pbc::{Fault, Instr as I};

#[test]
fn jump_base_is_the_next_instruction() {
    // 0 JMP +4 | 3 PUSHI16 1 | 6 TRACE | 7 JUMPDEST | 8 PUSHI16 2 | 11 TRACE | 12 HALT ; cible 0+3+4 = 7
    let r = run_default(&[
        I::Jmp(4),
        I::PushI16(1),
        I::Trace,
        I::JumpDest,
        I::PushI16(2),
        I::Trace,
        I::Halt,
    ]);
    assert_eq!(r.trace, vec![2]);
    assert_eq!(r.gas_used, 2 + 1 + 1 + 1);
}

#[test]
fn a_target_must_carry_the_marker() {
    // 0 JMP +0 → cible 3 = HALT (0x00), pas 0x43
    let r = run_default(&[I::Jmp(0), I::Halt]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::BadJump), 2));
}

#[test]
fn a_target_must_be_inside_the_code() {
    assert_eq!(run_default(&[I::Jmp(100)]).fault, Some(Fault::BadJump));
    assert_eq!(run_default(&[I::Jmp(-4)]).fault, Some(Fault::BadJump)); // cible -1
}

#[test]
fn a_counted_loop() {
    // 0 PUSHI16 3 | 3 JUMPDEST | 4 DUP | 5 TRACE | 6 PUSHI16 1 | 9 SUB | 10 DUP | 11 JNZ -11 | 14 HALT
    let r = run_default(&[
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
    assert_eq!(r.trace, vec![3, 2, 1]);
    assert_eq!(r.gas_used, 1 + 3 * 9);
}

#[test]
fn conditional_jumps_check_the_target_only_when_taken() {
    assert_eq!(
        run_default(&[I::PushI16(1), I::Jz(100), I::Halt]).fault,
        None
    );
    assert_eq!(
        run_default(&[I::PushI16(0), I::Jz(100)]).fault,
        Some(Fault::BadJump)
    );
    assert_eq!(
        run_default(&[I::PushI16(0), I::Jnz(100), I::Halt]).fault,
        None
    );
    assert_eq!(
        run_default(&[I::PushI16(1), I::Jnz(100)]).fault,
        Some(Fault::BadJump)
    );
    assert_eq!(run_default(&[I::Jz(0)]).fault, Some(Fault::StackUnderflow));
}

#[test]
fn a_marker_inside_an_operand_is_a_valid_target() {
    // Cas 006 : 0 JMP +2 | 3 PUSHI16 0x43 (octets 02 00 43) | 6 HALT ; cible 5 = 0x43
    let r = run_default(&[I::Jmp(2), I::PushI16(0x43), I::Halt]);
    assert_eq!((r.fault, r.gas_used), (None, 3));
}

#[test]
fn jumping_onto_the_last_byte_then_running_off_the_end() {
    // Cas 021 : la cible est le dernier octet ; après JUMPDEST, pc = code_len.
    let r = run_default(&[I::Jmp(0), I::JumpDest]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::PcOutOfRange), 3));
}

#[test]
fn jmpx_target_is_absolute() {
    // 0 PUSHI16 5 | 3 JMPX | 4 HALT | 5 JUMPDEST | 6 PUSHI16 7 | 9 TRACE | 10 HALT
    let r = run_default(&[
        I::PushI16(5),
        I::JmpX,
        I::Halt,
        I::JumpDest,
        I::PushI16(7),
        I::Trace,
        I::Halt,
    ]);
    assert_eq!(r.trace, vec![7]);
    assert_eq!(r.gas_used, 1 + 3 + 1 + 1 + 1);
}

#[test]
fn jmpx_refusals_are_plain_badjumps() {
    let r = run_default(&[I::PushI16(-1), I::JmpX]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::BadJump), 4));
    assert_eq!(
        run_default(&[I::PushI16(4), I::JmpX]).fault,
        Some(Fault::BadJump)
    ); // = code_len
    assert_eq!(
        run_default(&[I::PushI16(0), I::JmpX]).fault,
        Some(Fault::BadJump)
    ); // 0x02
    let r = run_default(&[I::JmpX]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::StackUnderflow), 3));
}

#[test]
fn call_then_return_lands_after_the_call() {
    // 0 CALL +2 | 3 TRACE | 4 HALT | 5 JUMPDEST | 6 PUSHI16 9 | 9 RETURN
    // RETURN revient en 3, un octet ordinaire (pas de marqueur exigé, B.4).
    let r = run_default(&[
        I::Call(2),
        I::Trace,
        I::Halt,
        I::JumpDest,
        I::PushI16(9),
        I::Return,
    ]);
    assert_eq!(r.trace, vec![9]);
    assert_eq!(r.gas_used, 6 + 1 + 1 + 4 + 1);
}

#[test]
fn return_without_a_frame_underflows() {
    let r = run_default(&[I::Return]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::CallStackUnderflow), 4));
}

#[test]
fn the_seventeenth_frame_overflows() {
    // Cas 023 : 0 JUMPDEST | 1 CALL -4 → cible 0, récursion sans fin
    let r = run_default(&[I::JumpDest, I::Call(-4)]);
    assert_eq!(r.fault, Some(Fault::CallStackOverflow));
    assert_eq!(r.gas_used, 17 * (1 + 6));
}

#[test]
fn call_pushes_its_frame_before_checking_the_target() {
    // D-005 (à confirmer à J4) : seize cadres empilés, puis CALL vers une cible invalide.
    //  0 JUMPDEST | 1 LOAD 0 | 3 PUSHI16 1 | 6 ADD | 7 DUP | 8 STORE 0 | 10 PUSHI16 17 | 13 EQ
    // 14 JNZ +3 (→ 20) | 17 CALL -20 (→ 0) | 20 JUMPDEST | 21 CALL +100
    let r = run_default(&[
        I::JumpDest,
        I::Load(0),
        I::PushI16(1),
        I::Add,
        I::Dup,
        I::Store(0),
        I::PushI16(17),
        I::Eq,
        I::Jnz(3),
        I::Call(-20),
        I::JumpDest,
        I::Call(100),
    ]);
    assert_eq!(r.fault, Some(Fault::CallStackOverflow));
    assert_eq!(r.mem[0], 17);
}

#[test]
fn callx_pops_an_absolute_target_and_returns_like_call() {
    // 0 PUSHI16 6 | 3 CALLX | 4 TRACE | 5 HALT | 6 JUMPDEST | 7 PUSHI16 11 | 10 RETURN
    let r = run_default(&[
        I::PushI16(6),
        I::CallX,
        I::Trace,
        I::Halt,
        I::JumpDest,
        I::PushI16(11),
        I::Return,
    ]);
    assert_eq!(r.trace, vec![11]);
    assert_eq!(r.gas_used, 1 + 7 + 1 + 1 + 4 + 1);
}

#[test]
fn callx_on_an_empty_stack_underflows_first() {
    // Cas 054b
    let r = run_default(&[I::CallX]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::StackUnderflow), 7));
}

#[test]
fn jumpdest_alone_costs_one() {
    assert_eq!(run_default(&[I::JumpDest, I::Halt]).gas_used, 1);
}
