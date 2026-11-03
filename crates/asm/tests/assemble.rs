#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use asm::{AsmError, AsmErrorKind, assemble};
use pbc::LoadError;

#[test]
fn instructions_constants_and_comments() {
    let p = assemble(".const 1234567890123\n.const -5\n  CONST 1\n  pushi16 -2 ; commentaire\n  ADD\n  TRACE\n  HALT\n")
        .unwrap();
    assert_eq!(p.constants(), &[1_234_567_890_123, -5]);
    assert_eq!(
        p.code(),
        &[0x01, 0x00, 0x01, 0x02, 0xFF, 0xFE, 0x10, 0xF0, 0x00]
    );
}

#[test]
fn a_forward_label_is_relative_to_the_next_instruction() {
    // JMP en 0 (3 octets), PUSHI16 en 3..6, fin = 6 → d = 6 - 3 = 3
    let p = assemble("    JMP fin\n    PUSHI16 1\nfin:\n    JUMPDEST\n    HALT\n").unwrap();
    assert_eq!(p.code(), &[0x40, 0x00, 0x03, 0x02, 0x00, 0x01, 0x43, 0x00]);
}

#[test]
fn a_backward_label() {
    // boucle = 0, JMP en 1 → d = 0 - 4 = -4
    let p = assemble("boucle:\n JUMPDEST\n JMP boucle\n").unwrap();
    assert_eq!(p.code(), &[0x43, 0x40, 0xFF, 0xFC]);
}

#[test]
fn an_absolute_label_for_jmpx() {
    let p = assemble(" PUSHI16 @cible\n JMPX\ncible:\n JUMPDEST\n HALT\n").unwrap();
    assert_eq!(p.code(), &[0x02, 0x00, 0x04, 0x44, 0x43, 0x00]);
}

#[test]
fn raw_bytes_hex_and_raw_offsets() {
    let p = assemble(" .byte 0x43 255 0\n PUSHI16 0x7FFF\n JZ +4\n").unwrap();
    assert_eq!(
        p.code(),
        &[0x43, 0xFF, 0x00, 0x02, 0x7F, 0xFF, 0x41, 0x00, 0x04]
    );
}

#[test]
fn errors_carry_line_numbers() {
    assert_eq!(
        assemble("HALT\nFLY\n").unwrap_err(),
        AsmError {
            line: 2,
            kind: AsmErrorKind::UnknownMnemonic("FLY".to_string())
        }
    );
    assert_eq!(
        assemble("a:\na:\nHALT\n").unwrap_err(),
        AsmError {
            line: 2,
            kind: AsmErrorKind::DuplicateLabel("a".to_string())
        }
    );
    assert_eq!(
        assemble("PUSHI16 40000\n").unwrap_err().kind,
        AsmErrorKind::OutOfRange {
            value: 40_000,
            min: -32_768,
            max: 32_767
        }
    );
    assert_eq!(
        assemble("LOAD 256\n").unwrap_err().kind,
        AsmErrorKind::OutOfRange {
            value: 256,
            min: 0,
            max: 255
        }
    );
    assert_eq!(
        assemble("JMP nulle_part\n").unwrap_err().kind,
        AsmErrorKind::UnknownLabel("nulle_part".to_string())
    );
    assert_eq!(
        assemble("ADD 3\n").unwrap_err().kind,
        AsmErrorKind::UnexpectedOperand
    );
    assert_eq!(
        assemble("LOAD\n").unwrap_err().kind,
        AsmErrorKind::MissingOperand("LOAD")
    );
    assert_eq!(
        assemble("PUSHI16 douze\n").unwrap_err().kind,
        AsmErrorKind::UnknownLabel("douze".to_string())
    );
    assert_eq!(
        assemble("PUSHI16 1x\n").unwrap_err().kind,
        AsmErrorKind::BadNumber("1x".to_string())
    );
    assert_eq!(
        assemble(".data 1\n").unwrap_err().kind,
        AsmErrorKind::BadDirective(".data".to_string())
    );
    assert_eq!(
        assemble("; vide\n").unwrap_err().kind,
        AsmErrorKind::Program(LoadError::BadCodeLen(0))
    );
}

#[test]
fn a_jump_too_far_for_an_i16_is_refused() {
    let mut src = String::from("debut:\nJUMPDEST\n");
    src.push_str(&".byte 0\n".repeat(40_000));
    src.push_str("JMP debut\n");
    assert!(matches!(
        assemble(&src).unwrap_err().kind,
        AsmErrorKind::OutOfRange { .. }
    ));
}
