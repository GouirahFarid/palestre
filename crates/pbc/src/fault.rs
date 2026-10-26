//! Les fautes d'exécution d'un tour (annexe B.5).

use std::fmt;

/// Fautes d'exécution d'un tour (B.5). `BadHeader` n'en fait pas partie : c'est une erreur de chargement, levée avant
/// toute exécution (`LoadError`, D-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fault {
    BadConst,
    BadOpcode,
    Truncated,
    PcOutOfRange,
    BadJump,
    StackUnderflow,
    StackOverflow,
    CallStackOverflow,
    CallStackUnderflow,
    OutOfGas,
    DivByZero,
    OutOfBounds,
    BadSensor,
    BadAction,
    AlreadyActed,
}

impl Fault {
    /// Les quinze fautes d'exécution, dans l'ordre de B.5.
    pub const ALL: [Fault; 15] = [
        Fault::BadConst,
        Fault::BadOpcode,
        Fault::Truncated,
        Fault::PcOutOfRange,
        Fault::BadJump,
        Fault::StackUnderflow,
        Fault::StackOverflow,
        Fault::CallStackOverflow,
        Fault::CallStackUnderflow,
        Fault::OutOfGas,
        Fault::DivByZero,
        Fault::OutOfBounds,
        Fault::BadSensor,
        Fault::BadAction,
        Fault::AlreadyActed,
    ];

    /// Chaîne normative exacte (B.5), telle qu'elle circule dans `RESULT.fault` et `SENSED.fault`.
    ///
    /// Pas de bras `_` : une faute ajoutée en v2 sans nom ne compilerait pas.
    pub fn as_str(self) -> &'static str {
        match self {
            Fault::BadConst => "BadConst",
            Fault::BadOpcode => "BadOpcode",
            Fault::Truncated => "Truncated",
            Fault::PcOutOfRange => "PcOutOfRange",
            Fault::BadJump => "BadJump",
            Fault::StackUnderflow => "StackUnderflow",
            Fault::StackOverflow => "StackOverflow",
            Fault::CallStackOverflow => "CallStackOverflow",
            Fault::CallStackUnderflow => "CallStackUnderflow",
            Fault::OutOfGas => "OutOfGas",
            Fault::DivByZero => "DivByZero",
            Fault::OutOfBounds => "OutOfBounds",
            Fault::BadSensor => "BadSensor",
            Fault::BadAction => "BadAction",
            Fault::AlreadyActed => "AlreadyActed",
        }
    }

    /// Inverse de `as_str`, sensible à la casse. `"BadHeader"` rend `None` : un `SENSED` qui nomme une faute de
    /// chargement est donc refusé naturellement (K.3).
    pub fn from_name(name: &str) -> Option<Fault> {
        Fault::ALL.into_iter().find(|fault| fault.as_str() == name)
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
