//! Rejouer un cas contre une machine et signaler la première différence, dans l'ordre de `corpus/FORMAT.md`.

use std::fmt;
use std::net::SocketAddr;
use std::num::NonZeroU16;

use exec::client::{Client, ClientError};
use exec::message::ErrorCode;
use pbc::{Fault, Program};
use vm::{ActRecord, Action, Env, Machine, Memory, Report, run_with};

use crate::case::{Answer, Call, Case, Expectation};

/// Une machine à éprouver : la VM de ce dépôt (`InProcess`), ou un serveur `palestre exec` distant (T11).
pub trait Backend {
    /// `Ok(true)` si le chargeur accepte ces octets.
    fn accepts(&mut self, program: &[u8]) -> Result<bool, BackendError>;
    fn execute(
        &mut self,
        program: &[u8],
        mem: &Memory,
        budget: NonZeroU16,
        env: &mut dyn Env,
    ) -> Result<Report, BackendError>;
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("programme refusé au chargement")]
    Refused,
    #[error("transport : {0}")]
    Transport(String),
}

/// La VM de ce dépôt, en mémoire.
pub struct InProcess;

impl Backend for InProcess {
    fn accepts(&mut self, program: &[u8]) -> Result<bool, BackendError> {
        Ok(Program::load(program).is_ok())
    }

    fn execute(
        &mut self,
        program: &[u8],
        mem: &Memory,
        budget: NonZeroU16,
        env: &mut dyn Env,
    ) -> Result<Report, BackendError> {
        let program = Program::load(program).map_err(|_| BackendError::Refused)?;
        Ok(run_with(Machine::new(&program, *mem, budget), env))
    }
}

/// Un serveur `palestre exec`, le nôtre ou celui d'un autre groupe. Une connexion neuve par exécution : aucun cas ne
/// dépend du précédent, et un serveur tiers n'atteint jamais son `sessions_max`.
pub struct Remote {
    addr: SocketAddr,
}

impl Remote {
    pub fn new(addr: SocketAddr) -> Remote {
        Remote { addr }
    }
}

impl Backend for Remote {
    fn accepts(&mut self, program: &[u8]) -> Result<bool, BackendError> {
        match Client::connect(self.addr).and_then(|mut client| client.submit(program)) {
            Ok(_) => Ok(true),
            Err(error) => match refusal(error) {
                BackendError::Refused => Ok(false),
                other => Err(other),
            },
        }
    }

    fn execute(
        &mut self,
        program: &[u8],
        mem: &Memory,
        budget: NonZeroU16,
        env: &mut dyn Env,
    ) -> Result<Report, BackendError> {
        let mut client = Client::connect(self.addr).map_err(refusal)?;
        let session = client.submit(program).map_err(refusal)?;
        client
            .exec(&session.session, mem, budget, env)
            .map_err(refusal)
    }
}

/// `bad_program` est un refus de chargement ; toute autre erreur vient du transport ou du serveur.
fn refusal(error: ClientError) -> BackendError {
    match error {
        ClientError::Server(ref msg) if msg.code == ErrorCode::BadProgram => BackendError::Refused,
        other => BackendError::Transport(other.to_string()),
    }
}

/// Répond aux rappels avec les `answers` d'un cas et enregistre le transcript réel.
///
/// Chaque rappel consomme une réponse. Si elle n'est pas du bon type, ou si le script est épuisé, la réponse est
/// neutre (`0` ou `false`). Le rappel est enregistré avec la réponse donnée, si bien que deux transcripts sont égaux
/// exactement quand la machine a posé les mêmes questions.
pub struct Replay<'a> {
    answers: std::slice::Iter<'a, Answer>,
    transcript: Vec<Call>,
}

impl<'a> Replay<'a> {
    pub fn new(answers: &'a [Answer]) -> Replay<'a> {
        Replay {
            answers: answers.iter(),
            transcript: Vec::new(),
        }
    }

    pub fn transcript(&self) -> &[Call] {
        &self.transcript
    }
}

impl Env for Replay<'_> {
    fn sense(&mut self, k: u8) -> Result<i64, Fault> {
        let answer = match self.answers.next() {
            Some(Answer::Sense(value)) => Ok(*value),
            Some(Answer::SenseFault(fault)) => Err(*fault),
            _ => Ok(0),
        };
        self.transcript.push(Call::Sense { k, answer });
        answer
    }

    fn act(&mut self, action: Action) -> bool {
        let ok = matches!(self.answers.next(), Some(Answer::Act(true)));
        self.transcript.push(Call::Act { action, ok });
        ok
    }

    fn rand(&mut self) -> i64 {
        let value = match self.answers.next() {
            Some(Answer::Rand(value)) => *value,
            _ => 0,
        };
        self.transcript.push(Call::Rand { value });
        value
    }
}

/// La première différence entre ce qu'un cas attend et ce que la machine a fait.
#[derive(Debug, Clone, PartialEq)]
pub enum Divergence {
    /// Le cas attendait un refus de chargement.
    ExpectedRefusal,
    /// Le programme aurait dû se charger.
    UnexpectedRefusal,
    Calls {
        expected: Vec<Call>,
        actual: Vec<Call>,
    },
    GasUsed {
        expected: u16,
        actual: u16,
    },
    Fault {
        expected: Option<Fault>,
        actual: Option<Fault>,
    },
    Act {
        expected: Option<ActRecord>,
        actual: Option<ActRecord>,
    },
    Trace {
        expected: Vec<i64>,
        actual: Vec<i64>,
    },
    Mem {
        index: usize,
        expected: i64,
        actual: i64,
    },
}

/// Première différence dans l'ordre de FORMAT.md : rappels, gaz, faute, action, trace, mémoire.
pub fn compare(
    expected_calls: &[Call],
    expected: &Report,
    actual_calls: &[Call],
    actual: &Report,
) -> Option<Divergence> {
    if expected_calls != actual_calls {
        return Some(Divergence::Calls {
            expected: expected_calls.to_vec(),
            actual: actual_calls.to_vec(),
        });
    }
    if expected.gas_used != actual.gas_used {
        return Some(Divergence::GasUsed {
            expected: expected.gas_used,
            actual: actual.gas_used,
        });
    }
    if expected.fault != actual.fault {
        return Some(Divergence::Fault {
            expected: expected.fault,
            actual: actual.fault,
        });
    }
    if expected.act != actual.act {
        return Some(Divergence::Act {
            expected: expected.act,
            actual: actual.act,
        });
    }
    if expected.trace != actual.trace {
        return Some(Divergence::Trace {
            expected: expected.trace.clone(),
            actual: actual.trace.clone(),
        });
    }
    expected
        .mem
        .iter()
        .zip(actual.mem.iter())
        .enumerate()
        .find(|(_, (e, a))| e != a)
        .map(|(index, (&expected, &actual))| Divergence::Mem {
            index,
            expected,
            actual,
        })
}

/// Un cas en échec : son nom, l'exécution fautive (aucune pour un refus de chargement) et la raison.
#[derive(Debug)]
pub struct CaseFailure {
    pub case: String,
    pub exec: Option<usize>,
    pub reason: Reason,
}

#[derive(Debug)]
pub enum Reason {
    Diverged(Divergence),
    Backend(BackendError),
}

/// Rejoue un cas : un refus doit être refusé, et chaque exécution doit donner le transcript et le `RESULT` attendus.
pub fn check_case(case: &Case, backend: &mut dyn Backend) -> Result<(), CaseFailure> {
    let fail = |exec, reason| CaseFailure {
        case: case.name.clone(),
        exec,
        reason,
    };
    match &case.expectation {
        Expectation::Refused { .. } => match backend.accepts(&case.program) {
            Ok(false) => Ok(()),
            Ok(true) => Err(fail(None, Reason::Diverged(Divergence::ExpectedRefusal))),
            Err(error) => Err(fail(None, Reason::Backend(error))),
        },
        Expectation::Runs(execs) => {
            for (index, exec) in execs.iter().enumerate() {
                let mut replay = Replay::new(&exec.answers);
                let report =
                    match backend.execute(&case.program, &exec.mem, exec.budget, &mut replay) {
                        Ok(report) => report,
                        Err(BackendError::Refused) => {
                            return Err(fail(
                                Some(index),
                                Reason::Diverged(Divergence::UnexpectedRefusal),
                            ));
                        }
                        Err(error) => return Err(fail(Some(index), Reason::Backend(error))),
                    };
                if let Some(divergence) =
                    compare(&exec.calls, &exec.expected, replay.transcript(), &report)
                {
                    return Err(fail(Some(index), Reason::Diverged(divergence)));
                }
            }
            Ok(())
        }
    }
}

/// « 016-gas-exact-exhaustion [exec 0] : gaz 12 au lieu de 11 »
impl fmt::Display for CaseFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.exec {
            Some(index) => write!(f, "{} [exec {index}] : {}", self.case, self.reason),
            None => write!(f, "{} : {}", self.case, self.reason),
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::Diverged(divergence) => divergence.fmt(f),
            Reason::Backend(error) => error.fmt(f),
        }
    }
}

/// Ce qui a été obtenu, puis ce qui était attendu ; rappels et actions sous leur forme de expected.json.
impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Divergence::ExpectedRefusal => f.write_str("programme chargé, il devait être refusé"),
            Divergence::UnexpectedRefusal => {
                f.write_str("programme refusé au chargement, il devait se charger")
            }
            Divergence::Calls { expected, actual } => {
                write!(
                    f,
                    "rappels {} au lieu de {}",
                    calls(actual),
                    calls(expected)
                )
            }
            Divergence::GasUsed { expected, actual } => {
                write!(f, "gaz {actual} au lieu de {expected}")
            }
            Divergence::Fault { expected, actual } => {
                write!(
                    f,
                    "faute {} au lieu de {}",
                    fault(*actual),
                    fault(*expected)
                )
            }
            Divergence::Act { expected, actual } => {
                write!(f, "action {} au lieu de {}", act(*actual), act(*expected))
            }
            Divergence::Trace { expected, actual } => {
                write!(f, "trace {actual:?} au lieu de {expected:?}")
            }
            Divergence::Mem {
                index,
                expected,
                actual,
            } => write!(f, "mem[{index}] vaut {actual} au lieu de {expected}"),
        }
    }
}

fn calls(calls: &[Call]) -> serde_json::Value {
    calls.iter().map(Call::to_json).collect()
}

fn fault(fault: Option<Fault>) -> &'static str {
    fault.map_or("aucune", Fault::as_str)
}

fn act(record: Option<ActRecord>) -> String {
    match record {
        None => "aucune".to_string(),
        Some(ActRecord { action, ok }) => {
            serde_json::json!({"kind": action.kind.name(), "arg": action.arg, "ok": ok}).to_string()
        }
    }
}
