#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Cursor;

use exec::frame::{
    CLIENT_MAX_LEN, Frame, FrameError, FrameType, SERVER_MAX_LEN, read_frame, write_frame,
};

#[test]
fn bounds_match_k2() {
    assert_eq!(CLIENT_MAX_LEN, 8 + 8 * 4095 + 4 + 65_535);
    assert_eq!(SERVER_MAX_LEN, 4_194_304);
}

#[test]
fn frames_are_type_then_big_endian_length() {
    let mut buf = Vec::new();
    write_frame(&mut buf, FrameType::Open, b"{}").unwrap();
    assert_eq!(buf, [0x11, 0, 0, 0, 2, b'{', b'}']);
    let frame = read_frame(&mut Cursor::new(buf), CLIENT_MAX_LEN).unwrap();
    assert_eq!(
        frame,
        Frame {
            kind: FrameType::Open,
            payload: b"{}".to_vec()
        }
    );
    let mut buf = Vec::new();
    write_frame(&mut buf, FrameType::Submit, &[0; 300]).unwrap();
    assert_eq!(&buf[..5], &[0x13, 0, 0, 0x01, 0x2C]);
}

#[test]
fn every_type_byte_maps_both_ways() {
    for byte in 0x11..=0x1D {
        assert_eq!(FrameType::from_byte(byte).unwrap().byte(), byte);
    }
    for byte in [0x00, 0x10, 0x1E, 0xFF] {
        assert_eq!(FrameType::from_byte(byte), None);
    }
}

#[test]
fn an_unknown_type_is_rejected() {
    let err = read_frame(&mut Cursor::new(vec![0x1E, 0, 0, 0, 0]), CLIENT_MAX_LEN).unwrap_err();
    assert!(matches!(err, FrameError::UnknownType(0x1E)));
}

#[test]
fn length_bound_is_checked_before_reading_the_payload() {
    let header = |len: u32| {
        let mut bytes = vec![0x13];
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes
    };
    let err = read_frame(&mut Cursor::new(header(98_308)), CLIENT_MAX_LEN).unwrap_err();
    assert!(matches!(
        err,
        FrameError::TooLong {
            len: 98_308,
            max: 98_307
        }
    ));
    let err = read_frame(&mut Cursor::new(header(u32::MAX)), SERVER_MAX_LEN).unwrap_err();
    assert!(matches!(err, FrameError::TooLong { .. }));
    let mut exact = header(98_307);
    exact.resize(5 + 98_307, 0);
    assert_eq!(
        read_frame(&mut Cursor::new(exact), CLIENT_MAX_LEN)
            .unwrap()
            .payload
            .len(),
        98_307
    );
}

#[test]
fn a_cut_frame_means_a_closed_connection() {
    for bytes in [vec![], vec![0x11, 0], vec![0x11, 0, 0, 0, 5, b'{']] {
        let result = read_frame(&mut Cursor::new(bytes), CLIENT_MAX_LEN);
        assert!(matches!(result, Err(FrameError::Closed)));
    }
}

#[test]
fn an_empty_payload_is_allowed() {
    let frame = read_frame(&mut Cursor::new(vec![0x1A, 0, 0, 0, 0]), SERVER_MAX_LEN).unwrap();
    assert_eq!(
        frame,
        Frame {
            kind: FrameType::Rand,
            payload: vec![]
        }
    );
}
