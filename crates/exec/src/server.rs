//! Le serveur de l'annexe K : séquentiel par défaut (D-008), une connexion après l'autre, ou un thread par connexion
//! (D-013). Dans les deux cas, chaque connexion a ses propres sessions.
//!
//! Machine d'états d'une connexion (K.4) : `Greeting` n'accepte que `OPEN`, `Idle` accepte `SUBMIT` et `EXEC`, et
//! `Executing` n'accepte que la réplique du rappel en cours.

use std::convert::Infallible;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::num::NonZeroU16;
use std::thread;
use std::time::Duration;

use pbc::{Program, SPEC_VERSION};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use vm::{Machine, Memory, Step};

use crate::frame::{CLIENT_MAX_LEN, Frame, FrameError, FrameType, read_frame, write_frame};
use crate::message::{
    self, ActReq, Acted, Drew, ErrorCode, ErrorMsg, Exec, Open, RandReq, Ready, ResultMsg,
    SenseReq, Sensed, Session, valid_session_id,
};

/// La seule version du protocole gérée (K.3).
const PROTO: u32 = 1;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Nombre de sessions qu'une connexion peut ouvrir ; annoncé dans `READY`.
    pub sessions_max: u32,
    /// Délai au-delà duquel un client muet est déconnecté, sans `ERROR` (D-008). `None` : aucun délai.
    pub read_timeout: Option<Duration>,
}

impl Default for ServerConfig {
    fn default() -> ServerConfig {
        ServerConfig {
            sessions_max: 16,
            read_timeout: Some(Duration::from_secs(30)),
        }
    }
}

/// Comment une connexion s'est terminée.
#[derive(Debug)]
pub enum ConnectionEnd {
    /// Le client est parti, y compris au milieu d'une trame (K.8).
    Closed,
    /// Une `ERROR` qui ferme la connexion a été envoyée.
    Refused(ErrorCode),
    /// Une erreur d'entrée-sortie, dont le délai de lecture dépassé.
    Io(io::Error),
}

impl fmt::Display for ConnectionEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionEnd::Closed => f.write_str("fermée par le client"),
            ConnectionEnd::Refused(code) => write!(f, "refusée ({code:?})"),
            ConnectionEnd::Io(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                f.write_str("client muet, délai de lecture dépassé")
            }
            ConnectionEnd::Io(error) => write!(f, "erreur d'entrée-sortie : {error}"),
        }
    }
}

/// Sert les connexions une par une, indéfiniment. Une connexion qui échoue est consignée sur la sortie d'erreur, et
/// le serveur passe à la suivante.
pub fn serve(listener: TcpListener, config: &ServerConfig) -> io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => serve_one(stream, config),
            Err(error) => eprintln!("palestre exec : connexion manquée : {error}"),
        }
    }
    Ok(())
}

/// Un thread par connexion (D-013). Rien n'est partagé entre threads : chaque connexion possède son flux, sa copie
/// de la configuration et ses sessions.
pub fn serve_concurrent(listener: TcpListener, config: &ServerConfig) -> io::Result<()> {
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("palestre exec : connexion manquée : {error}");
                continue;
            }
        };
        // `stream` (un `TcpStream`) et `config` (un `u32` et une `Option<Duration>`) sont `Send` et possédés : ils
        // sont déplacés dans le thread, ce qui satisfait `Send + 'static` sans `Arc` ni `Mutex`.
        let config = config.clone();
        // `Builder::spawn` plutôt que `thread::spawn`, qui panique quand le système refuse un thread de plus.
        let spawned = thread::Builder::new().spawn(move || serve_one(stream, &config));
        if let Err(error) = spawned {
            eprintln!("palestre exec : connexion abandonnée, pas de thread : {error}");
        }
    }
    Ok(())
}

/// Toute une connexion TCP : délais, protocole, puis une ligne sur la sortie d'erreur pour dire comment elle a fini.
fn serve_one(mut stream: TcpStream, config: &ServerConfig) {
    let timeout = config.read_timeout.filter(|delay| !delay.is_zero());
    let peer = stream
        .peer_addr()
        .map_or_else(|_| "client inconnu".to_string(), |addr| addr.to_string());
    // Le même délai vaut pour l'écriture : un client qui ne lit plus bloquerait lui aussi le serveur.
    if let Err(error) = stream
        .set_read_timeout(timeout)
        .and_then(|()| stream.set_write_timeout(timeout))
    {
        eprintln!("palestre exec : {peer} : {error}");
        return;
    }
    let end = handle_connection(&mut stream, config);
    eprintln!("palestre exec : {peer} : {end}");
}

/// Toute la vie d'une connexion. Générique pour être testée et fuzzée en mémoire (T14).
pub fn handle_connection<S: Read + Write>(stream: &mut S, config: &ServerConfig) -> ConnectionEnd {
    let mut connection = Connection {
        config,
        sessions: Vec::new(),
        next_id: 1,
    };
    let Err(end) = connection.run(stream);
    end
}

struct Connection<'c> {
    config: &'c ServerConfig,
    /// Les sessions de cette connexion, dans l'ordre des `SUBMIT` (D-008).
    sessions: Vec<(String, Program)>,
    next_id: u64,
}

impl Connection<'_> {
    /// Ne rend la main que quand la connexion se termine.
    fn run<S: Read + Write>(&mut self, stream: &mut S) -> Result<Infallible, ConnectionEnd> {
        self.greet(stream)?;
        loop {
            let frame = receive(stream)?;
            match frame.kind {
                FrameType::Submit => self.submit(stream, &frame.payload)?,
                FrameType::Exec => self.exec(stream, &frame.payload)?,
                other => {
                    return reject(
                        stream,
                        ErrorCode::UnexpectedMessage,
                        format!("{} interdit hors d'une exécution", name(other)),
                    );
                }
            }
        }
    }

    /// `Greeting` : `OPEN`, puis `READY`.
    fn greet<S: Read + Write>(&self, stream: &mut S) -> Result<(), ConnectionEnd> {
        let frame = receive(stream)?;
        if frame.kind != FrameType::Open {
            return reject(
                stream,
                ErrorCode::UnexpectedMessage,
                format!("OPEN attendu, {} reçu", name(frame.kind)),
            );
        }
        let open: Open = parse(stream, &frame.payload)?;
        if open.proto != PROTO {
            return reject(
                stream,
                ErrorCode::BadProto,
                format!("proto {} non géré, seul {PROTO} l'est", open.proto),
            );
        }
        let ready = Ready {
            proto: PROTO,
            spec: "PBC1".to_string(),
            spec_version: SPEC_VERSION.to_string(),
            sessions_max: self.config.sessions_max,
        };
        send(stream, FrameType::Ready, &ready)
    }

    /// `SUBMIT` : le programme doit se charger (sinon la connexion se ferme), puis il faut une place libre (sinon un
    /// refus qui laisse la connexion ouverte).
    fn submit<S: Write>(&mut self, stream: &mut S, bytes: &[u8]) -> Result<(), ConnectionEnd> {
        let program = match Program::load(bytes) {
            Ok(program) => program,
            Err(error) => return reject(stream, ErrorCode::BadProgram, error.to_string()),
        };
        let full =
            u32::try_from(self.sessions.len()).map_or(true, |n| n >= self.config.sessions_max);
        if full {
            return send_error(
                stream,
                ErrorCode::TooManySessions,
                format!("{} sessions déjà ouvertes", self.sessions.len()),
            );
        }
        let id = self.next_id.to_string();
        self.next_id = self.next_id.saturating_add(1);
        let session = Session {
            session: id.clone(),
            pbc_sha256: Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            code_len: u32::try_from(program.code().len()).unwrap_or(u32::MAX),
        };
        self.sessions.push((id, program));
        send(stream, FrameType::Session, &session)
    }

    /// `EXEC`, validé dans l'ordre de D-007 : forme et identifiant, `mem`, `budget`, puis l'existence de la session.
    fn exec<S: Read + Write>(&self, stream: &mut S, payload: &[u8]) -> Result<(), ConnectionEnd> {
        let request: Exec = parse(stream, payload)?;
        if !valid_session_id(&request.session) {
            return reject(
                stream,
                ErrorCode::BadFrame,
                "session hors des bornes de K.4 (1 à 64 caractères dans [A-Za-z0-9_-])",
            );
        }
        let Ok(mem) = Memory::try_from(request.mem.as_slice()) else {
            return reject(
                stream,
                ErrorCode::BadMemory,
                format!(
                    "`mem` a {} valeurs au lieu de {}",
                    request.mem.len(),
                    vm::MEM_SIZE
                ),
            );
        };
        let Some(budget) = u16::try_from(request.budget).ok().and_then(NonZeroU16::new) else {
            return reject(
                stream,
                ErrorCode::BadBudget,
                format!("budget {} hors de 1..=65535", request.budget),
            );
        };
        let Some((_, program)) = self.sessions.iter().find(|(id, _)| *id == request.session) else {
            return send_error(
                stream,
                ErrorCode::NoSuchSession,
                format!("session `{}` inconnue sur cette connexion", request.session),
            );
        };
        execute(stream, program, mem, budget)
    }
}

/// `Executing` : chaque rappel de la machine devient un aller-retour avec le client (K.6). Le `RESULT` n'est envoyé
/// que si tous les rappels ont abouti.
fn execute<S: Read + Write>(
    stream: &mut S,
    program: &Program,
    mem: Memory,
    budget: NonZeroU16,
) -> Result<(), ConnectionEnd> {
    let mut step = Machine::new(program, mem, budget).run();
    loop {
        step = match step {
            Step::Done(report) => {
                return send(stream, FrameType::Result, &ResultMsg::from_report(&report));
            }
            Step::Sense(call) => {
                send(stream, FrameType::Sense, &SenseReq { k: call.k() })?;
                let sensed: Sensed = reply(stream, FrameType::Sensed)?;
                match sensed.to_answer() {
                    Ok(answer) => call.answer(answer),
                    Err(error) => return reject(stream, ErrorCode::BadFrame, error.to_string()),
                }
            }
            Step::Act(call) => {
                send(stream, FrameType::Act, &ActReq::from_action(call.action()))?;
                let acted: Acted = reply(stream, FrameType::Acted)?;
                call.answer(acted.ok)
            }
            Step::Rand(call) => {
                send(stream, FrameType::Rand, &RandReq {})?;
                let drew: Drew = reply(stream, FrameType::Drew)?;
                call.answer(drew.value)
            }
        };
    }
}

/// Lit la trame suivante. Un type inconnu ou une longueur hors borne donne `bad_frame` (K.2) ; une coupure ferme la
/// connexion sans `ERROR` (K.8).
fn receive<S: Read + Write>(stream: &mut S) -> Result<Frame, ConnectionEnd> {
    match read_frame(stream, CLIENT_MAX_LEN) {
        Ok(frame) => Ok(frame),
        Err(FrameError::Closed) => Err(ConnectionEnd::Closed),
        Err(FrameError::Io(error)) => Err(ConnectionEnd::Io(error)),
        Err(error @ (FrameError::UnknownType(_) | FrameError::TooLong { .. })) => {
            reject(stream, ErrorCode::BadFrame, error.to_string())
        }
    }
}

/// La réplique exacte du rappel en cours, sinon `unexpected_message` (K.4).
fn reply<S: Read + Write, T: DeserializeOwned>(
    stream: &mut S,
    expected: FrameType,
) -> Result<T, ConnectionEnd> {
    let frame = receive(stream)?;
    if frame.kind != expected {
        return reject(
            stream,
            ErrorCode::UnexpectedMessage,
            format!("{} attendu, {} reçu", name(expected), name(frame.kind)),
        );
    }
    parse(stream, &frame.payload)
}

/// Un JSON invalide donne `bad_frame` (K.8).
fn parse<S: Write, T: DeserializeOwned>(
    stream: &mut S,
    payload: &[u8],
) -> Result<T, ConnectionEnd> {
    message::from_json(payload)
        .or_else(|error| reject(stream, ErrorCode::BadFrame, error.to_string()))
}

fn send<S: Write, T: Serialize>(
    stream: &mut S,
    kind: FrameType,
    msg: &T,
) -> Result<(), ConnectionEnd> {
    let payload =
        message::to_json(msg).map_err(|error| ConnectionEnd::Io(io::Error::other(error)))?;
    write_frame(stream, kind, &payload).map_err(ConnectionEnd::Io)
}

/// Envoie une `ERROR` qui laisse la connexion ouverte (`too_many_sessions`, `no_such_session`).
fn send_error<S: Write>(
    stream: &mut S,
    code: ErrorCode,
    message: impl Into<String>,
) -> Result<(), ConnectionEnd> {
    let error = ErrorMsg {
        code,
        message: message.into(),
    };
    send(stream, FrameType::Error, &error)
}

/// Envoie une `ERROR` qui ferme la connexion, et rend la fin correspondante.
fn reject<S: Write, T>(
    stream: &mut S,
    code: ErrorCode,
    message: impl Into<String>,
) -> Result<T, ConnectionEnd> {
    debug_assert!(
        code.closes_connection(),
        "{code:?} ne ferme pas la connexion"
    );
    send_error(stream, code, message)?;
    Err(ConnectionEnd::Refused(code))
}

/// Le nom d'une trame tel que K.3 l'écrit : `OPEN`, `SENSED`…
fn name(kind: FrameType) -> String {
    format!("{kind:?}").to_uppercase()
}
