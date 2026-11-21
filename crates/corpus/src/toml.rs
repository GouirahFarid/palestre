//! Le sous-ensemble de TOML utilisé par les `case.toml` (D-006) : clés nues, chaînes, entiers `i64`, booléens,
//! tables en ligne, tableaux (sur plusieurs lignes si besoin), en-têtes `[[nom]]` et commentaires. Tout le reste
//! (flottants, dates, tables `[nom]`, clés pointées) est refusé avec le numéro de la ligne.

use std::iter::Peekable;
use std::str::Chars;

/// Une valeur TOML du sous-ensemble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Int(i64),
    Bool(bool),
    Array(Vec<Value>),
    Table(Table),
}

/// Une table, dans l'ordre du fichier.
pub type Table = Vec<(String, Value)>;

/// Un fichier lu : les clés de tête, puis une table par en-tête `[[nom]]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    pub root: Table,
    /// Chaque `[[nom]]` ouvre une nouvelle table, dans l'ordre du fichier.
    pub arrays: Vec<(String, Table)>,
}

/// Une erreur de lecture, avec la ligne (comptée depuis 1) où elle a été vue.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ligne {line} : {message}")]
pub struct TomlError {
    pub line: usize,
    pub message: String,
}

/// Lit un document entier.
pub fn parse(source: &str) -> Result<Document, TomlError> {
    Parser {
        chars: source.chars().peekable(),
        line: 1,
    }
    .document()
}

/// Un curseur sur les caractères, qui compte les lignes.
struct Parser<'s> {
    chars: Peekable<Chars<'s>>,
    line: usize,
}

impl Parser<'_> {
    /// Une ligne logique est vide, un en-tête `[[nom]]`, ou `clé = valeur`, suivie de la fin de ligne.
    fn document(mut self) -> Result<Document, TomlError> {
        let mut doc = Document::default();
        loop {
            self.skip_blank();
            match self.peek() {
                None => return Ok(doc),
                Some('[') => {
                    let name = self.header()?;
                    doc.arrays.push((name, Table::new()));
                }
                Some(_) => {
                    let table = match doc.arrays.last_mut() {
                        Some((_, table)) => table,
                        None => &mut doc.root,
                    };
                    self.key_value(table)?;
                }
            }
            self.end_of_line()?;
        }
    }

    /// `[[nom]]`. Un seul crochet ouvre une table `[nom]`, hors du sous-ensemble.
    fn header(&mut self) -> Result<String, TomlError> {
        self.expect('[')?;
        if self.peek() != Some('[') {
            return self.error("seules les tables `[[nom]]` sont prises en charge");
        }
        self.bump();
        self.skip_spaces();
        let name = self.key()?;
        self.skip_spaces();
        self.expect(']')?;
        self.expect(']')?;
        Ok(name)
    }

    /// `clé = valeur`, ajoutée à `table`. Une clé déjà présente est refusée.
    fn key_value(&mut self, table: &mut Table) -> Result<(), TomlError> {
        let key = self.key()?;
        self.skip_spaces();
        self.expect('=')?;
        self.skip_spaces();
        let value = self.value()?;
        if table.iter().any(|(k, _)| *k == key) {
            return self.error(format!("clé `{key}` répétée"));
        }
        table.push((key, value));
        Ok(())
    }

    /// Une clé nue : `[A-Za-z0-9_-]+`.
    fn key(&mut self) -> Result<String, TomlError> {
        let mut key = String::new();
        while let Some(c) = self
            .peek()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        {
            key.push(c);
            self.bump();
        }
        if key.is_empty() {
            return self.error("clé attendue");
        }
        Ok(key)
    }

    fn value(&mut self) -> Result<Value, TomlError> {
        match self.peek() {
            Some('"') => self.string().map(Value::Str),
            Some('{') => self.inline_table().map(Value::Table),
            Some('[') => self.array().map(Value::Array),
            _ => self.scalar(),
        }
    }

    /// `"…"`, avec les échappements `\"`, `\\`, `\n` et `\t`. Une chaîne ne traverse pas la fin de ligne.
    fn string(&mut self) -> Result<String, TomlError> {
        self.expect('"')?;
        let mut text = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return self.error("chaîne non terminée"),
                Some('"') => {
                    self.bump();
                    return Ok(text);
                }
                Some('\\') => {
                    self.bump();
                    let escaped = match self.peek() {
                        Some('"') => '"',
                        Some('\\') => '\\',
                        Some('n') => '\n',
                        Some('t') => '\t',
                        _ => return self.error("échappement inconnu"),
                    };
                    self.bump();
                    text.push(escaped);
                }
                Some(c) => {
                    self.bump();
                    text.push(c);
                }
            }
        }
    }

    /// `{ clé = valeur, … }`, sur une seule ligne, sans virgule finale.
    fn inline_table(&mut self) -> Result<Table, TomlError> {
        self.expect('{')?;
        let mut table = Table::new();
        self.skip_spaces();
        if self.peek() == Some('}') {
            self.bump();
            return Ok(table);
        }
        loop {
            self.key_value(&mut table)?;
            self.skip_spaces();
            match self.bump() {
                Some(',') => self.skip_spaces(),
                Some('}') => return Ok(table),
                _ => return self.error("`,` ou `}` attendu"),
            }
        }
    }

    /// `[ valeur, … ]`. Retours à la ligne, commentaires et virgule finale sont permis entre les éléments.
    fn array(&mut self) -> Result<Vec<Value>, TomlError> {
        self.expect('[')?;
        let mut items = Vec::new();
        loop {
            self.skip_blank();
            if self.peek() == Some(']') {
                self.bump();
                return Ok(items);
            }
            items.push(self.value()?);
            self.skip_blank();
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(']') => {}
                _ => return self.error("`,` ou `]` attendu"),
            }
        }
    }

    /// `true`, `false` ou un entier signé. `str::parse` refuse ce qui dépasse `i64`, et accepte `i64::MIN`.
    fn scalar(&mut self) -> Result<Value, TomlError> {
        let mut word = String::new();
        while let Some(c) = self
            .peek()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | '_' | ':'))
        {
            word.push(c);
            self.bump();
        }
        match word.as_str() {
            "" => self.error("valeur attendue"),
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => match word.parse::<i64>() {
                Ok(n) => Ok(Value::Int(n)),
                Err(_) => self.error(format!("valeur non prise en charge : `{word}`")),
            },
        }
    }

    /// Après une valeur ou un en-tête : des blancs, un commentaire éventuel, puis la fin de ligne ou du fichier.
    fn end_of_line(&mut self) -> Result<(), TomlError> {
        self.skip_spaces();
        match self.peek() {
            None | Some('\n') | Some('#') => Ok(()),
            Some(c) => self.error(format!("fin de ligne attendue, trouvé `{c}`")),
        }
    }

    /// Blancs, retours à la ligne et commentaires.
    fn skip_blank(&mut self) {
        loop {
            match self.peek() {
                Some(' ' | '\t' | '\r' | '\n') => {
                    self.bump();
                }
                Some('#') => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.bump();
                    }
                }
                _ => return,
            }
        }
    }

    /// Blancs d'une même ligne.
    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\r')) {
            self.bump();
        }
    }

    fn expect(&mut self, wanted: char) -> Result<(), TomlError> {
        if self.peek() == Some(wanted) {
            self.bump();
            Ok(())
        } else {
            self.error(format!("`{wanted}` attendu"))
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next();
        if c == Some('\n') {
            self.line = self.line.saturating_add(1);
        }
        c
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T, TomlError> {
        Err(TomlError {
            line: self.line,
            message: message.into(),
        })
    }
}
