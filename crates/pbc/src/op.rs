//! La table des 43 opcodes de B.3 : octet, mnémonique, type d'opérande et gaz.
//!
//! C'est le seul endroit du code où ces valeurs existent (D-003) : une règle de gaz ou un opcode v2 se change ici.

/// Type de l'opérande immédiat qui suit l'octet d'opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    None,
    U8,
    U16,
    I16,
}

/// Un opcode de B.3. Le `repr(u8)` fait de chaque discriminant l'octet de l'opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Op {
    Halt = 0x00,
    Const = 0x01,
    PushI16 = 0x02,
    Drop = 0x03,
    Dup = 0x04,
    Swap = 0x05,
    Over = 0x06,
    Rot = 0x07,
    ConstX = 0x08,
    Add = 0x10,
    Sub = 0x11,
    Mul = 0x12,
    Div = 0x13,
    Mod = 0x14,
    Neg = 0x15,
    Eq = 0x20,
    Lt = 0x21,
    Gt = 0x22,
    Not = 0x23,
    And = 0x24,
    Or = 0x25,
    Xor = 0x26,
    Shl = 0x27,
    Shr = 0x28,
    BNot = 0x29,
    Sar = 0x2D,
    LoadX = 0x30,
    StoreX = 0x31,
    Load = 0x32,
    Store = 0x33,
    Jmp = 0x40,
    Jz = 0x41,
    Jnz = 0x42,
    JumpDest = 0x43,
    JmpX = 0x44,
    Call = 0x50,
    Return = 0x51,
    CallX = 0x52,
    Sense = 0x60,
    Act = 0x61,
    Rand = 0x70,
    Gas = 0x71,
    Trace = 0xF0,
}

impl Op {
    /// Les 43 opcodes, par octet croissant.
    pub const ALL: [Op; 43] = [
        Op::Halt,
        Op::Const,
        Op::PushI16,
        Op::Drop,
        Op::Dup,
        Op::Swap,
        Op::Over,
        Op::Rot,
        Op::ConstX,
        Op::Add,
        Op::Sub,
        Op::Mul,
        Op::Div,
        Op::Mod,
        Op::Neg,
        Op::Eq,
        Op::Lt,
        Op::Gt,
        Op::Not,
        Op::And,
        Op::Or,
        Op::Xor,
        Op::Shl,
        Op::Shr,
        Op::BNot,
        Op::Sar,
        Op::LoadX,
        Op::StoreX,
        Op::Load,
        Op::Store,
        Op::Jmp,
        Op::Jz,
        Op::Jnz,
        Op::JumpDest,
        Op::JmpX,
        Op::Call,
        Op::Return,
        Op::CallX,
        Op::Sense,
        Op::Act,
        Op::Rand,
        Op::Gas,
        Op::Trace,
    ];

    /// L'opcode porté par cet octet, ou `None` (ce qui donnera `Fault::BadOpcode` au décodage).
    pub fn from_byte(byte: u8) -> Option<Op> {
        match byte {
            0x00 => Some(Op::Halt),
            0x01 => Some(Op::Const),
            0x02 => Some(Op::PushI16),
            0x03 => Some(Op::Drop),
            0x04 => Some(Op::Dup),
            0x05 => Some(Op::Swap),
            0x06 => Some(Op::Over),
            0x07 => Some(Op::Rot),
            0x08 => Some(Op::ConstX),
            0x10 => Some(Op::Add),
            0x11 => Some(Op::Sub),
            0x12 => Some(Op::Mul),
            0x13 => Some(Op::Div),
            0x14 => Some(Op::Mod),
            0x15 => Some(Op::Neg),
            0x20 => Some(Op::Eq),
            0x21 => Some(Op::Lt),
            0x22 => Some(Op::Gt),
            0x23 => Some(Op::Not),
            0x24 => Some(Op::And),
            0x25 => Some(Op::Or),
            0x26 => Some(Op::Xor),
            0x27 => Some(Op::Shl),
            0x28 => Some(Op::Shr),
            0x29 => Some(Op::BNot),
            0x2D => Some(Op::Sar),
            0x30 => Some(Op::LoadX),
            0x31 => Some(Op::StoreX),
            0x32 => Some(Op::Load),
            0x33 => Some(Op::Store),
            0x40 => Some(Op::Jmp),
            0x41 => Some(Op::Jz),
            0x42 => Some(Op::Jnz),
            0x43 => Some(Op::JumpDest),
            0x44 => Some(Op::JmpX),
            0x50 => Some(Op::Call),
            0x51 => Some(Op::Return),
            0x52 => Some(Op::CallX),
            0x60 => Some(Op::Sense),
            0x61 => Some(Op::Act),
            0x70 => Some(Op::Rand),
            0x71 => Some(Op::Gas),
            0xF0 => Some(Op::Trace),
            _ => None,
        }
    }

    /// L'octet de l'opcode : le discriminant fixé par `repr(u8)`.
    pub fn byte(self) -> u8 {
        self as u8
    }

    /// Mnémonique en majuscules de B.3 (indicatif selon la spec, mais utilisé par l'assembleur).
    pub fn mnemonic(self) -> &'static str {
        self.info().0
    }

    pub fn operand(self) -> Operand {
        self.info().1
    }

    /// Gaz prélevé avant l'exécution (B.2, étape 4).
    pub fn gas(self) -> u16 {
        self.info().2
    }

    /// Taille de l'instruction en octets : l'opcode plus son opérande.
    pub fn size(self) -> usize {
        match self.operand() {
            Operand::None => 1,
            Operand::U8 => 2,
            Operand::U16 | Operand::I16 => 3,
        }
    }

    /// Cherche un opcode par son mnémonique, sans tenir compte de la casse.
    pub fn from_mnemonic(text: &str) -> Option<Op> {
        Op::ALL
            .into_iter()
            .find(|op| op.mnemonic().eq_ignore_ascii_case(text))
    }

    /// La table de B.3 : (mnémonique, opérande, gaz). Elle n'est volontairement ni monotone ni intuitive (D.1).
    fn info(self) -> (&'static str, Operand, u16) {
        match self {
            Op::Halt => ("HALT", Operand::None, 0),
            Op::Const => ("CONST", Operand::U16, 1),
            Op::PushI16 => ("PUSHI16", Operand::I16, 1),
            Op::Drop => ("DROP", Operand::None, 1),
            Op::Dup => ("DUP", Operand::None, 1),
            Op::Swap => ("SWAP", Operand::None, 1),
            Op::Over => ("OVER", Operand::None, 1),
            Op::Rot => ("ROT", Operand::None, 1),
            Op::ConstX => ("CONSTX", Operand::None, 2),
            Op::Add => ("ADD", Operand::None, 2),
            Op::Sub => ("SUB", Operand::None, 2),
            Op::Mul => ("MUL", Operand::None, 2),
            Op::Div => ("DIV", Operand::None, 7),
            Op::Mod => ("MOD", Operand::None, 7),
            Op::Neg => ("NEG", Operand::None, 2),
            Op::Eq => ("EQ", Operand::None, 2),
            Op::Lt => ("LT", Operand::None, 2),
            Op::Gt => ("GT", Operand::None, 2),
            Op::Not => ("NOT", Operand::None, 2),
            Op::And => ("AND", Operand::None, 2),
            Op::Or => ("OR", Operand::None, 2),
            Op::Xor => ("XOR", Operand::None, 4),
            Op::Shl => ("SHL", Operand::None, 3),
            Op::Shr => ("SHR", Operand::None, 6),
            Op::BNot => ("BNOT", Operand::None, 2),
            Op::Sar => ("SAR", Operand::None, 6),
            Op::LoadX => ("LOADX", Operand::None, 4),
            Op::StoreX => ("STOREX", Operand::None, 5),
            Op::Load => ("LOAD", Operand::U8, 3),
            Op::Store => ("STORE", Operand::U8, 4),
            Op::Jmp => ("JMP", Operand::I16, 2),
            Op::Jz => ("JZ", Operand::I16, 2),
            Op::Jnz => ("JNZ", Operand::I16, 2),
            Op::JumpDest => ("JUMPDEST", Operand::None, 1),
            Op::JmpX => ("JMPX", Operand::None, 3),
            Op::Call => ("CALL", Operand::I16, 6),
            Op::Return => ("RETURN", Operand::None, 4),
            Op::CallX => ("CALLX", Operand::None, 7),
            Op::Sense => ("SENSE", Operand::U8, 8),
            Op::Act => ("ACT", Operand::U8, 12),
            Op::Rand => ("RAND", Operand::None, 5),
            Op::Gas => ("GAS", Operand::None, 2),
            Op::Trace => ("TRACE", Operand::None, 1),
        }
    }
}
