#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use pbc::{Program, decode};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::Index;

proptest! {
    #[test]
    fn load_never_panics_on_arbitrary_bytes(bytes in vec(any::<u8>(), 0..512)) {
        let _ = Program::load(&bytes);
    }

    #[test]
    fn load_reads_back_what_to_bytes_writes(
        constants in vec(any::<i64>(), 0..64),
        code in vec(any::<u8>(), 1..512),
    ) {
        let p = Program::new(constants, code).unwrap();
        prop_assert_eq!(Program::load(&p.to_bytes()).unwrap(), p);
    }

    #[test]
    fn a_corrupted_byte_never_panics(
        constants in vec(any::<i64>(), 0..8),
        code in vec(any::<u8>(), 1..64),
        index in any::<Index>(),
        byte in any::<u8>(),
    ) {
        let mut bytes = Program::new(constants, code).unwrap().to_bytes();
        let i = index.index(bytes.len());
        bytes[i] = byte;
        let _ = Program::load(&bytes);
    }

    #[test]
    fn decode_never_panics(code in vec(any::<u8>(), 0..64), pc in 0usize..80) {
        let _ = decode(&code, pc);
    }
}
