//! Assemblage en deux passes (D-009) : la première fixe l'adresse de chaque ligne et la table des étiquettes, la
//! seconde résout les opérandes et encode.

use std::collections::HashMap;

use pbc::{LoadError, Op, Operand, Program};

/// Une erreur d'assemblage, avec sa ligne dans le source (à partir de 1). La ligne 0 désigne le programme entier :
/// code vide, plus de 4095 constantes…
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ligne {line} : {kind}")]
pub struct AsmError {
    pub line: usize,
    pub kind: AsmErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AsmErrorKind {
    #[error("mnémonique inconnu « {0} »")]
    UnknownMnemonic(String),
    #[error("{0} attend un opérande")]
    MissingOperand(&'static str),
    #[error("opérande inattendu")]
    UnexpectedOperand,
    #[error("nombre invalide « {0} »")]
    BadNumber(String),
    #[error("{value} hors de {min}..={max}")]
    OutOfRange { value: i64, min: i64, max: i64 },
    #[error("étiquette inconnue « {0} »")]
    UnknownLabel(String),
    #[error("étiquette « {0} » définie deux fois")]
    DuplicateLabel(String),
    #[error("directive invalide « {0} »")]
    BadDirective(String),
    #[error(transparent)]
    Program(#[from] LoadError),
}

/// Un opérande tel qu'il est écrit, avant la résolution des étiquettes.
enum Arg<'s> {
    Number(i64),
    /// Étiquette nue : déplacement depuis le premier octet de l'instruction suivante (B.4).
    Relative(&'s str),
    /// `@étiquette` : l'adresse de l'étiquette.
    Absolute(&'s str),
}

/// Ce qu'une ligne ajoute au code. Après la passe 1, une instruction porte un opérande si et seulement si son
/// opcode en attend un.
enum Item<'s> {
    Instr(Op, Option<Arg<'s>>),
    Bytes(Vec<u8>),
}

struct Line<'s> {
    number: usize,
    pc: usize,
    item: Item<'s>,
}

/// Assemble un source (syntaxe de D-009) en programme. Les bornes du format (code de 1 à 65 535 octets, au plus
/// 4095 constantes) sont celles de `Program::new`.
pub fn assemble(source: &str) -> Result<Program, AsmError> {
    let mut constants = Vec::new();
    let mut labels: HashMap<&str, usize> = HashMap::new();
    let mut lines = Vec::new();
    let mut pc = 0;

    // Passe 1 : l'adresse de chaque ligne, les étiquettes et les constantes.
    for (index, text) in source.lines().enumerate() {
        let number = index + 1;
        let at = |kind| AsmError { line: number, kind };
        let mut words = text
            .split_once(';')
            .map_or(text, |(code, _)| code)
            .split_whitespace()
            .peekable();

        while let Some(name) = words.peek().and_then(|word| label_definition(word)) {
            if labels.insert(name, pc).is_some() {
                return Err(at(AsmErrorKind::DuplicateLabel(name.to_string())));
            }
            words.next();
        }
        let Some(head) = words.next() else { continue };
        let rest: Vec<&str> = words.collect();

        if head.eq_ignore_ascii_case(".const") {
            if rest.is_empty() {
                return Err(at(AsmErrorKind::MissingOperand(".const")));
            }
            for word in rest {
                constants.push(number_value(word).map_err(at)?);
            }
        } else if head.eq_ignore_ascii_case(".byte") {
            if rest.is_empty() {
                return Err(at(AsmErrorKind::MissingOperand(".byte")));
            }
            let bytes = rest
                .into_iter()
                .map(|word| fit(number_value(word)?, u8::MIN, u8::MAX))
                .collect::<Result<Vec<u8>, _>>()
                .map_err(at)?;
            let len = bytes.len();
            lines.push(Line {
                number,
                pc,
                item: Item::Bytes(bytes),
            });
            pc += len;
        } else if head.starts_with('.') {
            return Err(at(AsmErrorKind::BadDirective(head.to_string())));
        } else {
            let op = Op::from_mnemonic(head)
                .ok_or_else(|| at(AsmErrorKind::UnknownMnemonic(head.to_string())))?;
            let arg = match (op.operand(), rest.as_slice()) {
                (Operand::None, []) => None,
                (Operand::None, _) | (_, [_, _, ..]) => {
                    return Err(at(AsmErrorKind::UnexpectedOperand));
                }
                (_, []) => return Err(at(AsmErrorKind::MissingOperand(op.mnemonic()))),
                (_, [word]) => Some(arg(word).map_err(at)?),
            };
            lines.push(Line {
                number,
                pc,
                item: Item::Instr(op, arg),
            });
            pc += op.size();
        }
    }

    // Passe 2 : toutes les adresses sont connues, on résout les opérandes et on encode.
    let mut code = Vec::with_capacity(pc);
    for Line { number, pc, item } in lines {
        let at = |kind| AsmError { line: number, kind };
        match item {
            Item::Bytes(bytes) => code.extend_from_slice(&bytes),
            Item::Instr(op, arg) => {
                code.push(op.byte());
                if let Some(arg) = arg {
                    let value = resolve(arg, &labels, pc + op.size()).map_err(at)?;
                    encode_operand(op.operand(), value, &mut code).map_err(at)?;
                }
            }
        }
    }

    Program::new(constants, code).map_err(|error| AsmError {
        line: 0,
        kind: error.into(),
    })
}

/// `nom:` définit une étiquette si `nom` est un identifiant : une lettre ou `_`, puis des lettres, chiffres ou `_`.
/// Une étiquette ne peut donc pas commencer comme un nombre.
fn label_definition(word: &str) -> Option<&str> {
    let name = word.strip_suffix(':')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    let valid =
        (first.is_alphabetic() || first == '_') && chars.all(|c| c.is_alphanumeric() || c == '_');
    valid.then_some(name)
}

/// Un opérande qui commence par `@` est une adresse absolue, par un chiffre ou un signe un nombre, et sinon une
/// étiquette relative.
fn arg(word: &str) -> Result<Arg<'_>, AsmErrorKind> {
    if let Some(name) = word.strip_prefix('@') {
        return Ok(Arg::Absolute(name));
    }
    match word.chars().next() {
        Some(c) if c.is_ascii_digit() || c == '-' || c == '+' => {
            number_value(word).map(Arg::Number)
        }
        _ => Ok(Arg::Relative(word)),
    }
}

/// Un entier signé, décimal (`-3`, `+4`) ou hexadécimal (`0x7FFF`, `-0x10`), qui doit tenir sur un `i64`.
fn number_value(word: &str) -> Result<i64, AsmErrorKind> {
    let bad = || AsmErrorKind::BadNumber(word.to_string());
    let (negative, unsigned) = match word.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, word.strip_prefix('+').unwrap_or(word)),
    };
    let (radix, digits) = match unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        Some(hex) => (16, hex),
        None => (10, unsigned),
    };
    // `from_str_radix` accepterait un second signe (`-+5`) : on n'admet que des chiffres.
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return Err(bad());
    }
    let magnitude = i128::from(u64::from_str_radix(digits, radix).map_err(|_| bad())?);
    let value = if negative { -magnitude } else { magnitude };
    i64::try_from(value).map_err(|_| bad())
}

/// La valeur numérique d'un opérande. `next` est l'adresse de l'instruction suivante, origine des sauts relatifs.
fn resolve(arg: Arg<'_>, labels: &HashMap<&str, usize>, next: usize) -> Result<i64, AsmErrorKind> {
    let address = |name: &str| {
        labels
            .get(name)
            .map(|&target| signed(target))
            .ok_or_else(|| AsmErrorKind::UnknownLabel(name.to_string()))
    };
    match arg {
        Arg::Number(value) => Ok(value),
        Arg::Absolute(name) => address(name),
        // Deux adresses positives : la différence ne peut pas déborder.
        Arg::Relative(name) => Ok(address(name)? - signed(next)),
    }
}

/// Une adresse en `i64`. Elle tient toujours, le code étant borné bien avant ; sinon la valeur saturée sera de
/// toute façon refusée par la vérification de plage.
fn signed(address: usize) -> i64 {
    i64::try_from(address).unwrap_or(i64::MAX)
}

/// Ajoute l'opérande gros-boutiste à `code`, après avoir vérifié qu'il tient dans le type que la table lui donne.
fn encode_operand(operand: Operand, value: i64, code: &mut Vec<u8>) -> Result<(), AsmErrorKind> {
    match operand {
        Operand::None => {}
        Operand::U8 => code.push(fit(value, u8::MIN, u8::MAX)?),
        Operand::U16 => code.extend_from_slice(&fit(value, u16::MIN, u16::MAX)?.to_be_bytes()),
        Operand::I16 => code.extend_from_slice(&fit(value, i16::MIN, i16::MAX)?.to_be_bytes()),
    }
    Ok(())
}

/// `value` converti dans le type de `min` et `max`, ou `OutOfRange` avec ces bornes.
fn fit<T: TryFrom<i64> + Into<i64>>(value: i64, min: T, max: T) -> Result<T, AsmErrorKind> {
    T::try_from(value).map_err(|_| AsmErrorKind::OutOfRange {
        value,
        min: min.into(),
        max: max.into(),
    })
}
