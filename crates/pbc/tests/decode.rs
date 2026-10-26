#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use pbc::{Fault, Instr, Op, Operand, decode, encode};

#[test]
fn the_table_has_43_opcodes_and_bytes_round_trip() {
    assert_eq!(Op::ALL.len(), 43);
    for op in Op::ALL {
        assert_eq!(Op::from_byte(op.byte()), Some(op));
    }
    let known = (0u8..=255).filter(|b| Op::from_byte(*b).is_some()).count();
    assert_eq!(known, 43);
}

#[test]
fn gas_table_matches_b3() {
    let expected: [(u8, u16); 43] = [
        (0x00, 0),
        (0x01, 1),
        (0x02, 1),
        (0x03, 1),
        (0x04, 1),
        (0x05, 1),
        (0x06, 1),
        (0x07, 1),
        (0x08, 2),
        (0x10, 2),
        (0x11, 2),
        (0x12, 2),
        (0x13, 7),
        (0x14, 7),
        (0x15, 2),
        (0x20, 2),
        (0x21, 2),
        (0x22, 2),
        (0x23, 2),
        (0x24, 2),
        (0x25, 2),
        (0x26, 4),
        (0x27, 3),
        (0x28, 6),
        (0x29, 2),
        (0x2D, 6),
        (0x30, 4),
        (0x31, 5),
        (0x32, 3),
        (0x33, 4),
        (0x40, 2),
        (0x41, 2),
        (0x42, 2),
        (0x43, 1),
        (0x44, 3),
        (0x50, 6),
        (0x51, 4),
        (0x52, 7),
        (0x60, 8),
        (0x61, 12),
        (0x70, 5),
        (0x71, 2),
        (0xF0, 1),
    ];
    for (byte, gas) in expected {
        let op = Op::from_byte(byte).unwrap();
        assert_eq!(op.gas(), gas, "gaz de {}", op.mnemonic());
    }
}

#[test]
fn operand_kinds_and_sizes() {
    let with_operand = [
        (0x01, Operand::U16),
        (0x02, Operand::I16),
        (0x32, Operand::U8),
        (0x33, Operand::U8),
        (0x40, Operand::I16),
        (0x41, Operand::I16),
        (0x42, Operand::I16),
        (0x50, Operand::I16),
        (0x60, Operand::U8),
        (0x61, Operand::U8),
    ];
    for op in Op::ALL {
        let expected = with_operand
            .iter()
            .find(|(byte, _)| *byte == op.byte())
            .map_or(Operand::None, |(_, kind)| *kind);
        assert_eq!(op.operand(), expected, "{}", op.mnemonic());
        let size = match expected {
            Operand::None => 1,
            Operand::U8 => 2,
            Operand::U16 | Operand::I16 => 3,
        };
        assert_eq!(op.size(), size, "{}", op.mnemonic());
    }
}

#[test]
fn mnemonics_round_trip_case_insensitively() {
    for op in Op::ALL {
        assert_eq!(Op::from_mnemonic(op.mnemonic()), Some(op));
        assert_eq!(Op::from_mnemonic(&op.mnemonic().to_lowercase()), Some(op));
    }
    assert_eq!(Op::from_mnemonic("NOPE"), None);
    assert_eq!(Op::Shr.mnemonic(), "SHR");
    assert_eq!(Op::PushI16.mnemonic(), "PUSHI16");
    assert_eq!(Op::JumpDest.mnemonic(), "JUMPDEST");
}

#[test]
fn operands_are_big_endian_and_typed() {
    assert_eq!(decode(&[0x02, 0xFF, 0xFE], 0), Ok(Instr::PushI16(-2)));
    assert_eq!(decode(&[0x01, 0x01, 0x02], 0), Ok(Instr::Const(0x0102)));
    assert_eq!(decode(&[0x60, 0x0D], 0), Ok(Instr::Sense(13)));
    assert_eq!(decode(&[0x40, 0x80, 0x00], 0), Ok(Instr::Jmp(i16::MIN)));
    assert_eq!(decode(&[0x00, 0x43], 1), Ok(Instr::JumpDest));
}

#[test]
fn decode_faults_follow_b2_order() {
    assert_eq!(decode(&[0x00], 1), Err(Fault::PcOutOfRange));
    assert_eq!(decode(&[0x00], usize::MAX), Err(Fault::PcOutOfRange));
    assert_eq!(decode(&[0x09], 0), Err(Fault::BadOpcode));
    assert_eq!(decode(&[0xFF, 0x00, 0x00], 0), Err(Fault::BadOpcode));
    assert_eq!(decode(&[0x02, 0x00], 0), Err(Fault::Truncated));
    assert_eq!(decode(&[0x60], 0), Err(Fault::Truncated));
}

#[test]
fn decoding_resumes_inside_an_operand() {
    // PUSHI16 67 : son octet de poids faible vaut 0x43, l'opcode de JUMPDEST (A.3).
    let code = [0x02, 0x00, 0x43, 0x00];
    assert_eq!(decode(&code, 0), Ok(Instr::PushI16(67)));
    assert_eq!(decode(&code, 2), Ok(Instr::JumpDest));
}

#[test]
fn encode_then_decode() {
    let samples = [
        Instr::Halt,
        Instr::Const(4095),
        Instr::PushI16(i16::MIN),
        Instr::Load(255),
        Instr::Store(0),
        Instr::Jmp(-3),
        Instr::Jz(7),
        Instr::Jnz(0),
        Instr::Call(i16::MAX),
        Instr::Sense(200),
        Instr::Act(3),
        Instr::Trace,
        Instr::Sar,
    ];
    for instr in samples {
        let mut code = Vec::new();
        encode(instr, &mut code);
        assert_eq!(code.len(), instr.size(), "{instr:?}");
        assert_eq!(code[0], instr.op().byte(), "{instr:?}");
        assert_eq!(decode(&code, 0), Ok(instr));
    }
}

#[test]
fn every_opcode_decodes_and_carries_its_gas() {
    for op in Op::ALL {
        let mut code = vec![op.byte()];
        code.resize(op.size(), 0);
        let instr = decode(&code, 0).unwrap();
        assert_eq!(instr.op(), op);
        assert_eq!(instr.gas(), op.gas());
    }
}
