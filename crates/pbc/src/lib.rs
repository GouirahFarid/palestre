//! Format `PBC v1` : chargeur, opcodes et décodeur du bytecode Palestre.
#![forbid(unsafe_code)]
#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

/// Version de la spécification implémentée (`SPEC-PBC-v1.md`).
///
/// C'est l'unique occurrence de ce numéro dans le code : la version 2.0.0 sera révélée en soutenance (annexe H.2).
pub const SPEC_VERSION: &str = "1.9.0";

mod program;

pub use program::{LoadError, MAX_CODE_LEN, MAX_CONSTANTS, Program};
