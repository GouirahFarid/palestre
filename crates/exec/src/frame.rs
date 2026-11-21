//! Le cadrage de K.2 : un octet de type, une longueur `u32` gros-boutiste, puis la charge utile.

use std::io::{self, Read, Write};

/// Borne de `len` du client vers le serveur : le plus grand `.pbc` possible (A.1), `8 + 8 × 4095 + 4 + 65535`.
pub const CLIENT_MAX_LEN: u32 = 98_307;
/// Borne de `len` du serveur vers le client : un `RESULT` de `256 + 65535` entiers écrits en JSON.
pub const SERVER_MAX_LEN: u32 = 4_194_304;

/// Les treize types de trame de K.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Open = 0x11,
    Ready = 0x12,
    Submit = 0x13,
    Session = 0x14,
    Exec = 0x15,
    Sense = 0x16,
    Sensed = 0x17,
    Act = 0x18,
    Acted = 0x19,
    Rand = 0x1A,
    Drew = 0x1B,
    Result = 0x1C,
    Error = 0x1D,
}

impl FrameType {
    const ALL: [FrameType; 13] = [
        FrameType::Open,
        FrameType::Ready,
        FrameType::Submit,
        FrameType::Session,
        FrameType::Exec,
        FrameType::Sense,
        FrameType::Sensed,
        FrameType::Act,
        FrameType::Acted,
        FrameType::Rand,
        FrameType::Drew,
        FrameType::Result,
        FrameType::Error,
    ];

    /// Le type désigné par cet octet, ou `None` hors de `0x11..=0x1D`.
    pub fn from_byte(byte: u8) -> Option<FrameType> {
        FrameType::ALL.into_iter().find(|kind| kind.byte() == byte)
    }

    pub fn byte(self) -> u8 {
        self as u8
    }
}

/// Une trame lue : son type et sa charge utile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: FrameType,
    pub payload: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    /// La connexion s'est fermée, avant la trame ou au milieu (K.8 : pas d'`ERROR` dans ce cas).
    #[error("connexion fermée")]
    Closed,
    #[error("type de trame inconnu {0:#04x}")]
    UnknownType(u8),
    #[error("longueur {len} au-delà de la borne {max}")]
    TooLong { len: u32, max: u32 },
    /// Toute autre erreur d'entrée-sortie, dont un délai de lecture dépassé.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Lit une trame entière. La longueur annoncée est comparée à `max_len` avant toute allocation (K.2).
pub fn read_frame<R: Read>(reader: &mut R, max_len: u32) -> Result<Frame, FrameError> {
    let mut header = [0; 5];
    read_all(reader, &mut header)?;
    let [kind, len @ ..] = header;
    let kind = FrameType::from_byte(kind).ok_or(FrameError::UnknownType(kind))?;
    let len = u32::from_be_bytes(len);
    if len > max_len {
        return Err(FrameError::TooLong { len, max: max_len });
    }
    let len = usize::try_from(len).map_err(|_| FrameError::TooLong { len, max: max_len })?;
    let mut payload = vec![0; len];
    read_all(reader, &mut payload)?;
    Ok(Frame { kind, payload })
}

/// Écrit une trame en un seul `write_all`, puis vide le tampon.
///
/// La longueur n'est pas bornée ici : c'est l'appelant qui sait dans quel sens il écrit. Une charge utile de plus de
/// `u32::MAX` octets est refusée avec `InvalidInput`.
pub fn write_frame<W: Write>(writer: &mut W, kind: FrameType, payload: &[u8]) -> io::Result<()> {
    let len = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "charge utile trop longue"))?;
    let mut bytes = Vec::with_capacity(payload.len().saturating_add(5));
    bytes.push(kind.byte());
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(payload);
    writer.write_all(&bytes)?;
    writer.flush()
}

/// `read_exact`, où une fin de flux devient `Closed`.
fn read_all<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<(), FrameError> {
    reader.read_exact(buf).map_err(|error| match error.kind() {
        io::ErrorKind::UnexpectedEof => FrameError::Closed,
        _ => FrameError::Io(error),
    })
}
