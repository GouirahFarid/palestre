//! Chargement d'un fichier `.pbc` (annexes A.1 et A.2).
//!
//! Le chargeur accepte n'importe quelle suite d'octets : il rend un `Program` ou une `LoadError`, jamais un `panic`,
//! et n'alloue jamais plus que ce que les bornes de A.1 autorisent.

/// Nombre maximal de constantes (annexe A.1).
pub const MAX_CONSTANTS: usize = 4095;
/// Taille maximale du code en octets (annexe A.1).
pub const MAX_CODE_LEN: usize = 65_535;

const MAGIC: &[u8; 4] = b"PBC1";
const VERSION: u8 = 0x01;
const FLAGS: u8 = 0x00;

/// Pourquoi un fichier a été refusé.
///
/// Sur le fil, toutes ces erreurs portent le même nom, `BadHeader` (A.2) : le détail ne sert qu'au débogage (D-002).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
    #[error("fichier trop court")]
    TooShort,
    #[error("nombre magique invalide (attendu \"PBC1\")")]
    BadMagic,
    #[error("version {0} non gérée (attendu 1)")]
    BadVersion(u8),
    #[error("octet flags {0:#04x} non nul")]
    BadFlags(u8),
    #[error("{0} constantes annoncées (maximum 4095)")]
    TooManyConstants(usize),
    #[error("code_len {0} hors de 1..=65535")]
    BadCodeLen(usize),
    #[error("taille incohérente : {expected} octets attendus, {actual} fournis")]
    SizeMismatch { expected: usize, actual: usize },
}

impl LoadError {
    /// Nom normatif de la faute (B.5) : toute erreur de chargement est `BadHeader`.
    pub fn fault_name(&self) -> &'static str {
        "BadHeader"
    }
}

/// Un programme chargé : ses bornes ont été vérifiées, et ses champs privés empêchent de construire un programme qui
/// les violerait autrement que par `load` ou `new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    constants: Vec<i64>,
    code: Vec<u8>,
}

impl Program {
    /// Charge un fichier `.pbc` complet, dans l'ordre de A.1.
    ///
    /// Chaque borne est vérifiée avant de lire ou d'allouer ce qu'elle protège : un fichier qui annonce 4096
    /// constantes ou 4 Go de code est refusé sur son seul en-tête.
    pub fn load(bytes: &[u8]) -> Result<Program, LoadError> {
        let mut reader = Reader::new(bytes);

        if reader.take(4)? != MAGIC {
            return Err(LoadError::BadMagic);
        }
        let [version] = reader.array()?;
        if version != VERSION {
            return Err(LoadError::BadVersion(version));
        }
        let [flags] = reader.array()?;
        if flags != FLAGS {
            return Err(LoadError::BadFlags(flags));
        }

        let count = usize::from(u16::from_be_bytes(reader.array()?));
        if count > MAX_CONSTANTS {
            return Err(LoadError::TooManyConstants(count));
        }
        let mut constants = Vec::with_capacity(count); // au plus 4095 × 8 octets
        for _ in 0..count {
            constants.push(i64::from_be_bytes(reader.array()?));
        }

        let raw_len = u32::from_be_bytes(reader.array()?);
        // Sur une plateforme 16 bits, une longueur qui ne tient pas dans `usize` est de toute façon hors bornes.
        let code_len = usize::try_from(raw_len).unwrap_or(usize::MAX);
        if !(1..=MAX_CODE_LEN).contains(&code_len) {
            return Err(LoadError::BadCodeLen(code_len));
        }

        // Le fichier doit s'arrêter exactement à la fin du code : ni octet manquant, ni octet en trop.
        let expected = reader
            .position()
            .checked_add(code_len)
            .ok_or(LoadError::TooShort)?;
        if bytes.len() != expected {
            return Err(LoadError::SizeMismatch {
                expected,
                actual: bytes.len(),
            });
        }
        let code = reader.take(code_len)?.to_vec();

        Program::new(constants, code)
    }

    /// Construit un programme en mémoire (assembleur, tests) avec les mêmes bornes que `load`.
    pub fn new(constants: Vec<i64>, code: Vec<u8>) -> Result<Program, LoadError> {
        if constants.len() > MAX_CONSTANTS {
            return Err(LoadError::TooManyConstants(constants.len()));
        }
        if !(1..=MAX_CODE_LEN).contains(&code.len()) {
            return Err(LoadError::BadCodeLen(code.len()));
        }
        Ok(Program { constants, code })
    }

    pub fn constants(&self) -> &[i64] {
        &self.constants
    }

    pub fn code(&self) -> &[u8] {
        &self.code
    }

    /// Sérialise au format A.1 : `Program::load(&p.to_bytes()) == Ok(p)`.
    pub fn to_bytes(&self) -> Vec<u8> {
        // Les conversions ne peuvent pas échouer : `new` garantit au plus 4095 constantes et 65 535 octets de code.
        let count = u16::try_from(self.constants.len()).unwrap_or(u16::MAX);
        let code_len = u32::try_from(self.code.len()).unwrap_or(u32::MAX);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.push(VERSION);
        bytes.push(FLAGS);
        bytes.extend_from_slice(&count.to_be_bytes());
        for constant in &self.constants {
            bytes.extend_from_slice(&constant.to_be_bytes());
        }
        bytes.extend_from_slice(&code_len.to_be_bytes());
        bytes.extend_from_slice(&self.code);
        bytes
    }
}

/// Curseur de lecture qui ne peut pas lire hors limites : chaque lecture passe par `get` et rend `TooShort` s'il
/// manque des octets.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, pos: 0 }
    }

    fn position(&self) -> usize {
        self.pos
    }

    /// Les `n` octets suivants, ou `TooShort`.
    fn take(&mut self, n: usize) -> Result<&'a [u8], LoadError> {
        let end = self.pos.checked_add(n).ok_or(LoadError::TooShort)?;
        let slice = self.bytes.get(self.pos..end).ok_or(LoadError::TooShort)?;
        self.pos = end;
        Ok(slice)
    }

    /// Les `N` octets suivants sous forme de tableau, prêts pour `from_be_bytes`.
    fn array<const N: usize>(&mut self) -> Result<[u8; N], LoadError> {
        // `take(N)` rend exactement N octets : la conversion ne peut pas échouer.
        self.take(N)?.try_into().map_err(|_| LoadError::TooShort)
    }
}
