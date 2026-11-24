//! L'arbre syntaxique d'`agon` (D-015). Chaque nœud garde sa ligne, pour les erreurs de la génération de code (B4).
//!
//! `Display` l'imprime en S-expressions, sans les lignes : `(+ 1 (* 2 3))`, `(let x e)`, `(if c (then …) (else …))`.
//! C'est la forme que vérifient les tests.

use std::fmt;

/// Un fichier source : des constantes et des fonctions, dans l'ordre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Const {
        name: String,
        value: Expr,
        line: u32,
    },
    Function(Function),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Let {
        name: String,
        value: Expr,
        line: u32,
    },
    Assign {
        name: String,
        value: Expr,
        line: u32,
    },
    /// `mem[addr] = value;`
    MemStore {
        addr: Expr,
        value: Expr,
        line: u32,
    },
    /// `else if` donne un `otherwise` qui contient un seul `If`.
    If {
        cond: Expr,
        then: Vec<Stmt>,
        otherwise: Option<Vec<Stmt>>,
        line: u32,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
        line: u32,
    },
    Return {
        value: Option<Expr>,
        line: u32,
    },
    Halt {
        line: u32,
    },
    /// Une expression évaluée pour son effet, comme `trace(x);`.
    Expr {
        expr: Expr,
        line: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64, u32),
    Var(String, u32),
    Unary {
        op: UnOp,
        expr: Box<Expr>,
        line: u32,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        line: u32,
    },
    Call {
        name: String,
        args: Vec<Expr>,
        line: u32,
    },
    /// `mem[addr]`
    MemLoad {
        addr: Box<Expr>,
        line: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `-`
    Neg,
    /// `!`, le non logique (`NOT`)
    Not,
    /// `~`, le complément bit à bit (`BNOT`)
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    /// `>>`, arithmétique (`SAR`)
    Sar,
    /// `>>>`, logique (`SHR`)
    Shr,
}

/// L'opérateur s'affiche avec son symbole dans le source.
impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UnOp::Neg => "-",
            UnOp::Not => "!",
            UnOp::BitNot => "~",
        })
    }
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Sar => ">>",
            BinOp::Shr => ">>>",
        })
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Int(n, _) => write!(f, "{n}"),
            Expr::Var(name, _) => f.write_str(name),
            Expr::Unary { op, expr, .. } => write!(f, "({op} {expr})"),
            Expr::Binary { op, lhs, rhs, .. } => write!(f, "({op} {lhs} {rhs})"),
            Expr::Call { name, args, .. } => {
                write!(f, "({name}")?;
                for arg in args {
                    write!(f, " {arg}")?;
                }
                f.write_str(")")
            }
            Expr::MemLoad { addr, .. } => write!(f, "(mem {addr})"),
        }
    }
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stmt::Let { name, value, .. } => write!(f, "(let {name} {value})"),
            Stmt::Assign { name, value, .. } => write!(f, "(= {name} {value})"),
            Stmt::MemStore { addr, value, .. } => write!(f, "(mem= {addr} {value})"),
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                write!(f, "(if {cond} (then{}", Block(then))?;
                if let Some(otherwise) = otherwise {
                    write!(f, ") (else{}", Block(otherwise))?;
                }
                f.write_str("))")
            }
            Stmt::While { cond, body, .. } => write!(f, "(while {cond} (do{}))", Block(body)),
            Stmt::Return {
                value: Some(value), ..
            } => write!(f, "(return {value})"),
            Stmt::Return { value: None, .. } => f.write_str("(return)"),
            Stmt::Halt { .. } => f.write_str("(halt)"),
            Stmt::Expr { expr, .. } => write!(f, "{expr}"),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(fn {} ({}){})",
            self.name,
            self.params.join(" "),
            Block(&self.body)
        )
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Item::Const { name, value, .. } => write!(f, "(const {name} {value})"),
            Item::Function(function) => write!(f, "{function}"),
        }
    }
}

/// Les items, un par ligne.
impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, item) in self.items.iter().enumerate() {
            if index > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{item}")?;
        }
        Ok(())
    }
}

/// Des instructions, chacune précédée d'une espace : `(then)` pour un bloc vide, `(then s1 s2)` sinon.
struct Block<'a>(&'a [Stmt]);

impl fmt::Display for Block<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for stmt in self.0 {
            write!(f, " {stmt}")?;
        }
        Ok(())
    }
}
