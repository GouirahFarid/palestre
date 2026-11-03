#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{program, run};
use pbc::{Fault, Instr as I};
use vm::Report;

/// `a` puis `b` sont empilés : `b` est au sommet (notation B.3).
fn binop(a: i64, b: i64, op: I) -> Report {
    run(
        &program(&[a, b], &[I::Const(0), I::Const(1), op, I::Trace, I::Halt]),
        [0; 256],
        1000,
    )
}

fn bin(a: i64, b: i64, op: I) -> i64 {
    let r = binop(a, b, op);
    assert_eq!(r.fault, None, "{a} {b} {op:?}");
    r.trace[0]
}

fn un(a: i64, op: I) -> i64 {
    let r = run(
        &program(&[a], &[I::Const(0), op, I::Trace, I::Halt]),
        [0; 256],
        1000,
    );
    assert_eq!(r.fault, None, "{a} {op:?}");
    r.trace[0]
}

#[test]
fn operand_order_is_a_then_b() {
    assert_eq!(bin(7, 3, I::Sub), 4);
    assert_eq!(bin(3, 7, I::Sub), -4);
    assert_eq!(bin(6, 7, I::Mul), 42);
    assert_eq!(bin(7, 2, I::Div), 3);
    assert_eq!(bin(1, 2, I::Lt), 1);
    assert_eq!(bin(2, 1, I::Lt), 0);
}

#[test]
fn arithmetic_wraps() {
    assert_eq!(bin(i64::MAX, 1, I::Add), i64::MIN);
    assert_eq!(bin(i64::MIN, 1, I::Sub), i64::MAX);
    assert_eq!(bin(i64::MAX, 2, I::Mul), -2);
    assert_eq!(un(i64::MIN, I::Neg), i64::MIN);
    assert_eq!(un(5, I::Neg), -5);
}

#[test]
fn division_truncates_toward_zero_like_rust() {
    assert_eq!(bin(-7, 2, I::Div), -3);
    assert_eq!(bin(7, -2, I::Div), -3);
    assert_eq!(bin(-7, 2, I::Mod), -1);
    assert_eq!(bin(7, -2, I::Mod), 1);
    assert_eq!(bin(i64::MIN, -1, I::Div), i64::MIN);
    assert_eq!(bin(i64::MIN, -1, I::Mod), 0);
}

#[test]
fn division_by_zero_is_a_paid_fault() {
    for op in [I::Div, I::Mod] {
        let r = binop(1, 0, op);
        assert_eq!(r.fault, Some(Fault::DivByZero));
        assert_eq!(r.gas_used, 1 + 1 + 7);
        assert!(r.trace.is_empty());
    }
}

#[test]
fn comparisons_are_signed_and_yield_0_or_1() {
    assert_eq!(bin(-1, 0, I::Lt), 1);
    assert_eq!(bin(2, 1, I::Gt), 1);
    assert_eq!(bin(i64::MIN, i64::MAX, I::Gt), 0);
    assert_eq!(bin(5, 5, I::Eq), 1);
    assert_eq!(bin(5, 6, I::Eq), 0);
}

#[test]
fn not_is_logical_and_bnot_is_bitwise() {
    assert_eq!(un(0, I::Not), 1);
    assert_eq!(un(7, I::Not), 0);
    assert_eq!(un(-1, I::Not), 0);
    assert_eq!(un(0, I::BNot), -1);
    assert_eq!(un(5, I::BNot), -6);
}

#[test]
fn bitwise_ops_act_on_twos_complement() {
    assert_eq!(bin(0b1100, 0b1010, I::And), 0b1000);
    assert_eq!(bin(0b1100, 0b1010, I::Or), 0b1110);
    assert_eq!(bin(0b1100, 0b1010, I::Xor), 0b0110);
    assert_eq!(bin(-1, 0x0F, I::And), 0x0F);
}

#[test]
fn shl_and_shr_are_logical() {
    assert_eq!(bin(1, 3, I::Shl), 8);
    assert_eq!(bin(1, 63, I::Shl), i64::MIN);
    assert_eq!(bin(-1, 1, I::Shr), 9_223_372_036_854_775_807);
    assert_eq!(bin(i64::MIN, 63, I::Shr), 1);
    assert_eq!(bin(42, 0, I::Shr), 42);
}

#[test]
fn sar_propagates_the_sign_and_rounds_down() {
    assert_eq!(bin(-1, 1, I::Sar), -1);
    assert_eq!(bin(-7, 1, I::Sar), -4);
    assert_eq!(bin(-7, 2, I::Div), -3);
    assert_eq!(bin(-1, 63, I::Sar), -1);
    assert_eq!(bin(64, 3, I::Sar), 8);
}

#[test]
fn shift_counts_outside_0_63_never_fault() {
    assert_eq!(bin(1, 64, I::Shl), 0);
    assert_eq!(bin(-1, 64, I::Shr), 0);
    assert_eq!(bin(-1, -1, I::Shl), 0);
    assert_eq!(bin(-5, 64, I::Sar), -1);
    assert_eq!(bin(5, 64, I::Sar), 0);
    assert_eq!(bin(5, -3, I::Sar), 0);
    assert_eq!(bin(-5, i64::MIN, I::Sar), -1);
}

#[test]
fn each_instruction_costs_what_b3_says() {
    let binary = [
        (I::Add, 2),
        (I::Sub, 2),
        (I::Mul, 2),
        (I::Div, 7),
        (I::Mod, 7),
        (I::Eq, 2),
        (I::Lt, 2),
        (I::Gt, 2),
        (I::And, 2),
        (I::Or, 2),
        (I::Xor, 4),
        (I::Shl, 3),
        (I::Shr, 6),
        (I::Sar, 6),
    ];
    for (op, cost) in binary {
        assert_eq!(binop(7, 3, op).gas_used, 3 + cost, "{op:?}");
    }
    for op in [I::Neg, I::Not, I::BNot] {
        let r = run(
            &program(&[7], &[I::Const(0), op, I::Trace, I::Halt]),
            [0; 256],
            1000,
        );
        assert_eq!(r.gas_used, 2 + 2, "{op:?}");
    }
}

#[test]
fn missing_operands_underflow() {
    let r = run(&program(&[1], &[I::Const(0), I::Add]), [0; 256], 1000);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::StackUnderflow), 3));
    let r = run(&program(&[], &[I::Neg]), [0; 256], 1000);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::StackUnderflow), 2));
}
