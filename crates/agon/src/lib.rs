//! `agon` : un langage pour écrire des agents (D-015).
#![forbid(unsafe_code)]
#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

pub mod ast;
pub mod lexer;
pub mod parser;
