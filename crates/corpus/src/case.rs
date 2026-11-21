//! Un cas du corpus : `case.toml`, `program.pbc` et `expected.json` (`corpus/FORMAT.md`), lus et vérifiés.
//!
//! Une clé inconnue est refusée partout : un `budjet` mal écrit ne doit pas devenir en silence un budget de 1000.

use std::fs;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};

use pbc::{Fault, SPEC_VERSION};
use serde::Deserialize;
use serde_json::json;
use vm::{ActKind, ActRecord, Action, MEM_SIZE, Memory, Report};

use crate::toml::{self, Table, TomlError, Value};

/// Le budget d'une exécution qui n'en donne pas (`FORMAT.md`).
const DEFAULT_BUDGET: u16 = 1000;

/// Une entrée de `answers` : la réponse au prochain rappel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Sense(i64),
    SenseFault(Fault),
    Act(bool),
    Rand(i64),
}

/// Un rappel posé par la machine, avec la réponse qu'il a reçue : une entrée du transcript (K.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call {
    Sense { k: u8, answer: Result<i64, Fault> },
    Act { action: Action, ok: bool },
    Rand { value: i64 },
}

impl Call {
    /// Forme exacte d'une entrée `calls` de expected.json.
    pub fn to_json(&self) -> serde_json::Value {
        match *self {
            Call::Sense {
                k,
                answer: Ok(value),
            } => json!({"call": "SENSE", "k": k, "value": value}),
            Call::Sense {
                k,
                answer: Err(fault),
            } => json!({"call": "SENSE", "k": k, "fault": fault.as_str()}),
            Call::Act { action, ok } => {
                json!({"call": "ACT", "kind": action.kind.name(), "arg": action.arg, "ok": ok})
            }
            Call::Rand { value } => json!({"call": "RAND", "value": value}),
        }
    }
}

/// Une exécution : son entrée (`[[exec]]`) et ce qu'elle doit donner (`expected.json`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExecCase {
    /// 1000 si `case.toml` n'en donne pas.
    pub budget: NonZeroU16,
    /// Rendue dense : les cellules absentes valent 0.
    pub mem: Memory,
    pub answers: Vec<Answer>,
    /// Le transcript attendu.
    pub calls: Vec<Call>,
    /// Le `RESULT` attendu.
    pub expected: Report,
}

/// Ce qu'un cas attend : un refus de chargement, ou une liste d'exécutions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expectation {
    Refused { fault: String },
    Runs(Vec<ExecCase>),
}

/// Un cas lu, nommé comme son répertoire.
#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub name: String,
    pub description: String,
    pub category: String,
    pub clause: String,
    pub program: Vec<u8>,
    pub expectation: Expectation,
}

#[derive(Debug, thiserror::Error)]
pub enum CaseError {
    #[error("{path} : {source}", path = path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} : {source}", path = path.display())]
    Toml { path: PathBuf, source: TomlError },
    #[error("{path} : {source}", path = path.display())]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("{path} : {message}", path = path.display())]
    Invalid { path: PathBuf, message: String },
    #[error("versions incomparables : machine {ours}, corpus {theirs} (H.1)")]
    VersionMismatch { ours: String, theirs: String },
}

/// Vrai si MAJEUR.MINEUR sont égaux (H.1).
pub fn comparable(ours: &str, theirs: &str) -> bool {
    match (major_minor(ours), major_minor(theirs)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// `MAJEUR.MINEUR.CORRECTIF`, trois nombres ; on garde les deux premiers.
fn major_minor(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
    let (major, minor, _patch) = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some((major, minor))
}

/// Lit le cas du répertoire `dir`.
pub fn load_case(dir: &Path) -> Result<Case, CaseError> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let toml_path = dir.join("case.toml");
    let source = read_to_string(&toml_path)?;
    let doc = toml::parse(&source).map_err(|source| CaseError::Toml {
        path: toml_path.clone(),
        source,
    })?;

    let mut root = Fields::new(&toml_path, "case.toml", doc.root);
    let description = root.string("description")?;
    let category = root.string("category")?;
    let clause = root.string("clause")?;
    let load_error = root.optional_string("load_error")?;
    root.finish()?;

    let program_path = dir.join("program.pbc");
    let program = fs::read(&program_path).map_err(|source| CaseError::Io {
        path: program_path,
        source,
    })?;

    let expectation = match load_error {
        Some(fault) => {
            if !doc.arrays.is_empty() {
                return Err(invalid(
                    &toml_path,
                    "un cas `load_error` n'a pas d'exécution",
                ));
            }
            Expectation::Refused { fault }
        }
        None => Expectation::Runs(executions(dir, &toml_path, doc.arrays)?),
    };

    Ok(Case {
        name,
        description,
        category,
        clause,
        program,
        expectation,
    })
}

/// `root/edge/*` puis `root/nominal/*`, triés par nom de répertoire.
pub fn load_corpus(root: &Path) -> Result<Vec<Case>, CaseError> {
    let mut cases = Vec::new();
    for kind in ["edge", "nominal"] {
        let dir = root.join(kind);
        if !dir.is_dir() {
            continue;
        }
        let entries = fs::read_dir(&dir).map_err(|source| CaseError::Io {
            path: dir.clone(),
            source,
        })?;
        let mut dirs = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|source| CaseError::Io {
                    path: dir.clone(),
                    source,
                })?
                .path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
        dirs.sort();
        for case_dir in dirs {
            cases.push(load_case(&case_dir)?);
        }
    }
    if cases.is_empty() {
        return Err(invalid(root, "aucun cas dans `edge/` ni dans `nominal/`"));
    }
    Ok(cases)
}

/// Les `[[exec]]` de `case.toml`, appariées dans l'ordre avec les `executions` de expected.json.
fn executions(
    dir: &Path,
    toml_path: &Path,
    arrays: Vec<(String, Table)>,
) -> Result<Vec<ExecCase>, CaseError> {
    let json_path = dir.join("expected.json");
    let text = read_to_string(&json_path)?;
    let json_error = |source| CaseError::Json {
        path: json_path.clone(),
        source,
    };

    // La version d'abord, avant de lire le reste : un fichier v2 peut avoir une autre forme (H.1).
    let version: RawVersion = serde_json::from_str(&text).map_err(json_error)?;
    if !comparable(SPEC_VERSION, &version.spec_version) {
        return Err(CaseError::VersionMismatch {
            ours: SPEC_VERSION.to_string(),
            theirs: version.spec_version,
        });
    }
    let expected: RawExpected = serde_json::from_str(&text).map_err(json_error)?;

    if expected.executions.len() != arrays.len() {
        return Err(invalid(
            &json_path,
            format!(
                "{} exécutions, mais {} `[[exec]]` dans case.toml",
                expected.executions.len(),
                arrays.len()
            ),
        ));
    }

    arrays
        .into_iter()
        .zip(expected.executions)
        .map(|((header, table), raw)| {
            if header != "exec" {
                return Err(invalid(
                    toml_path,
                    format!("en-tête `[[{header}]]` inconnu"),
                ));
            }
            let mut fields = Fields::new(toml_path, "[[exec]]", table);
            let budget = fields.budget()?;
            let mem = fields.mem()?;
            let answers = fields.answers()?;
            fields.finish()?;
            let calls = raw
                .calls
                .into_iter()
                .map(|call| call.convert(&json_path))
                .collect::<Result<_, _>>()?;
            let expected = raw.result.convert(&json_path)?;
            Ok(ExecCase {
                budget,
                mem,
                answers,
                calls,
                expected,
            })
        })
        .collect()
}

/// Les clés d'une table de `case.toml`, retirées une à une ; celles qui restent à la fin sont inconnues.
struct Fields<'a> {
    path: &'a Path,
    what: &'static str,
    entries: Table,
}

impl<'a> Fields<'a> {
    fn new(path: &'a Path, what: &'static str, entries: Table) -> Fields<'a> {
        Fields {
            path,
            what,
            entries,
        }
    }

    fn take(&mut self, key: &str) -> Option<Value> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(index).1)
    }

    fn wrong_type(&self, key: &str, wanted: &str) -> CaseError {
        invalid(
            self.path,
            format!("{} : `{key}` doit être {wanted}", self.what),
        )
    }

    fn string(&mut self, key: &str) -> Result<String, CaseError> {
        self.optional_string(key)?
            .ok_or_else(|| invalid(self.path, format!("{} : `{key}` manquant", self.what)))
    }

    fn optional_string(&mut self, key: &str) -> Result<Option<String>, CaseError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::Str(text)) => Ok(Some(text)),
            Some(_) => Err(self.wrong_type(key, "une chaîne")),
        }
    }

    /// `budget`, de 1 à 65535 ; 1000 s'il est absent.
    fn budget(&mut self) -> Result<NonZeroU16, CaseError> {
        let gas = match self.take("budget") {
            None => DEFAULT_BUDGET,
            Some(Value::Int(n)) => {
                u16::try_from(n).map_err(|_| self.wrong_type("budget", "entre 1 et 65535"))?
            }
            Some(_) => return Err(self.wrong_type("budget", "un entier")),
        };
        NonZeroU16::new(gas).ok_or_else(|| self.wrong_type("budget", "entre 1 et 65535"))
    }

    /// `mem = { cellule = valeur }`, rendue dense sur 256 cellules (B.1).
    fn mem(&mut self) -> Result<Memory, CaseError> {
        let mut mem = [0; MEM_SIZE];
        let cells = match self.take("mem") {
            None => return Ok(mem),
            Some(Value::Table(cells)) => cells,
            Some(_) => return Err(self.wrong_type("mem", "une table en ligne")),
        };
        for (key, value) in cells {
            let cell = key
                .parse::<usize>()
                .ok()
                .and_then(|index| mem.get_mut(index));
            match (cell, value) {
                (Some(cell), Value::Int(v)) => *cell = v,
                (None, _) => {
                    return Err(self.wrong_type("mem", "indexée par des cellules de 0 à 255"));
                }
                (Some(_), _) => return Err(self.wrong_type("mem", "faite d'entiers")),
            }
        }
        Ok(mem)
    }

    /// `answers = [{ sense = v }, …]` : chaque réponse porte exactement une clé.
    fn answers(&mut self) -> Result<Vec<Answer>, CaseError> {
        let items = match self.take("answers") {
            None => return Ok(Vec::new()),
            Some(Value::Array(items)) => items,
            Some(_) => return Err(self.wrong_type("answers", "un tableau")),
        };
        items
            .into_iter()
            .map(|item| {
                let answer = match &item {
                    Value::Table(table) => match table.as_slice() {
                        [(key, value)] => match (key.as_str(), value) {
                            ("sense", Value::Int(v)) => Some(Answer::Sense(*v)),
                            ("sense_fault", Value::Str(name)) => Fault::from_name(name).map(Answer::SenseFault),
                            ("act", Value::Bool(ok)) => Some(Answer::Act(*ok)),
                            ("rand", Value::Int(v)) => Some(Answer::Rand(*v)),
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                };
                answer.ok_or_else(|| {
                    self.wrong_type(
                        "answers",
                        "fait de tables à une clé : `sense`, `sense_fault` (une faute de B.5), `act` ou `rand`",
                    )
                })
            })
            .collect()
    }

    fn finish(self) -> Result<(), CaseError> {
        match self.entries.first() {
            None => Ok(()),
            Some((key, _)) => Err(invalid(
                self.path,
                format!("{} : clé `{key}` inconnue", self.what),
            )),
        }
    }
}

#[derive(Deserialize)]
struct RawVersion {
    spec_version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExpected {
    #[allow(dead_code)] // déjà lue par `RawVersion`
    spec_version: String,
    executions: Vec<RawExecution>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExecution {
    calls: Vec<RawCall>,
    result: RawResult,
}

/// Une entrée de `calls` : les champs présents dépendent de `call`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCall {
    call: String,
    k: Option<u8>,
    value: Option<i64>,
    fault: Option<String>,
    kind: Option<String>,
    arg: Option<i64>,
    ok: Option<bool>,
}

impl RawCall {
    fn convert(self, path: &Path) -> Result<Call, CaseError> {
        let call = match (
            self.call.as_str(),
            self.k,
            self.value,
            self.fault,
            self.kind,
            self.arg,
            self.ok,
        ) {
            ("SENSE", Some(k), Some(value), None, None, None, None) => Some(Call::Sense {
                k,
                answer: Ok(value),
            }),
            ("SENSE", Some(k), None, Some(fault), None, None, None) => Fault::from_name(&fault)
                .map(|fault| Call::Sense {
                    k,
                    answer: Err(fault),
                }),
            ("ACT", None, None, None, Some(kind), Some(arg), Some(ok)) => ActKind::from_name(&kind)
                .map(|kind| Call::Act {
                    action: Action { kind, arg },
                    ok,
                }),
            ("RAND", None, Some(value), None, None, None, None) => Some(Call::Rand { value }),
            _ => None,
        };
        call.ok_or_else(|| {
            invalid(
                path,
                format!("rappel `{}` mal formé dans `calls`", self.call),
            )
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResult {
    mem: Vec<i64>,
    gas_used: u16,
    fault: Option<String>,
    act: Option<RawAct>,
    trace: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAct {
    kind: String,
    arg: i64,
    ok: bool,
}

impl RawResult {
    fn convert(self, path: &Path) -> Result<Report, CaseError> {
        let mem = Memory::try_from(self.mem).map_err(|mem| {
            invalid(
                path,
                format!("`mem` compte {} cellules au lieu de 256", mem.len()),
            )
        })?;
        let fault = match self.fault {
            None => None,
            Some(name) => Some(
                Fault::from_name(&name)
                    .ok_or_else(|| invalid(path, format!("faute `{name}` inconnue (B.5)")))?,
            ),
        };
        let act = match self.act {
            None => None,
            Some(RawAct { kind, arg, ok }) => {
                let kind = ActKind::from_name(&kind)
                    .ok_or_else(|| invalid(path, format!("action `{kind}` inconnue (E.4)")))?;
                Some(ActRecord {
                    action: Action { kind, arg },
                    ok,
                })
            }
        };
        Ok(Report {
            mem,
            gas_used: self.gas_used,
            fault,
            act,
            trace: self.trace,
        })
    }
}

fn read_to_string(path: &Path) -> Result<String, CaseError> {
    fs::read_to_string(path).map_err(|source| CaseError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn invalid(path: &Path, message: impl Into<String>) -> CaseError {
    CaseError::Invalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}
