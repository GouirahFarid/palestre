//! Instructions décodées, et décodage à la volée à n'importe quel décalage du code (B.2, A.3, D-003).

use crate::{Fault, Op};

/// Une instruction décodée. Chaque opérande a son type exact : un `PUSHI16` porte un `i16`, un `LOAD` un `u8`, et
/// un opérande du mauvais type est irreprésentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    Halt,
    Const(u16),
    PushI16(i16),
    Drop,
    Dup,
    Swap,
    Over,
    Rot,
    ConstX,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Eq,
    Lt,
    Gt,
    Not,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    BNot,
    Sar,
    LoadX,
    StoreX,
    Load(u8),
    Store(u8),
    Jmp(i16),
    Jz(i16),
    Jnz(i16),
    JumpDest,
    JmpX,
    Call(i16),
    Return,
    CallX,
    Sense(u8),
    Act(u8),
    Rand,
    Gas,
    Trace,
}

impl Instr {
    pub fn op(self) -> Op {
        match self {
            Instr::Halt => Op::Halt,
            Instr::Const(_) => Op::Const,
            Instr::PushI16(_) => Op::PushI16,
            Instr::Drop => Op::Drop,
            Instr::Dup => Op::Dup,
            Instr::Swap => Op::Swap,
            Instr::Over => Op::Over,
            Instr::Rot => Op::Rot,
            Instr::ConstX => Op::ConstX,
            Instr::Add => Op::Add,
            Instr::Sub => Op::Sub,
            Instr::Mul => Op::Mul,
            Instr::Div => Op::Div,
            Instr::Mod => Op::Mod,
            Instr::Neg => Op::Neg,
            Instr::Eq => Op::Eq,
            Instr::Lt => Op::Lt,
            Instr::Gt => Op::Gt,
            Instr::Not => Op::Not,
            Instr::And => Op::And,
            Instr::Or => Op::Or,
            Instr::Xor => Op::Xor,
            Instr::Shl => Op::Shl,
            Instr::Shr => Op::Shr,
            Instr::BNot => Op::BNot,
            Instr::Sar => Op::Sar,
            Instr::LoadX => Op::LoadX,
            Instr::StoreX => Op::StoreX,
            Instr::Load(_) => Op::Load,
            Instr::Store(_) => Op::Store,
            Instr::Jmp(_) => Op::Jmp,
            Instr::Jz(_) => Op::Jz,
            Instr::Jnz(_) => Op::Jnz,
            Instr::JumpDest => Op::JumpDest,
            Instr::JmpX => Op::JmpX,
            Instr::Call(_) => Op::Call,
            Instr::Return => Op::Return,
            Instr::CallX => Op::CallX,
            Instr::Sense(_) => Op::Sense,
            Instr::Act(_) => Op::Act,
            Instr::Rand => Op::Rand,
            Instr::Gas => Op::Gas,
            Instr::Trace => Op::Trace,
        }
    }

    /// Taille en octets, opérande compris.
    pub fn size(self) -> usize {
        self.op().size()
    }

    /// Gaz de l'instruction (table de B.3, portée par `Op`).
    pub fn gas(self) -> u16 {
        self.op().gas()
    }
}

/// Décode l'instruction qui commence à `pc`, sans prélever de gaz (étapes 1 à 3 de B.2, dans cet ordre) :
/// `pc` hors du code donne `PcOutOfRange`, un octet inconnu `BadOpcode`, un opérande coupé par la fin du code
/// `Truncated`.
///
/// Rien n'est pré-décodé : un saut peut atterrir au milieu d'un opérande, et le décodage reprend à cet octet (A.3).
pub fn decode(code: &[u8], pc: usize) -> Result<Instr, Fault> {
    let byte = *code.get(pc).ok_or(Fault::PcOutOfRange)?;
    let op = Op::from_byte(byte).ok_or(Fault::BadOpcode)?;
    // `pc < code.len()` ici, donc `pc + 1` ne déborde pas ; `checked_add` le dit au lint `arithmetic_side_effects`.
    let start = pc.checked_add(1).ok_or(Fault::Truncated)?;
    let end = pc.checked_add(op.size()).ok_or(Fault::Truncated)?;
    let operand = code.get(start..end).ok_or(Fault::Truncated)?;

    Ok(match op {
        Op::Halt => Instr::Halt,
        Op::Const => Instr::Const(u16::from_be_bytes(array(operand)?)),
        Op::PushI16 => Instr::PushI16(i16::from_be_bytes(array(operand)?)),
        Op::Drop => Instr::Drop,
        Op::Dup => Instr::Dup,
        Op::Swap => Instr::Swap,
        Op::Over => Instr::Over,
        Op::Rot => Instr::Rot,
        Op::ConstX => Instr::ConstX,
        Op::Add => Instr::Add,
        Op::Sub => Instr::Sub,
        Op::Mul => Instr::Mul,
        Op::Div => Instr::Div,
        Op::Mod => Instr::Mod,
        Op::Neg => Instr::Neg,
        Op::Eq => Instr::Eq,
        Op::Lt => Instr::Lt,
        Op::Gt => Instr::Gt,
        Op::Not => Instr::Not,
        Op::And => Instr::And,
        Op::Or => Instr::Or,
        Op::Xor => Instr::Xor,
        Op::Shl => Instr::Shl,
        Op::Shr => Instr::Shr,
        Op::BNot => Instr::BNot,
        Op::Sar => Instr::Sar,
        Op::LoadX => Instr::LoadX,
        Op::StoreX => Instr::StoreX,
        Op::Load => Instr::Load(u8::from_be_bytes(array(operand)?)),
        Op::Store => Instr::Store(u8::from_be_bytes(array(operand)?)),
        Op::Jmp => Instr::Jmp(i16::from_be_bytes(array(operand)?)),
        Op::Jz => Instr::Jz(i16::from_be_bytes(array(operand)?)),
        Op::Jnz => Instr::Jnz(i16::from_be_bytes(array(operand)?)),
        Op::JumpDest => Instr::JumpDest,
        Op::JmpX => Instr::JmpX,
        Op::Call => Instr::Call(i16::from_be_bytes(array(operand)?)),
        Op::Return => Instr::Return,
        Op::CallX => Instr::CallX,
        Op::Sense => Instr::Sense(u8::from_be_bytes(array(operand)?)),
        Op::Act => Instr::Act(u8::from_be_bytes(array(operand)?)),
        Op::Rand => Instr::Rand,
        Op::Gas => Instr::Gas,
        Op::Trace => Instr::Trace,
    })
}

/// Ajoute à `out` l'encodage gros-boutiste de `instr` : l'octet d'opcode, puis l'opérande éventuel.
pub fn encode(instr: Instr, out: &mut Vec<u8>) {
    out.push(instr.op().byte());
    match instr {
        Instr::Const(k) => out.extend_from_slice(&k.to_be_bytes()),
        Instr::PushI16(n) | Instr::Jmp(n) | Instr::Jz(n) | Instr::Jnz(n) | Instr::Call(n) => {
            out.extend_from_slice(&n.to_be_bytes());
        }
        Instr::Load(k) | Instr::Store(k) | Instr::Sense(k) | Instr::Act(k) => out.push(k),
        // Les autres instructions n'ont pas d'opérande (`Operand::None`).
        _ => {}
    }
}

/// L'opérande sous forme de tableau de N octets, prêt pour `from_be_bytes`.
///
/// `decode` a déjà découpé exactement `op.size() - 1` octets : la conversion ne peut pas échouer, mais elle reste
/// typée plutôt que de paniquer.
fn array<const N: usize>(operand: &[u8]) -> Result<[u8; N], Fault> {
    operand.try_into().map_err(|_| Fault::Truncated)
}
