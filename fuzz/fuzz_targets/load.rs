//! Le chargeur ne panique sur aucune suite d'octets (invariant 5).
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = pbc::Program::load(data);
});
