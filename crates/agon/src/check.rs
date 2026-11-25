//! Les vérifications sémantiques (D-016) : ce que la grammaire ne voit pas. Elles préparent aussi la génération de
//! code : la valeur de chaque constante, et la cellule de chaque variable.

use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;

use crate::CompileError;
use crate::ast::{BinOp, Expr, Function, Item, Module, Stmt, UnOp};

/// Les cellules de l'agent, par `mem[…]`. Les autres, `128..=255`, sont aux variables (D-016).
const USER_CELLS: RangeInclusive<i64> = 0..=127;
/// La première cellule d'une variable ; les suivantes descendent jusqu'à 128.
const LAST_CELL: u8 = 255;
const FIRST_CELL: u8 = 128;
/// Les cadres de la pile d'appels (B.1), celui de `main` compris.
const CALL_DEPTH: usize = 16;

/// Les fonctions prédéfinies et leur arité. Le premier argument de `sense` et d'`act` est l'opérande immédiat de
/// l'instruction : il doit être constant.
const BUILTINS: [(&str, usize); 5] = [
    ("sense", 1),
    ("act", 2),
    ("rand", 0),
    ("gas", 0),
    ("trace", 1),
];

/// Ce que la génération de code doit savoir du programme vérifié.
pub(crate) struct Checked {
    /// La valeur de chaque constante.
    pub consts: HashMap<String, i64>,
    /// Pour chaque fonction, la cellule de chacune de ses variables (un nom est unique dans sa fonction).
    pub cells: HashMap<String, HashMap<String, u8>>,
}

pub(crate) fn check(module: &Module) -> Result<Checked, CompileError> {
    // Les constantes d'abord, dans l'ordre : chacune ne peut citer que celles d'avant elle.
    let mut consts = HashMap::new();
    for item in &module.items {
        if let Item::Const { name, value, line } = item {
            if consts.contains_key(name) {
                return Err(error(
                    *line,
                    format!("constante « {name} » définie deux fois"),
                ));
            }
            let value = eval(value, &consts).map_err(|failure| match failure {
                Eval::DivByZero(at) => error(
                    at,
                    format!("division par zéro dans la constante « {name} »"),
                ),
                Eval::NotConstant(at, what) => error(
                    at,
                    format!(
                        "« {name} » doit se calculer à la compilation, et {what} ne le peut pas"
                    ),
                ),
            })?;
            consts.insert(name.clone(), value);
        }
    }

    // Puis la table des fonctions : leur indice dans le graphe d'appel et leur arité.
    let functions: Vec<&Function> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) => Some(function),
            Item::Const { .. } => None,
        })
        .collect();
    let mut table = HashMap::new();
    for (index, function) in functions.iter().enumerate() {
        let name = &function.name;
        if builtin(name).is_some() {
            return Err(error(
                function.line,
                format!("« {name} » est une fonction prédéfinie"),
            ));
        }
        if table
            .insert(name.as_str(), (index, function.params.len()))
            .is_some()
        {
            return Err(error(
                function.line,
                format!("fonction « {name} » définie deux fois"),
            ));
        }
    }
    let main = match table.get("main") {
        Some(&(index, 0)) => index,
        Some(_) => {
            return Err(error(
                line_of(&functions, "main"),
                "« main » ne prend pas de paramètre",
            ));
        }
        None => return Err(error(0, "il manque la fonction « main »")),
    };

    // Le corps de chaque fonction, qui reçoit au passage ses cellules et ses appels.
    let mut cells = HashMap::new();
    let mut calls = Vec::with_capacity(functions.len());
    let mut allocated = 0;
    for function in &functions {
        let mut body = Body {
            consts: &consts,
            functions: &table,
            scopes: Vec::new(),
            cells: HashMap::new(),
            allocated: &mut allocated,
            calls: Vec::new(),
        };
        body.function(function)?;
        calls.push(body.calls);
        cells.insert(function.name.clone(), body.cells);
    }

    check_calls(&functions, &calls, main)?;
    Ok(Checked { consts, cells })
}

/// Pourquoi une expression ne se calcule pas à la compilation : le nœud fautif, et sa ligne.
pub(crate) enum Eval {
    NotConstant(u32, String),
    DivByZero(u32),
}

/// Calcule une expression constante avec l'arithmétique de la machine : tout enveloppe (§0), la division tronque
/// vers zéro, et les décalages suivent B.6. Le test `constant_folding_matches_the_machine` le vérifie contre la VM.
pub(crate) fn eval(expr: &Expr, consts: &HashMap<String, i64>) -> Result<i64, Eval> {
    match expr {
        Expr::Int(n, _) => Ok(*n),
        Expr::Var(name, line) => consts.get(name).copied().ok_or_else(|| {
            Eval::NotConstant(
                *line,
                format!("« {name} » n'est pas une constante définie avant"),
            )
        }),
        Expr::Unary { op, expr, .. } => {
            let a = eval(expr, consts)?;
            Ok(match op {
                UnOp::Neg => a.wrapping_neg(),
                UnOp::Not => i64::from(a == 0),
                UnOp::BitNot => !a,
            })
        }
        Expr::Binary { op, lhs, rhs, line } => {
            let (a, b) = (eval(lhs, consts)?, eval(rhs, consts)?);
            Ok(match op {
                BinOp::Add => a.wrapping_add(b),
                BinOp::Sub => a.wrapping_sub(b),
                BinOp::Mul => a.wrapping_mul(b),
                // `checked_div` n'échoue plus que pour `MIN / -1`, qui enveloppe ; `MIN % -1` vaut 0.
                BinOp::Div if b == 0 => return Err(Eval::DivByZero(*line)),
                BinOp::Div => a.checked_div(b).unwrap_or(i64::MIN),
                BinOp::Mod if b == 0 => return Err(Eval::DivByZero(*line)),
                BinOp::Mod => a.checked_rem(b).unwrap_or(0),
                BinOp::Eq => i64::from(a == b),
                BinOp::Ne => i64::from(a != b),
                BinOp::Lt => i64::from(a < b),
                BinOp::Gt => i64::from(a > b),
                BinOp::Le => i64::from(a <= b),
                BinOp::Ge => i64::from(a >= b),
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                BinOp::BitXor => a ^ b,
                // Un compte hors de 0..=63 donne 0, ou le remplissage de signe pour `>>` (B.6).
                BinOp::Shl => shift(b)
                    .and_then(|n| (a as u64).checked_shl(n))
                    .map_or(0, |v| v as i64),
                BinOp::Shr => shift(b)
                    .and_then(|n| (a as u64).checked_shr(n))
                    .map_or(0, |v| v as i64),
                BinOp::Sar => {
                    shift(b)
                        .and_then(|n| a.checked_shr(n))
                        .unwrap_or(if a < 0 { -1 } else { 0 })
                }
            })
        }
        Expr::Call { name, line, .. } => {
            Err(Eval::NotConstant(*line, format!("l'appel de « {name} »")))
        }
        Expr::MemLoad { line, .. } => Err(Eval::NotConstant(*line, "« mem[…] »".to_string())),
    }
}

/// Le compte d'un décalage, s'il est dans `0..=63`.
fn shift(n: i64) -> Option<u32> {
    u32::try_from(n).ok().filter(|&n| n < 64)
}

/// L'arité d'une fonction prédéfinie.
pub(crate) fn builtin(name: &str) -> Option<usize> {
    BUILTINS
        .iter()
        .find(|(builtin, _)| *builtin == name)
        .map(|&(_, arity)| arity)
}

fn error(line: u32, message: impl Into<String>) -> CompileError {
    CompileError {
        line,
        message: message.into(),
    }
}

fn line_of(functions: &[&Function], name: &str) -> u32 {
    functions
        .iter()
        .find(|function| function.name == name)
        .map_or(0, |function| function.line)
}

fn arguments(n: usize) -> String {
    match n {
        0 | 1 => format!("{n} argument"),
        _ => format!("{n} arguments"),
    }
}

/// La vérification d'un corps de fonction.
struct Body<'c> {
    consts: &'c HashMap<String, i64>,
    /// Indice dans le graphe d'appel et arité de chaque fonction.
    functions: &'c HashMap<&'c str, (usize, usize)>,
    /// Les variables visibles, un ensemble par bloc ouvert.
    scopes: Vec<HashSet<String>>,
    /// La cellule de chaque variable déclarée dans la fonction, visible ou non.
    cells: HashMap<String, u8>,
    /// Le nombre de cellules déjà données, toutes fonctions confondues.
    allocated: &'c mut usize,
    /// Les fonctions appelées, avec la ligne de l'appel.
    calls: Vec<(usize, u32)>,
}

impl Body<'_> {
    fn function(&mut self, function: &Function) -> Result<(), CompileError> {
        self.scopes.push(HashSet::new());
        for param in &function.params {
            self.declare(param, function.line)?;
        }
        self.block(&function.body)
    }

    /// Un bloc ouvre une portée : ses `let` cessent d'être visibles à sa fin.
    fn block(&mut self, body: &[Stmt]) -> Result<(), CompileError> {
        self.scopes.push(HashSet::new());
        for stmt in body {
            self.stmt(stmt)?;
        }
        self.scopes.pop();
        Ok(())
    }

    /// Déclare une variable dans le bloc courant et lui donne sa cellule, de 255 vers le bas.
    fn declare(&mut self, name: &str, line: u32) -> Result<(), CompileError> {
        if self.consts.contains_key(name) {
            return Err(error(
                line,
                format!("« {name} » est une constante : une variable ne peut pas porter son nom"),
            ));
        }
        if self.cells.contains_key(name) {
            return Err(error(
                line,
                format!("variable « {name} » déjà déclarée dans cette fonction"),
            ));
        }
        let cell = u8::try_from(*self.allocated)
            .ok()
            .and_then(|n| LAST_CELL.checked_sub(n))
            .filter(|&cell| cell >= FIRST_CELL)
            .ok_or_else(|| {
                error(line, format!("trop de variables pour « {name} » : 128 au plus, dans les cellules 128 à 255"))
            })?;
        *self.allocated = self.allocated.saturating_add(1);
        self.cells.insert(name.to_string(), cell);
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
        Ok(())
    }

    fn visible(&self, name: &str) -> bool {
        self.scopes.iter().any(|scope| scope.contains(name))
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            // La valeur d'abord : dans `let y = y + 1;`, le `y` de droite n'existe pas encore.
            Stmt::Let { name, value, line } => {
                self.expr(value)?;
                self.declare(name, *line)
            }
            Stmt::Assign { name, value, line } => {
                if self.consts.contains_key(name) {
                    return Err(error(
                        *line,
                        format!("on ne peut pas affecter la constante « {name} »"),
                    ));
                }
                if !self.visible(name) {
                    return Err(error(*line, format!("variable inconnue « {name} »")));
                }
                self.expr(value)
            }
            Stmt::MemStore { addr, value, line } => {
                self.address(addr, *line)?;
                self.expr(value)
            }
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                self.expr(cond)?;
                self.block(then)?;
                otherwise
                    .as_deref()
                    .map_or(Ok(()), |otherwise| self.block(otherwise))
            }
            Stmt::While { cond, body, .. } => {
                self.expr(cond)?;
                self.block(body)
            }
            Stmt::Return { value, .. } => value.as_ref().map_or(Ok(()), |value| self.expr(value)),
            Stmt::Halt { .. } => Ok(()),
            Stmt::Expr { expr, .. } => self.expr(expr),
        }
    }

    fn expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Int(..) => Ok(()),
            Expr::Var(name, line) => {
                if self.consts.contains_key(name) || self.visible(name) {
                    Ok(())
                } else {
                    Err(error(*line, format!("variable inconnue « {name} »")))
                }
            }
            Expr::Unary { expr, .. } => self.expr(expr),
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs)?;
                self.expr(rhs)
            }
            Expr::MemLoad { addr, line } => self.address(addr, *line),
            Expr::Call { name, args, line } => self.call(name, args, *line),
        }
    }

    fn call(&mut self, name: &str, args: &[Expr], line: u32) -> Result<(), CompileError> {
        let arity = match (builtin(name), self.functions.get(name)) {
            (Some(arity), _) => arity,
            (None, Some(&(index, arity))) => {
                self.calls.push((index, line));
                arity
            }
            (None, None) => return Err(error(line, format!("fonction inconnue « {name} »"))),
        };
        if args.len() != arity {
            return Err(error(
                line,
                format!(
                    "« {name} » attend {}, en reçoit {}",
                    arguments(arity),
                    args.len()
                ),
            ));
        }
        for arg in args {
            self.expr(arg)?;
        }
        match (name, args) {
            ("sense", [k, ..]) => self.immediate("sense", k, 0..=255),
            ("act", [k, ..]) => self.immediate("act", k, 0..=3),
            _ => Ok(()),
        }
    }

    /// Le numéro d'un capteur ou d'une action est l'opérande immédiat de `SENSE k` ou `ACT k` : une constante.
    fn immediate(
        &self,
        builtin: &str,
        k: &Expr,
        range: RangeInclusive<i64>,
    ) -> Result<(), CompileError> {
        let (start, end) = (range.start(), range.end());
        match eval(k, self.consts) {
            Ok(value) if range.contains(&value) => Ok(()),
            Ok(value) => Err(error(
                k.line(),
                format!("{builtin} : le premier argument vaut {value}, hors de {start} à {end}"),
            )),
            Err(Eval::DivByZero(at)) => Err(error(at, format!("{builtin} : division par zéro"))),
            Err(Eval::NotConstant(at, _)) => Err(error(
                at,
                format!(
                    "{builtin} : le premier argument doit être une constante, de {start} à {end}"
                ),
            )),
        }
    }

    /// Une adresse `mem[…]` constante doit désigner une cellule de l'agent, pas celle d'une variable.
    fn address(&mut self, addr: &Expr, line: u32) -> Result<(), CompileError> {
        self.expr(addr)?;
        match eval(addr, self.consts) {
            Ok(value) if !USER_CELLS.contains(&value) => Err(error(
                line,
                format!(
                    "mem[{value}] : l'agent a les cellules 0 à 127, celles de 128 à 255 sont aux variables"
                ),
            )),
            _ => Ok(()),
        }
    }
}

/// Refuse la récursion, puis les chaînes d'appels de plus de 16 cadres.
///
/// Un parcours en profondeur à trois couleurs, sans récursion : la pile du parcours est explicite, c'est elle qui
/// donne le chemin d'un cycle. Une fonction devient noire quand toutes celles qu'elle appelle le sont : on connaît
/// alors la plus longue chaîne d'appels qui part d'elle.
fn check_calls(
    functions: &[&Function],
    calls: &[Vec<(usize, u32)>],
    main: usize,
) -> Result<(), CompileError> {
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }
    let name = |index: usize| {
        functions
            .get(index)
            .map_or("?", |function| function.name.as_str())
    };
    let callees = |index: usize| calls.get(index).map_or(&[][..], Vec::as_slice);
    let mut color = vec![Color::White; functions.len()];
    // Pour chaque fonction : le nombre de cadres de sa plus longue chaîne, et la fonction appelée par cette chaîne.
    let mut frames = vec![(1usize, None::<(usize, u32)>); functions.len()];

    for root in 0..functions.len() {
        if color.get(root) != Some(&Color::White) {
            continue;
        }
        let mut stack = vec![(root, 0usize)];
        if let Some(c) = color.get_mut(root) {
            *c = Color::Gray;
        }
        while let Some(&(node, next)) = stack.last() {
            match callees(node).get(next) {
                Some(&(callee, line)) => {
                    if let Some(top) = stack.last_mut() {
                        top.1 = next.saturating_add(1);
                    }
                    match color.get(callee) {
                        Some(Color::White) => {
                            if let Some(c) = color.get_mut(callee) {
                                *c = Color::Gray;
                            }
                            stack.push((callee, 0));
                        }
                        Some(Color::Gray) => {
                            let start = stack.iter().position(|&(f, _)| f == callee).unwrap_or(0);
                            let path: Vec<&str> = stack
                                .iter()
                                .skip(start)
                                .map(|&(f, _)| name(f))
                                .chain([name(callee)])
                                .collect();
                            return Err(error(
                                line,
                                format!("récursion interdite : {}", path.join(" → ")),
                            ));
                        }
                        _ => {}
                    }
                }
                None => {
                    // Toutes les fonctions appelées sont noires : la plus longue chaîne passe par la plus profonde.
                    let deepest = callees(node)
                        .iter()
                        .map(|&(callee, line)| {
                            (frames.get(callee).map_or(1, |f| f.0), callee, line)
                        })
                        .max_by_key(|&(depth, _, _)| depth);
                    if let Some(entry) = frames.get_mut(node) {
                        *entry = match deepest {
                            Some((depth, callee, line)) => {
                                (depth.saturating_add(1), Some((callee, line)))
                            }
                            None => (1, None),
                        };
                    }
                    if let Some(c) = color.get_mut(node) {
                        *c = Color::Black;
                    }
                    stack.pop();
                }
            }
        }
    }

    // La plus longue chaîne depuis `main`, coupée au premier cadre de trop.
    if frames.get(main).map_or(1, |f| f.0) > CALL_DEPTH {
        let mut chain = vec![name(main)];
        let mut at = main;
        let mut line = 0;
        while chain.len() <= CALL_DEPTH {
            let Some(&(_, Some((callee, call_line)))) = frames.get(at) else {
                break;
            };
            chain.push(name(callee));
            (at, line) = (callee, call_line);
        }
        return Err(error(
            line,
            format!(
                "appels trop profonds : {} demande plus de {CALL_DEPTH} cadres d'appel (B.1)",
                chain.join(" → ")
            ),
        ));
    }
    Ok(())
}
