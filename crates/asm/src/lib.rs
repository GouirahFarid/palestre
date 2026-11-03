//! Assembleur et désassembleur `PBC v1` (D-009).
//!
//! ```text
//! ; commentaire jusqu'à la fin de la ligne
//! .const 1234567890123     ; ajoute une constante (index 0, 1, … dans l'ordre)
//! debut:                   ; étiquette = adresse de l'octet suivant
//!     JUMPDEST             ; une cible de saut doit porter le marqueur : l'assembleur ne l'ajoute pas
//!     PUSHI16 -3           ; décimal signé, ou 0x7FFF
//!     CONST 0              ; index de constante (u16)
//!     LOAD 7               ; u8
//!     JMP debut            ; une étiquette donne le déplacement relatif (B.4)…
//!     JZ +4                ; … ou un déplacement brut
//!     PUSHI16 @gestion     ; @étiquette = adresse absolue (pour JMPX/CALLX), qui doit tenir sur un i16
//!     .byte 0x43 255       ; octets bruts
//! ```
//!
//! Les mnémoniques et les directives sont insensibles à la casse, les étiquettes y sont sensibles.
#![forbid(unsafe_code)]

mod assemble;
mod disassemble;

pub use assemble::{AsmError, AsmErrorKind, assemble};
pub use disassemble::disassemble;
