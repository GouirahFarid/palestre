//! Désassemblage par balayage linéaire (D-009). Un octet qui ne se décode pas sort en `.byte`, si bien que
//! `assemble(&disassemble(&p)) == Ok(p)` pour tout programme, même pour du code qui ne s'exécuterait pas.

use pbc::{Instr, Program, decode};

/// Le listing d'un programme : une ligne `.const` par constante, puis une ligne par instruction, commentée de son
/// adresse et, pour un saut relatif, de sa cible. Les opérandes sortent en décimal signé.
pub fn disassemble(program: &Program) -> String {
    let code = program.code();
    let mut lines: Vec<String> = program
        .constants()
        .iter()
        .map(|value| format!(".const {value}"))
        .collect();

    let mut pc = 0;
    while let Some(&byte) = code.get(pc) {
        let (text, size, comment) = match decode(code, pc) {
            Ok(instr) => {
                let mnemonic = instr.op().mnemonic();
                let text = match operand(instr) {
                    Some(value) => format!("{mnemonic} {value}"),
                    None => mnemonic.to_string(),
                };
                let comment = offset(instr)
                    .map(|d| format!(" -> {}", target(pc + instr.size(), d, code.len())));
                (text, instr.size(), comment.unwrap_or_default())
            }
            // Opcode inconnu ou opérande coupé par la fin du code : un seul octet brut, et on reprend au suivant.
            Err(_) => (format!(".byte 0x{byte:02X}"), 1, String::new()),
        };
        lines.push(format!("    {text:<16}; 0x{pc:04X}{comment}"));
        pc += size;
    }

    lines.push(String::new());
    lines.join("\n")
}

/// L'opérande immédiat, s'il y en a un.
fn operand(instr: Instr) -> Option<i64> {
    match instr {
        Instr::Const(k) => Some(i64::from(k)),
        Instr::Load(k) | Instr::Store(k) | Instr::Sense(k) | Instr::Act(k) => Some(i64::from(k)),
        Instr::PushI16(n) | Instr::Jmp(n) | Instr::Jz(n) | Instr::Jnz(n) | Instr::Call(n) => {
            Some(i64::from(n))
        }
        _ => None,
    }
}

/// Le déplacement d'un saut relatif (B.4).
fn offset(instr: Instr) -> Option<i16> {
    match instr {
        Instr::Jmp(d) | Instr::Jz(d) | Instr::Jnz(d) | Instr::Call(d) => Some(d),
        _ => None,
    }
}

/// La cible `next + d` d'un saut relatif, en hexadécimal, ou « hors du code ».
fn target(next: usize, d: i16, code_len: usize) -> String {
    let target = i64::try_from(next)
        .ok()
        .and_then(|next| next.checked_add(i64::from(d)))
        .and_then(|target| usize::try_from(target).ok())
        .filter(|&target| target < code_len);
    match target {
        Some(target) => format!("0x{target:04X}"),
        None => "hors du code".to_string(),
    }
}
