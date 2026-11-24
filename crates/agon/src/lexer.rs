//! Analyse lexicale (D-015) : le texte devient une suite de lexèmes, chacun avec sa ligne et sa colonne.

use std::fmt;
use std::iter::Peekable;
use std::num::IntErrorKind;
use std::str::Chars;

/// Un lexème : un mot-clé, un nom, un entier ou un symbole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Int(i64),
    Ident(String),
    Fn,
    Let,
    If,
    Else,
    While,
    Return,
    Const,
    Mem,
    Halt,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Assign,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    Amp,
    Pipe,
    Caret,
    Shl,
    /// `>>`, le décalage arithmétique (`SAR`).
    Sar,
    /// `>>>`, le décalage logique (`SHR`).
    Shr,
    Bang,
    Tilde,
}

/// Le lexème tel qu'il s'écrit dans le source, pour les messages d'erreur.
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Token::Int(n) => return write!(f, "{n}"),
            Token::Ident(name) => return f.write_str(name),
            Token::Fn => "fn",
            Token::Let => "let",
            Token::If => "if",
            Token::Else => "else",
            Token::While => "while",
            Token::Return => "return",
            Token::Const => "const",
            Token::Mem => "mem",
            Token::Halt => "halt",
            Token::LParen => "(",
            Token::RParen => ")",
            Token::LBrace => "{",
            Token::RBrace => "}",
            Token::LBracket => "[",
            Token::RBracket => "]",
            Token::Comma => ",",
            Token::Semi => ";",
            Token::Assign => "=",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Slash => "/",
            Token::Percent => "%",
            Token::EqEq => "==",
            Token::NotEq => "!=",
            Token::Lt => "<",
            Token::Gt => ">",
            Token::Le => "<=",
            Token::Ge => ">=",
            Token::Amp => "&",
            Token::Pipe => "|",
            Token::Caret => "^",
            Token::Shl => "<<",
            Token::Sar => ">>",
            Token::Shr => ">>>",
            Token::Bang => "!",
            Token::Tilde => "~",
        };
        f.write_str(text)
    }
}

/// Un lexème et la position de son premier caractère, ligne et colonne à partir de 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned {
    pub token: Token,
    pub line: u32,
    pub col: u32,
}

/// Une erreur de lexique ou de syntaxe, à une position du source. Les colonnes comptent des caractères, pas des
/// octets.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{line}:{col} : {message}")]
pub struct SyntaxError {
    pub line: u32,
    pub col: u32,
    pub message: String,
}

/// Découpe `source` en lexèmes. Les blancs et les commentaires `//` sont sautés.
pub fn lex(source: &str) -> Result<Vec<Spanned>, SyntaxError> {
    let mut lexer = Lexer {
        chars: source.chars().peekable(),
        line: 1,
        col: 1,
    };
    let mut tokens = Vec::new();
    while let Some(spanned) = lexer.next_token()? {
        tokens.push(spanned);
    }
    Ok(tokens)
}

struct Lexer<'s> {
    chars: Peekable<Chars<'s>>,
    /// La position du prochain caractère.
    line: u32,
    col: u32,
}

impl Lexer<'_> {
    fn next_token(&mut self) -> Result<Option<Spanned>, SyntaxError> {
        self.skip_blanks();
        let (line, col) = (self.line, self.col);
        let error = |message: String| SyntaxError { line, col, message };
        let Some(c) = self.bump() else {
            return Ok(None);
        };
        let token = match c {
            '0'..='9' => self.number(c).map_err(error)?,
            'a'..='z' | 'A'..='Z' | '_' => self.word(c),
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            ',' => Token::Comma,
            ';' => Token::Semi,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '%' => Token::Percent,
            '^' => Token::Caret,
            '~' => Token::Tilde,
            // Du plus long au plus court : `>>>` avant `>>`, avant `>=`, avant `>`.
            '>' if self.eat('>') => {
                if self.eat('>') {
                    Token::Shr
                } else {
                    Token::Sar
                }
            }
            '>' if self.eat('=') => Token::Ge,
            '>' => Token::Gt,
            '<' if self.eat('<') => Token::Shl,
            '<' if self.eat('=') => Token::Le,
            '<' => Token::Lt,
            '=' if self.eat('=') => Token::EqEq,
            '=' => Token::Assign,
            '!' if self.eat('=') => Token::NotEq,
            '!' => Token::Bang,
            // Pas de court-circuit en agon (D-015) : on le dit plutôt que de laisser le parseur buter sur un `&` seul.
            '&' | '|' if self.chars.peek() == Some(&c) => {
                return Err(error(format!(
                    "« {c}{c} » n'existe pas en agon : sur des booléens 0 ou 1, « {c} » suffit"
                )));
            }
            '&' => Token::Amp,
            '|' => Token::Pipe,
            other => return Err(error(format!("caractère inattendu « {other} »"))),
        };
        Ok(Some(Spanned { token, line, col }))
    }

    /// Avance d'un caractère, en tenant la ligne et la colonne.
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.line = self.line.saturating_add(1);
            self.col = 1;
        } else {
            self.col = self.col.saturating_add(1);
        }
        Some(c)
    }

    /// Consomme `c` si c'est le prochain caractère.
    fn eat(&mut self, c: char) -> bool {
        let next = self.chars.peek() == Some(&c);
        if next {
            self.bump();
        }
        next
    }

    /// Le caractère d'après le prochain, sans rien consommer.
    fn second(&self) -> Option<char> {
        let mut ahead = self.chars.clone();
        ahead.next();
        ahead.next()
    }

    /// Saute les blancs et les commentaires `//`, jusqu'à la fin de leur ligne.
    fn skip_blanks(&mut self) {
        loop {
            match self.chars.peek().copied() {
                Some(c) if c.is_ascii_whitespace() => {
                    self.bump();
                }
                Some('/') if self.second() == Some('/') => {
                    while self.bump().is_some_and(|c| c != '\n') {}
                }
                _ => return,
            }
        }
    }

    /// Les caractères d'un mot qui suivent `first` : lettres, chiffres et `_`.
    fn rest_of_word(&mut self, first: Option<char>) -> String {
        let mut word = String::from_iter(first);
        while let Some(&c) = self.chars.peek() {
            if !(c.is_ascii_alphanumeric() || c == '_') {
                break;
            }
            word.push(c);
            self.bump();
        }
        word
    }

    /// Un entier décimal ou `0x…`, qui doit tenir dans un `i64` positif. Un nombre collé à des lettres (`12ab`)
    /// est mal formé plutôt que coupé en deux lexèmes.
    fn number(&mut self, first: char) -> Result<Token, String> {
        let hex = first == '0' && self.eat('x');
        let (digits, radix, text) = if hex {
            let digits = self.rest_of_word(None);
            let text = format!("0x{digits}");
            (digits, 16, text)
        } else {
            let digits = self.rest_of_word(Some(first));
            (digits.clone(), 10, digits)
        };
        if digits.is_empty() {
            return Err(format!("nombre sans chiffre « {text} »"));
        }
        i64::from_str_radix(&digits, radix)
            .map(Token::Int)
            .map_err(|error| match error.kind() {
                IntErrorKind::PosOverflow => {
                    format!("nombre trop grand « {text} » : au plus {}", i64::MAX)
                }
                _ => format!("nombre mal formé « {text} »"),
            })
    }

    /// Un mot-clé ou un nom.
    fn word(&mut self, first: char) -> Token {
        let word = self.rest_of_word(Some(first));
        match word.as_str() {
            "fn" => Token::Fn,
            "let" => Token::Let,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "return" => Token::Return,
            "const" => Token::Const,
            "mem" => Token::Mem,
            "halt" => Token::Halt,
            _ => Token::Ident(word),
        }
    }
}
