//! La génération de code (D-016) : l'arbre vérifié devient un texte assembleur (T12), une instruction par ligne,
//! avec la ligne source en commentaire (`; agon:12`).
//!
//! Les variables vivent en mémoire, à adresses fixes (`LOAD k`, `STORE k`), et les expressions sur la pile : chaque
//! expression laisse exactement une valeur, chaque instruction n'en laisse aucune.

use std::collections::HashMap;
use std::fmt::Display;

use crate::CompileError;
use crate::ast::{BinOp, Expr, Function, Item, Module, Stmt, UnOp};
use crate::check::{Checked, eval};

pub(crate) fn generate(module: &Module, checked: &Checked) -> Result<String, CompileError> {
    let mut generator = Generator {
        checked,
        code: String::new(),
        constants: Vec::new(),
        labels: 0,
        cells: HashMap::new(),
        line: 0,
    };
    generator.emit("CALL fn_main");
    generator.emit("HALT");
    for item in &module.items {
        if let Item::Function(function) = item {
            generator.function(function)?;
        }
    }

    let mut text = String::from("; Généré par le compilateur agon (D-016).\n");
    for constant in &generator.constants {
        text.push_str(&format!(".const {constant}\n"));
    }
    text.push_str(&generator.code);
    Ok(text)
}

struct Generator<'c> {
    checked: &'c Checked,
    code: String,
    /// La table des constantes du programme, sans doublon : l'indice d'une valeur est l'opérande de son `CONST`.
    constants: Vec<i64>,
    /// Le compteur des étiquettes `L0`, `L1`…
    labels: usize,
    /// Les cellules des variables de la fonction en cours.
    cells: HashMap<String, u8>,
    /// La ligne source de ce qu'on génère, pour les commentaires.
    line: u32,
}

impl Generator<'_> {
    /// Une instruction, et la ligne source qui l'a produite ; l'amorce `CALL fn_main ; HALT` n'en a pas.
    fn emit(&mut self, instr: impl Display) {
        let line = match self.line {
            0 => format!("    {instr}\n"),
            line => format!("    {:<20}; agon:{line}\n", instr.to_string()),
        };
        self.code.push_str(&line);
    }

    fn label(&mut self) -> String {
        let label = format!("L{}", self.labels);
        self.labels = self.labels.saturating_add(1);
        label
    }

    /// Pose une étiquette, toujours suivie du marqueur qu'un saut exige (B.4).
    fn place(&mut self, label: &str) {
        self.code.push_str(&format!("{label}:\n"));
        self.emit("JUMPDEST");
    }

    /// Une erreur qui ne devrait jamais arriver : `check` a déjà tout vérifié.
    fn internal(&self, what: String) -> CompileError {
        CompileError {
            line: self.line,
            message: format!("erreur interne du compilateur : {what}"),
        }
    }

    fn cell(&self, name: &str) -> Result<u8, CompileError> {
        self.cells
            .get(name)
            .copied()
            .ok_or_else(|| self.internal(format!("la variable « {name} » n'a pas de cellule")))
    }

    fn constant(&self, expr: &Expr) -> Result<i64, CompileError> {
        eval(expr, &self.checked.consts)
            .map_err(|_| self.internal("une constante ne se calcule pas".to_string()))
    }

    /// `PUSHI16` si la valeur tient sur un `i16`, sinon un `CONST` sur la table des constantes.
    fn literal(&mut self, value: i64) {
        if let Ok(small) = i16::try_from(value) {
            self.emit(format!("PUSHI16 {small}"));
            return;
        }
        let index = match self.constants.iter().position(|&known| known == value) {
            Some(index) => index,
            None => {
                self.constants.push(value);
                self.constants.len().saturating_sub(1)
            }
        };
        self.emit(format!("CONST {index}"));
    }

    /// Le prologue range les arguments dans les cellules des paramètres, du dernier au premier : le dernier est au
    /// sommet de la pile. La fin du corps rend 0.
    fn function(&mut self, function: &Function) -> Result<(), CompileError> {
        self.cells = self
            .checked
            .cells
            .get(&function.name)
            .cloned()
            .unwrap_or_default();
        self.line = function.line;
        self.code.push_str(&format!("fn_{}:\n", function.name));
        self.emit("JUMPDEST");
        for param in function.params.iter().rev() {
            let cell = self.cell(param)?;
            self.emit(format!("STORE {cell}"));
        }
        self.block(&function.body)?;
        self.emit("PUSHI16 0");
        self.emit("RETURN");
        Ok(())
    }

    fn block(&mut self, body: &[Stmt]) -> Result<(), CompileError> {
        body.iter().try_for_each(|stmt| self.stmt(stmt))
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        self.line = stmt.line();
        match stmt {
            Stmt::Let { name, value, .. } | Stmt::Assign { name, value, .. } => {
                self.expr(value)?;
                let cell = self.cell(name)?;
                self.emit(format!("STORE {cell}"));
            }
            // `STOREX` attend `v addr` : la valeur, puis l'adresse au sommet.
            Stmt::MemStore { addr, value, .. } => {
                self.expr(value)?;
                match eval(addr, &self.checked.consts) {
                    Ok(cell) => self.emit(format!("STORE {cell}")),
                    Err(_) => {
                        self.expr(addr)?;
                        self.emit("STOREX");
                    }
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                self.expr(cond)?;
                let otherwise_label = self.label();
                self.emit(format!("JZ {otherwise_label}"));
                self.block(then)?;
                match otherwise {
                    None => self.place(&otherwise_label),
                    Some(otherwise) => {
                        let end = self.label();
                        self.emit(format!("JMP {end}"));
                        self.place(&otherwise_label);
                        self.block(otherwise)?;
                        self.place(&end);
                    }
                }
            }
            Stmt::While { cond, body, .. } => {
                let (start, end) = (self.label(), self.label());
                self.place(&start);
                self.expr(cond)?;
                self.emit(format!("JZ {end}"));
                self.block(body)?;
                self.emit(format!("JMP {start}"));
                self.place(&end);
            }
            Stmt::Return { value, .. } => {
                match value {
                    Some(value) => self.expr(value)?,
                    None => self.emit("PUSHI16 0"),
                }
                self.emit("RETURN");
            }
            Stmt::Halt { .. } => self.emit("HALT"),
            // `TRACE` ne laisse rien sur la pile : inutile d'empiler son 0 pour le jeter.
            Stmt::Expr {
                expr: Expr::Call { name, args, .. },
                ..
            } if name == "trace" => {
                if let [arg] = args.as_slice() {
                    self.expr(arg)?;
                }
                self.emit("TRACE");
            }
            Stmt::Expr { expr, .. } => {
                self.expr(expr)?;
                self.emit("DROP");
            }
        }
        Ok(())
    }

    fn expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Int(n, _) => self.literal(*n),
            Expr::Var(name, _) => match self.checked.consts.get(name) {
                Some(&value) => self.literal(value),
                None => {
                    let cell = self.cell(name)?;
                    self.emit(format!("LOAD {cell}"));
                }
            },
            Expr::Unary { op, expr, .. } => match (op, expr.as_ref()) {
                // Un littéral négatif : un seul `PUSHI16 -n` plutôt que `PUSHI16 n ; NEG`.
                (UnOp::Neg, Expr::Int(n, _)) => self.literal(n.wrapping_neg()),
                _ => {
                    self.expr(expr)?;
                    self.emit(match op {
                        UnOp::Neg => "NEG",
                        UnOp::Not => "NOT",
                        UnOp::BitNot => "BNOT",
                    });
                }
            },
            Expr::Binary { op, lhs, rhs, .. } => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                for instr in binary(*op) {
                    self.emit(instr);
                }
            }
            Expr::MemLoad { addr, .. } => match eval(addr, &self.checked.consts) {
                Ok(cell) => self.emit(format!("LOAD {cell}")),
                Err(_) => {
                    self.expr(addr)?;
                    self.emit("LOADX");
                }
            },
            Expr::Call { name, args, .. } => match (name.as_str(), args.as_slice()) {
                ("sense", [k]) => {
                    let k = self.constant(k)?;
                    self.emit(format!("SENSE {k}"));
                }
                ("act", [k, arg]) => {
                    self.expr(arg)?;
                    let k = self.constant(k)?;
                    self.emit(format!("ACT {k}"));
                }
                ("rand", []) => self.emit("RAND"),
                ("gas", []) => self.emit("GAS"),
                // En position d'expression, `trace(e)` vaut 0.
                ("trace", [arg]) => {
                    self.expr(arg)?;
                    self.emit("TRACE");
                    self.emit("PUSHI16 0");
                }
                _ => {
                    for arg in args {
                        self.expr(arg)?;
                    }
                    self.emit(format!("CALL fn_{name}"));
                }
            },
        }
        Ok(())
    }
}

/// Les instructions d'un opérateur binaire. `!=`, `<=` et `>=` n'ont pas d'opcode : ce sont des négations.
fn binary(op: BinOp) -> &'static [&'static str] {
    match op {
        BinOp::Add => &["ADD"],
        BinOp::Sub => &["SUB"],
        BinOp::Mul => &["MUL"],
        BinOp::Div => &["DIV"],
        BinOp::Mod => &["MOD"],
        BinOp::Eq => &["EQ"],
        BinOp::Ne => &["EQ", "NOT"],
        BinOp::Lt => &["LT"],
        BinOp::Gt => &["GT"],
        BinOp::Le => &["GT", "NOT"],
        BinOp::Ge => &["LT", "NOT"],
        BinOp::BitAnd => &["AND"],
        BinOp::BitOr => &["OR"],
        BinOp::BitXor => &["XOR"],
        BinOp::Shl => &["SHL"],
        BinOp::Sar => &["SAR"],
        BinOp::Shr => &["SHR"],
    }
}
