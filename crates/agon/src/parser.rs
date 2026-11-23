//! Analyse syntaxique (D-015) : une descente récursive, une fonction par ligne de la grammaire.
//!
//! Les niveaux d'expression vont du plus faible au plus fort : comparaison (non associative), `|`, `^`, `&`, décalages,
//! `+ -`, `* / %`, unaires. C'est l'ordre de Rust : `x & 1 == 0` se lit `(x & 1) == 0`.

use std::fmt;

use crate::ast::{BinOp, Expr, Function, Item, Module, Stmt, UnOp};
use crate::lexer::{Spanned, SyntaxError, Token, lex};

/// La hauteur maximale de l'arbre. L'affichage, la libération et la génération de code (B4) parcourent l'arbre
/// récursivement : sans borne, un source comme `((((…` ferait déborder la pile au lieu de rendre une erreur.
const MAX_DEPTH: u32 = 64;

/// Analyse un fichier source entier.
pub fn parse(source: &str) -> Result<Module, SyntaxError> {
    let mut parser = Parser::new(source)?;
    let mut items = Vec::new();
    while parser.peek().is_some() {
        items.push(parser.item()?);
    }
    Ok(Module { items })
}

/// Analyse une expression seule, qui doit occuper tout le source.
pub fn parse_expr(source: &str) -> Result<Expr, SyntaxError> {
    let mut parser = Parser::new(source)?;
    let expr = parser.expr()?;
    if parser.peek().is_some() {
        return Err(parser.expected("la fin de l'expression"));
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
    /// La position juste après le dernier caractère du source : celle des erreurs en fin de texte.
    end: (u32, u32),
    /// La hauteur de l'arbre en cours de construction, bornée par `MAX_DEPTH`.
    depth: u32,
}

impl Parser {
    fn new(source: &str) -> Result<Parser, SyntaxError> {
        Ok(Parser {
            tokens: lex(source)?,
            pos: 0,
            end: end_of(source),
            depth: 0,
        })
    }

    // --- Les outils : regarder, avancer, exiger, signaler. ---

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|spanned| &spanned.token)
    }

    fn at(&self, token: &Token) -> bool {
        self.peek() == Some(token)
    }

    /// La ligne du lexème courant : celle du nœud qui commence ici.
    fn line(&self) -> u32 {
        self.tokens
            .get(self.pos)
            .map_or(self.end.0, |spanned| spanned.line)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self
            .tokens
            .get(self.pos)
            .map(|spanned| spanned.token.clone());
        if token.is_some() {
            self.pos = self.pos.saturating_add(1);
        }
        token
    }

    /// Consomme `token` s'il est le lexème courant.
    fn eat(&mut self, token: &Token) -> bool {
        let here = self.at(token);
        if here {
            self.advance();
        }
        here
    }

    fn expect(&mut self, token: Token) -> Result<(), SyntaxError> {
        if self.eat(&token) {
            Ok(())
        } else {
            Err(self.expected(format!("« {token} »")))
        }
    }

    /// Un nom (de variable, de fonction…), sinon une erreur qui dit lequel était attendu.
    fn ident(&mut self, what: &str) -> Result<String, SyntaxError> {
        match self.peek() {
            Some(Token::Ident(name)) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => Err(self.expected(what)),
        }
    }

    /// Une erreur sur le lexème courant, ou à la fin du texte s'il n'y en a plus.
    fn error(&self, message: String) -> SyntaxError {
        let (line, col) = self
            .tokens
            .get(self.pos)
            .map_or(self.end, |spanned| (spanned.line, spanned.col));
        SyntaxError { line, col, message }
    }

    /// « on attendait X, pas Y », où Y est le lexème courant.
    fn expected(&self, what: impl fmt::Display) -> SyntaxError {
        let found = match self.peek() {
            Some(token) => format!("« {token} »"),
            None => "la fin du texte".to_string(),
        };
        self.error(format!("on attendait {what}, pas {found}"))
    }

    /// Un niveau de plus dans l'arbre, ou une erreur au-delà de `MAX_DEPTH`.
    fn descend(&mut self) -> Result<(), SyntaxError> {
        if self.depth >= MAX_DEPTH {
            return Err(self.error(format!(
                "trop d'imbrication : l'arbre dépasserait {MAX_DEPTH} niveaux"
            )));
        }
        self.depth = self.depth.saturating_add(1);
        Ok(())
    }

    /// Analyse avec `f` un niveau plus bas dans l'arbre.
    fn nested<T>(
        &mut self,
        f: impl FnOnce(&mut Parser) -> Result<T, SyntaxError>,
    ) -> Result<T, SyntaxError> {
        let depth = self.depth;
        self.descend()?;
        let result = f(self);
        self.depth = depth;
        result
    }

    // --- Les items et les instructions. ---

    /// `item := "const" IDENT "=" expr ";" | "fn" IDENT "(" params ")" bloc`
    fn item(&mut self) -> Result<Item, SyntaxError> {
        let line = self.line();
        match self.peek() {
            Some(Token::Const) => {
                self.advance();
                let name = self.ident("un nom de constante")?;
                self.expect(Token::Assign)?;
                let value = self.expr()?;
                self.expect(Token::Semi)?;
                Ok(Item::Const { name, value, line })
            }
            Some(Token::Fn) => {
                self.advance();
                let name = self.ident("un nom de fonction")?;
                self.expect(Token::LParen)?;
                let mut params = Vec::new();
                if !self.at(&Token::RParen) {
                    params.push(self.ident("un nom de paramètre")?);
                    while self.eat(&Token::Comma) {
                        params.push(self.ident("un nom de paramètre")?);
                    }
                }
                self.expect(Token::RParen)?;
                let body = self.block()?;
                Ok(Item::Function(Function {
                    name,
                    params,
                    body,
                    line,
                }))
            }
            _ => Err(self.expected("« fn » ou « const »")),
        }
    }

    /// `bloc := "{" instr* "}"`
    fn block(&mut self) -> Result<Vec<Stmt>, SyntaxError> {
        self.expect(Token::LBrace)?;
        let body = self.nested(|parser| {
            let mut body = Vec::new();
            while parser.peek().is_some() && !parser.at(&Token::RBrace) {
                body.push(parser.statement()?);
            }
            Ok(body)
        })?;
        self.expect(Token::RBrace)?;
        Ok(body)
    }

    /// Une instruction : la forme se reconnaît à son premier lexème, sauf l'affectation (voir `assign_or_expr`).
    fn statement(&mut self) -> Result<Stmt, SyntaxError> {
        let line = self.line();
        match self.peek() {
            Some(Token::Let) => {
                self.advance();
                let name = self.ident("un nom de variable")?;
                self.expect(Token::Assign)?;
                let value = self.expr()?;
                self.expect(Token::Semi)?;
                Ok(Stmt::Let { name, value, line })
            }
            Some(Token::If) => self.if_statement(),
            Some(Token::While) => {
                self.advance();
                let cond = self.expr()?;
                let body = self.block()?;
                Ok(Stmt::While { cond, body, line })
            }
            Some(Token::Return) => {
                self.advance();
                let value = if self.at(&Token::Semi) {
                    None
                } else {
                    Some(self.expr()?)
                };
                self.expect(Token::Semi)?;
                Ok(Stmt::Return { value, line })
            }
            Some(Token::Halt) => {
                self.advance();
                self.expect(Token::Semi)?;
                Ok(Stmt::Halt { line })
            }
            _ => self.assign_or_expr(),
        }
    }

    /// `if expr bloc ("else" (bloc | if …))?` : `else if` est un `else` suivi d'un `if`.
    fn if_statement(&mut self) -> Result<Stmt, SyntaxError> {
        let line = self.line();
        self.expect(Token::If)?;
        let cond = self.expr()?;
        let then = self.block()?;
        let otherwise = if !self.eat(&Token::Else) {
            None
        } else if self.at(&Token::If) {
            Some(vec![self.nested(Parser::if_statement)?])
        } else {
            Some(self.block()?)
        };
        Ok(Stmt::If {
            cond,
            then,
            otherwise,
            line,
        })
    }

    /// `IDENT "=" expr ";"`, `"mem" "[" expr "]" "=" expr ";"` ou `expr ";"`. On lit d'abord une expression : si un
    /// `=` la suit, elle doit être une variable ou une cellule `mem[…]`, et devient la cible de l'affectation.
    fn assign_or_expr(&mut self) -> Result<Stmt, SyntaxError> {
        let line = self.line();
        let target = self.expr()?;
        if !self.at(&Token::Assign) {
            self.expect(Token::Semi)?;
            return Ok(Stmt::Expr { expr: target, line });
        }
        let stmt = match target {
            Expr::Var(name, _) => {
                self.advance();
                let value = self.expr()?;
                Stmt::Assign { name, value, line }
            }
            Expr::MemLoad { addr, .. } => {
                self.advance();
                let value = self.expr()?;
                Stmt::MemStore {
                    addr: *addr,
                    value,
                    line,
                }
            }
            _ => {
                return Err(self.error(
                    "on ne peut affecter qu'une variable ou une cellule « mem[…] »".to_string(),
                ));
            }
        };
        self.expect(Token::Semi)?;
        Ok(stmt)
    }

    // --- Les expressions, du niveau le plus faible au plus fort. ---

    fn expr(&mut self) -> Result<Expr, SyntaxError> {
        self.nested(Parser::comparison)
    }

    /// `comparaison := ou (op ou)?` : au plus une comparaison, `a < b < c` est une erreur.
    fn comparison(&mut self) -> Result<Expr, SyntaxError> {
        let lhs = self.or()?;
        let Some(op) = self.peek().and_then(comparison_op) else {
            return Ok(lhs);
        };
        let line = self.line();
        self.advance();
        let rhs = self.or()?;
        if self.peek().and_then(comparison_op).is_some() {
            return Err(self.error(
                "les comparaisons ne s'enchaînent pas : ajoutez des parenthèses".to_string(),
            ));
        }
        Ok(binary(op, lhs, rhs, line))
    }

    /// `ou := xou ("|" xou)*`
    fn or(&mut self) -> Result<Expr, SyntaxError> {
        self.left_assoc(Parser::xor, |token| match token {
            Token::Pipe => Some(BinOp::BitOr),
            _ => None,
        })
    }

    /// `xou := et ("^" et)*`
    fn xor(&mut self) -> Result<Expr, SyntaxError> {
        self.left_assoc(Parser::and, |token| match token {
            Token::Caret => Some(BinOp::BitXor),
            _ => None,
        })
    }

    /// `et := decalage ("&" decalage)*`
    fn and(&mut self) -> Result<Expr, SyntaxError> {
        self.left_assoc(Parser::shift, |token| match token {
            Token::Amp => Some(BinOp::BitAnd),
            _ => None,
        })
    }

    /// `decalage := somme (("<<" | ">>" | ">>>") somme)*`
    fn shift(&mut self) -> Result<Expr, SyntaxError> {
        self.left_assoc(Parser::sum, |token| match token {
            Token::Shl => Some(BinOp::Shl),
            Token::Sar => Some(BinOp::Sar),
            Token::Shr => Some(BinOp::Shr),
            _ => None,
        })
    }

    /// `somme := produit (("+" | "-") produit)*`
    fn sum(&mut self) -> Result<Expr, SyntaxError> {
        self.left_assoc(Parser::product, |token| match token {
            Token::Plus => Some(BinOp::Add),
            Token::Minus => Some(BinOp::Sub),
            _ => None,
        })
    }

    /// `produit := unaire (("*" | "/" | "%") unaire)*`
    fn product(&mut self) -> Result<Expr, SyntaxError> {
        self.left_assoc(Parser::unary, |token| match token {
            Token::Star => Some(BinOp::Mul),
            Token::Slash => Some(BinOp::Div),
            Token::Percent => Some(BinOp::Mod),
            _ => None,
        })
    }

    /// Un niveau associatif à gauche : `operand (op operand)*`, où `op_of` reconnaît les opérateurs du niveau.
    /// Chaque opérateur ajoute un étage à l'arbre (`1 - 2 - 3` est `(- (- 1 2) 3)`) : il compte dans la hauteur.
    fn left_assoc(
        &mut self,
        operand: fn(&mut Parser) -> Result<Expr, SyntaxError>,
        op_of: fn(&Token) -> Option<BinOp>,
    ) -> Result<Expr, SyntaxError> {
        let depth = self.depth;
        let mut lhs = operand(self)?;
        while let Some(op) = self.peek().and_then(op_of) {
            let line = self.line();
            self.descend()?;
            self.advance();
            let rhs = operand(self)?;
            lhs = binary(op, lhs, rhs, line);
        }
        self.depth = depth;
        Ok(lhs)
    }

    /// `unaire := ("-" | "!" | "~") unaire | primaire`
    fn unary(&mut self) -> Result<Expr, SyntaxError> {
        let op = match self.peek() {
            Some(Token::Minus) => UnOp::Neg,
            Some(Token::Bang) => UnOp::Not,
            Some(Token::Tilde) => UnOp::BitNot,
            _ => return self.primary(),
        };
        let line = self.line();
        self.advance();
        let expr = self.nested(Parser::unary)?;
        Ok(Expr::Unary {
            op,
            expr: Box::new(expr),
            line,
        })
    }

    /// `primaire := ENTIER | IDENT | IDENT "(" args ")" | "mem" "[" expr "]" | "(" expr ")"`
    fn primary(&mut self) -> Result<Expr, SyntaxError> {
        let line = self.line();
        match self.peek() {
            Some(&Token::Int(n)) => {
                self.advance();
                Ok(Expr::Int(n, line))
            }
            Some(Token::Ident(_)) => {
                let name = self.ident("un nom")?;
                if !self.eat(&Token::LParen) {
                    return Ok(Expr::Var(name, line));
                }
                let mut args = Vec::new();
                if !self.at(&Token::RParen) {
                    args.push(self.expr()?);
                    while self.eat(&Token::Comma) {
                        args.push(self.expr()?);
                    }
                }
                self.expect(Token::RParen)?;
                Ok(Expr::Call { name, args, line })
            }
            Some(Token::Mem) => {
                self.advance();
                self.expect(Token::LBracket)?;
                let addr = self.expr()?;
                self.expect(Token::RBracket)?;
                Ok(Expr::MemLoad {
                    addr: Box::new(addr),
                    line,
                })
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.expr()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            _ => Err(self.expected("une expression")),
        }
    }
}

fn comparison_op(token: &Token) -> Option<BinOp> {
    match token {
        Token::EqEq => Some(BinOp::Eq),
        Token::NotEq => Some(BinOp::Ne),
        Token::Lt => Some(BinOp::Lt),
        Token::Gt => Some(BinOp::Gt),
        Token::Le => Some(BinOp::Le),
        Token::Ge => Some(BinOp::Ge),
        _ => None,
    }
}

fn binary(op: BinOp, lhs: Expr, rhs: Expr, line: u32) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        line,
    }
}

/// La position qui suit le dernier caractère de `source`, comptée comme le fait le lexer.
fn end_of(source: &str) -> (u32, u32) {
    let line = source.matches('\n').count();
    let col = source
        .rsplit('\n')
        .next()
        .map_or(0, |last| last.chars().count());
    let position = |n: usize| u32::try_from(n).unwrap_or(u32::MAX).saturating_add(1);
    (position(line), position(col))
}
