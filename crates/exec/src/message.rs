//! Les charges utiles JSON de K.3, et leur conversion vers les types de la VM.
//!
//! Choix de D-007 : on écrit du JSON compact, avec les clés dans l'ordre de K.3 (l'ordre des champs de chaque
//! `struct`), et on ignore à la lecture les champs inconnus.

use pbc::Fault;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use vm::{ActKind, ActRecord, Action, Memory, Report};

/// `OPEN`, du client : la version du protocole et un nom d'affichage optionnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Open {
    pub proto: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
}

/// `READY`, du serveur : une seule version de spécification annoncée, jamais une liste (K.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ready {
    pub proto: u32,
    pub spec: String,
    pub spec_version: String,
    pub sessions_max: u32,
}

/// `SESSION`, du serveur : l'identifiant opaque de K.4, et deux sommes de contrôle du programme chargé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub session: String,
    pub pbc_sha256: String,
    pub code_len: u32,
}

/// `EXEC`, du client. `budget` est lu en `i64` pour qu'un budget hors de `1..=65535` donne `bad_budget`, et non
/// une erreur de JSON (D-007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exec {
    pub session: String,
    pub mem: Vec<i64>,
    pub budget: i64,
}

/// `SENSE`, du serveur : le numéro du capteur demandé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SenseReq {
    pub k: u8,
}

/// `SENSED`, du client : `value` ou `fault`, jamais les deux.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sensed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fault: Option<String>,
}

/// `ACT`, du serveur : l'action (`"MOVE"`, `"TAKE"`, `"DROP"` ou `"EAT"`) et son argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActReq {
    pub kind: String,
    pub arg: i64,
}

/// `ACTED`, du client : l'issue de l'action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acted {
    pub ok: bool,
}

/// `RAND`, du serveur : toujours `{}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandReq {}

/// `DREW`, du client : la valeur tirée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drew {
    pub value: i64,
}

/// `RESULT`, du serveur. `fault` et `act` s'écrivent `null` quand ils sont absents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultMsg {
    pub mem: Vec<i64>,
    pub gas_used: u16,
    pub fault: Option<String>,
    pub act: Option<ActJson>,
    pub trace: Vec<i64>,
}

/// `RESULT.act` : `{kind, arg, ok}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActJson {
    pub kind: String,
    pub arg: i64,
    pub ok: bool,
}

/// `ERROR`, du serveur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorMsg {
    pub code: ErrorCode,
    pub message: String,
}

/// Les codes de K.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadFrame,
    BadProto,
    BadProgram,
    TooManySessions,
    NoSuchSession,
    BadMemory,
    BadBudget,
    UnexpectedMessage,
}

impl ErrorCode {
    /// K.8 : seuls `too_many_sessions` et `no_such_session` laissent la connexion ouverte.
    pub fn closes_connection(self) -> bool {
        !matches!(self, ErrorCode::TooManySessions | ErrorCode::NoSuchSession)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("JSON invalide : {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

/// JSON compact, clés dans l'ordre de déclaration.
pub fn to_json<T: Serialize>(msg: &T) -> Result<Vec<u8>, ProtocolError> {
    Ok(serde_json::to_vec(msg)?)
}

/// Lit une charge utile. Les champs inconnus sont ignorés (D-007).
pub fn from_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ProtocolError> {
    Ok(serde_json::from_slice(bytes)?)
}

/// K.4 : 1 à 64 caractères pris dans `[A-Za-z0-9_-]`. Ce jeu étant ASCII, compter les octets revient à compter les
/// caractères.
pub fn valid_session_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

impl Sensed {
    pub fn from_answer(answer: Result<i64, Fault>) -> Sensed {
        match answer {
            Ok(value) => Sensed {
                value: Some(value),
                fault: None,
            },
            Err(fault) => Sensed {
                value: None,
                fault: Some(fault.as_str().to_string()),
            },
        }
    }

    /// Exactement un des deux champs. Une faute inconnue, ou de chargement comme `BadHeader`, est refusée : un
    /// programme déjà chargé ne peut plus lever ce genre de faute (K.3).
    pub fn to_answer(&self) -> Result<Result<i64, Fault>, ProtocolError> {
        match (self.value, &self.fault) {
            (Some(value), None) => Ok(Ok(value)),
            (None, Some(name)) => fault_from_name(name).map(Err),
            _ => Err(ProtocolError::Invalid(
                "`SENSED` doit porter `value` ou `fault`, exactement un des deux".to_string(),
            )),
        }
    }
}

impl ActReq {
    pub fn from_action(action: Action) -> ActReq {
        ActReq {
            kind: action.kind.name().to_string(),
            arg: action.arg,
        }
    }

    pub fn to_action(&self) -> Result<Action, ProtocolError> {
        Ok(Action {
            kind: kind_from_name(&self.kind)?,
            arg: self.arg,
        })
    }
}

impl ResultMsg {
    pub fn from_report(report: &Report) -> ResultMsg {
        ResultMsg {
            mem: report.mem.to_vec(),
            gas_used: report.gas_used,
            fault: report.fault.map(|fault| fault.as_str().to_string()),
            act: report.act.map(|record| ActJson {
                kind: record.action.kind.name().to_string(),
                arg: record.action.arg,
                ok: record.ok,
            }),
            trace: report.trace.clone(),
        }
    }

    /// L'inverse de `from_report` : 256 cellules exactement, des noms de faute et d'action connus.
    pub fn to_report(&self) -> Result<Report, ProtocolError> {
        let mem = Memory::try_from(self.mem.as_slice()).map_err(|_| {
            ProtocolError::Invalid(format!(
                "`mem` a {} valeurs au lieu de {}",
                self.mem.len(),
                vm::MEM_SIZE
            ))
        })?;
        let fault = self.fault.as_deref().map(fault_from_name).transpose()?;
        let act = self
            .act
            .as_ref()
            .map(|act| -> Result<ActRecord, ProtocolError> {
                Ok(ActRecord {
                    action: Action {
                        kind: kind_from_name(&act.kind)?,
                        arg: act.arg,
                    },
                    ok: act.ok,
                })
            })
            .transpose()?;
        Ok(Report {
            mem,
            gas_used: self.gas_used,
            fault,
            act,
            trace: self.trace.clone(),
        })
    }
}

fn fault_from_name(name: &str) -> Result<Fault, ProtocolError> {
    Fault::from_name(name)
        .ok_or_else(|| ProtocolError::Invalid(format!("faute d'exécution inconnue : `{name}`")))
}

fn kind_from_name(name: &str) -> Result<ActKind, ProtocolError> {
    ActKind::from_name(name)
        .ok_or_else(|| ProtocolError::Invalid(format!("action inconnue : `{name}`")))
}
