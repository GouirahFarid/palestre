//! Le corpus de cas de test : lire les cas (`corpus/FORMAT.md`), les rejouer contre une machine, comparer.
#![forbid(unsafe_code)]

mod case;
mod check;
pub mod toml;

pub use case::{
    Answer, Call, Case, CaseError, ExecCase, Expectation, comparable, load_case, load_corpus,
};
pub use check::{
    Backend, BackendError, CaseFailure, Divergence, InProcess, Reason, Remote, Replay, check_case,
    compare,
};
