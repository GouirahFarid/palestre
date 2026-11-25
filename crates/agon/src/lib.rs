//! `agon` : un langage pour écrire des agents (D-015), compilé vers `PBC` (D-016).
//!
//! ```text
//! const CARRIED = 3;
//! fn main() {
//!     if sense(CARRIED) != 0 { act(3, sense(CARRIED)); }   // manger tout ce qu'il porte
//! }
//! ```
//!
//! Le chemin : `parser::parse` (l'arbre), `check` (les noms, l'arité, la récursion, les constantes), `codegen` (un
//! texte assembleur), puis `asm::assemble`.
#![forbid(unsafe_code)]
#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

pub mod ast;
mod check;
mod codegen;
pub mod lexer;
pub mod parser;

use pbc::Program;

use crate::lexer::SyntaxError;

/// Une erreur de compilation, avec la ligne du source. La ligne 0 désigne le programme entier : `main` manquant, ou
/// un refus de l'assembleur.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ligne {line} : {message}")]
pub struct CompileError {
    pub line: u32,
    pub message: String,
}

/// Une erreur de syntaxe garde sa position : la ligne, et la colonne dans le message.
impl From<SyntaxError> for CompileError {
    fn from(error: SyntaxError) -> CompileError {
        CompileError {
            line: error.line,
            message: format!("colonne {} : {}", error.col, error.message),
        }
    }
}

/// Le texte assembleur commenté (`palestre compile --emit-asm`), avant l'assemblage.
pub fn compile_to_asm(source: &str) -> Result<String, CompileError> {
    let module = parser::parse(source)?;
    let checked = check::check(&module)?;
    codegen::generate(&module, &checked)
}

/// Compile un source `agon` en programme prêt à charger.
pub fn compile(source: &str) -> Result<Program, CompileError> {
    let text = compile_to_asm(source)?;
    asm::assemble(&text).map_err(|error| CompileError {
        line: 0,
        message: format!("l'assembleur refuse le code généré : {error}"),
    })
}
