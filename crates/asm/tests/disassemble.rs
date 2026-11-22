#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use asm::{assemble, disassemble};
use pbc::Program;
use proptest::collection::vec;
use proptest::prelude::*;

#[test]
fn disassembly_is_readable() {
    let p = Program::new(
        vec![42],
        vec![0x01, 0x00, 0x00, 0xF0, 0x40, 0xFF, 0xFC, 0xFF],
    )
    .unwrap();
    let text = disassemble(&p);
    for expected in [".const 42", "CONST 0", "TRACE", "JMP -4", ".byte 0xFF"] {
        assert!(
            text.contains(expected),
            "« {expected} » absent de :\n{text}"
        );
    }
}

#[test]
fn a_truncated_tail_becomes_raw_bytes() {
    let p = Program::new(vec![], vec![0x02, 0x00]).unwrap();
    let text = disassemble(&p);
    assert!(text.contains(".byte 0x02"), "{text}");
    assert_eq!(assemble(&text).unwrap(), p);
}

#[test]
fn every_public_corpus_program_round_trips() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut checked = 0;
    for kind in ["edge", "nominal"] {
        for entry in std::fs::read_dir(root.join(kind)).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path().join("program.pbc")).unwrap();
            if let Ok(p) = Program::load(&bytes) {
                assert_eq!(assemble(&disassemble(&p)).unwrap(), p);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 62);
}

proptest! {
    #[test]
    fn asm_of_disasm_is_identity(constants in vec(any::<i64>(), 0..20), code in vec(any::<u8>(), 1..400)) {
        let p = Program::new(constants, code).unwrap();
        prop_assert_eq!(assemble(&disassemble(&p)).unwrap(), p);
    }
}
