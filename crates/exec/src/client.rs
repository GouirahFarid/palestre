//! Le client de l'annexe K : il dépose un programme, le fait exécuter, et répond aux rappels du serveur par un
//! `vm::Env`. C'est l'outil de J4, pour rejouer le corpus contre le serveur d'un autre groupe.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::num::NonZeroU16;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use vm::{Env, Memory, Report};

use crate::frame::{FrameError, FrameType, SERVER_MAX_LEN, read_frame, write_frame};
use crate::message::{
    self, ActReq, Acted, Drew, ErrorMsg, Exec, Open, ProtocolError, RandReq, Ready, ResultMsg,
    SenseReq, Sensed, Session,
};

/// La version du protocole demandée dans `OPEN` (K.3).
const PROTO: u32 = 1;
/// Attente maximale d'une trame du serveur : un serveur figé fait échouer un cas au lieu de bloquer tout le corpus.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Une connexion ouverte, accueil passé.
pub struct Client<S = TcpStream> {
    stream: S,
    ready: Ready,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("le serveur répond {code:?} : {message}", code = .0.code, message = .0.message)]
    Server(ErrorMsg),
    #[error("trame inattendue {0:?}")]
    Unexpected(FrameType),
}

impl Client<TcpStream> {
    /// Connexion TCP, puis `OPEN` → `READY`.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Client<TcpStream>, ClientError> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        Client::over(stream)
    }
}

impl<S: Read + Write> Client<S> {
    /// `OPEN` → `READY` sur un flux déjà ouvert.
    pub fn over(mut stream: S) -> Result<Client<S>, ClientError> {
        let open = Open {
            proto: PROTO,
            client: Some("palestre".to_string()),
        };
        send(&mut stream, FrameType::Open, &open)?;
        let ready = expect(&mut stream, FrameType::Ready)?;
        Ok(Client { stream, ready })
    }

    /// Ce que le serveur a annoncé dans `READY`.
    pub fn ready(&self) -> &Ready {
        &self.ready
    }

    /// `SUBMIT` → `SESSION`. Une `ERROR` du serveur devient `ClientError::Server`.
    pub fn submit(&mut self, program: &[u8]) -> Result<Session, ClientError> {
        write_frame(&mut self.stream, FrameType::Submit, program)?;
        expect(&mut self.stream, FrameType::Session)
    }

    /// `EXEC`, puis répond à chaque rappel par `env`, jusqu'au `RESULT`. L'identifiant de session est renvoyé tel
    /// quel, sans être interprété (K.4).
    pub fn exec(
        &mut self,
        session: &str,
        mem: &Memory,
        budget: NonZeroU16,
        env: &mut dyn Env,
    ) -> Result<Report, ClientError> {
        let request = Exec {
            session: session.to_string(),
            mem: mem.to_vec(),
            budget: i64::from(budget.get()),
        };
        send(&mut self.stream, FrameType::Exec, &request)?;
        loop {
            let frame = read_frame(&mut self.stream, SERVER_MAX_LEN)?;
            match frame.kind {
                FrameType::Sense => {
                    let request: SenseReq = message::from_json(&frame.payload)?;
                    let reply = Sensed::from_answer(env.sense(request.k));
                    send(&mut self.stream, FrameType::Sensed, &reply)?;
                }
                FrameType::Act => {
                    let request: ActReq = message::from_json(&frame.payload)?;
                    let ok = env.act(request.to_action()?);
                    send(&mut self.stream, FrameType::Acted, &Acted { ok })?;
                }
                FrameType::Rand => {
                    let RandReq {} = message::from_json(&frame.payload)?;
                    let value = env.rand();
                    send(&mut self.stream, FrameType::Drew, &Drew { value })?;
                }
                FrameType::Result => {
                    let result: ResultMsg = message::from_json(&frame.payload)?;
                    return Ok(result.to_report()?);
                }
                FrameType::Error => {
                    return Err(ClientError::Server(message::from_json(&frame.payload)?));
                }
                other => return Err(ClientError::Unexpected(other)),
            }
        }
    }

    pub fn into_inner(self) -> S {
        self.stream
    }
}

fn send<S: Write, T: Serialize>(
    stream: &mut S,
    kind: FrameType,
    msg: &T,
) -> Result<(), ClientError> {
    write_frame(stream, kind, &message::to_json(msg)?)?;
    Ok(())
}

/// La trame attendue, ou l'`ERROR` du serveur, ou `Unexpected`.
fn expect<S: Read, T: DeserializeOwned>(stream: &mut S, kind: FrameType) -> Result<T, ClientError> {
    let frame = read_frame(stream, SERVER_MAX_LEN)?;
    if frame.kind == kind {
        Ok(message::from_json(&frame.payload)?)
    } else if frame.kind == FrameType::Error {
        Err(ClientError::Server(message::from_json(&frame.payload)?))
    } else {
        Err(ClientError::Unexpected(frame.kind))
    }
}
