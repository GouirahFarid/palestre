//! Le serveur ne panique sur aucune suite d'octets reçue (invariant 5, K.2, K.8).
#![no_main]
use std::io::{self, Cursor, Read, Write};

use exec::server::{ServerConfig, handle_connection};
use libfuzzer_sys::fuzz_target;

/// Un flux en mémoire : on lit les octets du fuzzer, on jette ce que le serveur écrit.
struct Duplex {
    input: Cursor<Vec<u8>>,
}

impl Read for Duplex {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf)
    }
}

impl Write for Duplex {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    let mut stream = Duplex { input: Cursor::new(data.to_vec()) };
    let _ = handle_connection(&mut stream, &ServerConfig::default());
});
