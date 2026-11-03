#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{program, run, run_default};
use pbc::{Fault, Instr as I};

#[test]
fn storex_then_loadx() {
    let r = run_default(&[
        I::PushI16(42),
        I::PushI16(7),
        I::StoreX,
        I::PushI16(7),
        I::LoadX,
        I::Trace,
        I::Halt,
    ]);
    assert_eq!(r.trace, vec![42]);
    assert_eq!(r.mem[7], 42);
    assert_eq!(r.gas_used, 1 + 1 + 5 + 1 + 4 + 1);
}

#[test]
fn store_and_load_by_immediate_address() {
    let r = run_default(&[
        I::PushI16(9),
        I::Store(255),
        I::Load(255),
        I::Trace,
        I::Halt,
    ]);
    assert_eq!(r.trace, vec![9]);
    assert_eq!(r.mem[255], 9);
    assert_eq!(r.gas_used, 1 + 4 + 3 + 1);
}

#[test]
fn immediate_forms_cost_one_less() {
    // Cas 062 : LOAD 3 contre LOADX 4, STORE 4 contre STOREX 5.
    assert_eq!(run_default(&[I::Load(0), I::Halt]).gas_used, 3);
    assert_eq!(
        run_default(&[I::PushI16(0), I::LoadX, I::Halt]).gas_used,
        1 + 4
    );
    assert_eq!(
        run_default(&[I::PushI16(0), I::Store(0), I::Halt]).gas_used,
        1 + 4
    );
    assert_eq!(
        run_default(&[I::PushI16(0), I::PushI16(0), I::StoreX, I::Halt]).gas_used,
        2 + 5
    );
}

#[test]
fn initial_memory_is_readable() {
    let mut mem = [0; 256];
    mem[3] = 42;
    assert_eq!(
        run(&program(&[], &[I::Load(3), I::Trace, I::Halt]), mem, 1000).trace,
        vec![42]
    );
}

#[test]
fn addresses_outside_0_255_are_out_of_bounds() {
    for addr in [256, -1, i16::MAX] {
        let r = run_default(&[I::PushI16(addr), I::LoadX]);
        assert_eq!(
            (r.fault, r.gas_used),
            (Some(Fault::OutOfBounds), 5),
            "LOADX {addr}"
        );
    }
    let r = run_default(&[I::PushI16(1), I::PushI16(256), I::StoreX]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::OutOfBounds), 7));
}

#[test]
fn missing_operands_underflow_before_bounds_are_checked() {
    let r = run_default(&[I::PushI16(300), I::StoreX]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::StackUnderflow), 6));
    // Cas 063 : STORE k sur pile vide → StackUnderflow, jamais OutOfBounds.
    let r = run_default(&[I::Store(0)]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::StackUnderflow), 4));
    let r = run_default(&[I::LoadX]);
    assert_eq!((r.fault, r.gas_used), (Some(Fault::StackUnderflow), 4));
}

#[test]
fn writes_before_a_fault_stay_in_the_report() {
    // corpus/FORMAT.md : la restauration de E.2 n'est pas le fait de la machine.
    let r = run_default(&[I::PushI16(5), I::Store(1), I::Drop]);
    assert_eq!(r.fault, Some(Fault::StackUnderflow));
    assert_eq!(r.mem[1], 5);
}
