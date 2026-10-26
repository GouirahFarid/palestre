#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use pbc::{LoadError, Program};

/// Construit un fichier `.pbc` conforme à A.1.
fn pbc(constants: &[i64], code: &[u8]) -> Vec<u8> {
    let mut bytes = b"PBC1".to_vec();
    bytes.push(0x01);
    bytes.push(0x00);
    bytes.extend_from_slice(&(constants.len() as u16).to_be_bytes());
    for c in constants {
        bytes.extend_from_slice(&c.to_be_bytes());
    }
    bytes.extend_from_slice(&(code.len() as u32).to_be_bytes());
    bytes.extend_from_slice(code);
    bytes
}

fn corpus_program(case: &str) -> Vec<u8> {
    let path = format!(
        "{}/../../corpus/edge/{case}/program.pbc",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).unwrap()
}

#[test]
fn loads_the_smallest_program() {
    let p = Program::load(&pbc(&[], &[0x00])).unwrap();
    assert!(p.constants().is_empty());
    assert_eq!(p.code(), &[0x00]);
}

#[test]
fn constants_are_big_endian_i64() {
    let bytes = pbc(&[-1, 0x0102_0304_0506_0708], &[0x00]);
    assert_eq!(&bytes[8..16], &[0xFF; 8]);
    assert_eq!(&bytes[16..24], &[1, 2, 3, 4, 5, 6, 7, 8]);
    let p = Program::load(&bytes).unwrap();
    assert_eq!(p.constants(), &[-1, 0x0102_0304_0506_0708]);
}

#[test]
fn header_fields_are_checked() {
    let good = pbc(&[], &[0x00]);
    let mut bad_magic = good.clone();
    bad_magic[3] = b'2';
    assert_eq!(Program::load(&bad_magic), Err(LoadError::BadMagic));
    let mut bad_version = good.clone();
    bad_version[4] = 2;
    assert_eq!(Program::load(&bad_version), Err(LoadError::BadVersion(2)));
    let mut bad_flags = good;
    bad_flags[5] = 0x80;
    assert_eq!(Program::load(&bad_flags), Err(LoadError::BadFlags(0x80)));
}

#[test]
fn constant_count_is_bounded_before_reading_the_table() {
    // 4096 constantes annoncées, aucune fournie : la borne est vérifiée avant toute lecture.
    let bytes = [b'P', b'B', b'C', b'1', 1, 0, 0x10, 0x00];
    assert_eq!(
        Program::load(&bytes),
        Err(LoadError::TooManyConstants(4096))
    );
    let p = Program::load(&pbc(&[7; 4095], &[0x00])).unwrap();
    assert_eq!(p.constants().len(), 4095);
}

#[test]
fn code_len_must_be_in_1_to_65535() {
    let mut header = pbc(&[], &[]);
    assert_eq!(Program::load(&header), Err(LoadError::BadCodeLen(0)));
    header.truncate(8);
    header.extend_from_slice(&65_536u32.to_be_bytes());
    assert_eq!(Program::load(&header), Err(LoadError::BadCodeLen(65_536)));
    let mut huge = b"PBC1\x01\x00\x00\x00".to_vec();
    huge.extend_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        Program::load(&huge),
        Err(LoadError::BadCodeLen(4_294_967_295))
    );
    assert!(Program::load(&pbc(&[], &[0x00; 65_535])).is_ok());
}

#[test]
fn file_size_must_match_exactly() {
    let good = pbc(&[42], &[0x02, 0x00, 0x07, 0xF0, 0x00]);
    let mut trailing = good.clone();
    trailing.push(0x00);
    assert_eq!(
        Program::load(&trailing),
        Err(LoadError::SizeMismatch {
            expected: good.len(),
            actual: trailing.len()
        })
    );
    for len in 0..good.len() {
        let err = Program::load(&good[..len]).unwrap_err();
        assert_eq!(err.fault_name(), "BadHeader", "préfixe de {len} octets");
    }
}

#[test]
fn every_load_error_is_named_badheader() {
    let all = [
        LoadError::TooShort,
        LoadError::BadMagic,
        LoadError::BadVersion(0),
        LoadError::BadFlags(1),
        LoadError::TooManyConstants(4096),
        LoadError::BadCodeLen(0),
        LoadError::SizeMismatch {
            expected: 1,
            actual: 2,
        },
    ];
    for err in all {
        assert_eq!(err.fault_name(), "BadHeader");
        assert!(!err.to_string().is_empty());
    }
}

#[test]
fn public_corpus_header_cases_are_refused() {
    for case in [
        "001-header-bad-magic",
        "002-header-nonzero-flags",
        "003-header-trailing-byte",
    ] {
        assert!(Program::load(&corpus_program(case)).is_err(), "{case}");
    }
    assert!(Program::load(&corpus_program("040-degenerate-empty-constant-table")).is_ok());
}

#[test]
fn to_bytes_is_the_inverse_of_load() {
    let bytes = pbc(&[i64::MIN, 0, i64::MAX], &[0x01, 0x00, 0x02, 0xF0, 0x00]);
    let p = Program::load(&bytes).unwrap();
    assert_eq!(p.to_bytes(), bytes);
    assert_eq!(Program::load(&p.to_bytes()).unwrap(), p);
}

#[test]
fn new_applies_the_same_bounds() {
    assert_eq!(Program::new(vec![], vec![]), Err(LoadError::BadCodeLen(0)));
    assert_eq!(
        Program::new(vec![0; 4096], vec![0x00]),
        Err(LoadError::TooManyConstants(4096))
    );
    assert_eq!(
        Program::new(vec![], vec![0; 65_536]),
        Err(LoadError::BadCodeLen(65_536))
    );
    assert!(Program::new(vec![1, 2], vec![0x00]).is_ok());
}
