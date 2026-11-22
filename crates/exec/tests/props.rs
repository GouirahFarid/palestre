#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{self, Cursor, Read, Write};

use exec::frame::{FrameType, SERVER_MAX_LEN, read_frame};
use exec::message::{self, ErrorMsg};
use exec::server::{ServerConfig, handle_connection};
use proptest::collection::vec;
use proptest::prelude::*;

/// Un flux en mémoire : on lit `input`, on écrit dans `output`.
struct Duplex {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
}

impl Read for Duplex {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf)
    }
}

impl Write for Duplex {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Le serveur n'émet que des trames bien formées, et rien après une ERROR fermante (K.8).
fn check_output(output: Vec<u8>) -> Result<(), TestCaseError> {
    let len = output.len();
    let mut cursor = Cursor::new(output);
    let mut frames = Vec::new();
    while let Ok(frame) = read_frame(&mut cursor, SERVER_MAX_LEN) {
        frames.push(frame);
    }
    prop_assert_eq!(
        cursor.position() as usize,
        len,
        "octets non encadrés en sortie"
    );
    for (i, frame) in frames.iter().enumerate() {
        if frame.kind == FrameType::Error {
            let err: ErrorMsg = message::from_json(&frame.payload).unwrap();
            if err.code.closes_connection() {
                prop_assert_eq!(i + 1, frames.len(), "trame émise après une ERROR fermante");
            }
        }
    }
    Ok(())
}

fn open_frame() -> Vec<u8> {
    let mut bytes = vec![0x11, 0, 0, 0, 11];
    bytes.extend_from_slice(br#"{"proto":1}"#);
    bytes
}

proptest! {
    #[test]
    fn arbitrary_bytes_never_crash_the_server(bytes in vec(any::<u8>(), 0..2048)) {
        let mut stream = Duplex { input: Cursor::new(bytes), output: Vec::new() };
        let _ = handle_connection(&mut stream, &ServerConfig::default());
        check_output(stream.output)?;
    }

    #[test]
    fn garbage_after_a_valid_open_never_crashes_the_server(tail in vec(any::<u8>(), 0..2048)) {
        let mut input = open_frame();
        input.extend_from_slice(&tail);
        let mut stream = Duplex { input: Cursor::new(input), output: Vec::new() };
        let _ = handle_connection(&mut stream, &ServerConfig::default());
        check_output(stream.output)?;
    }
}
