//! Vérificateur statique du bytecode `PBC v1` (bonus, D-014).
//!
//! Il explore, avant toute exécution, tous les états abstraits atteignables depuis `pc = 0` : une adresse, une hauteur
//! de pile, la pile des adresses de retour et le fait qu'une action a déjà été tentée. Les valeurs sont ignorées, donc
//! les deux branches d'un `JZ` sont suivies, et un `JMPX` peut atteindre tout octet `0x43` du code. Chaque état est
//! décodé à son propre `pc` : un saut au milieu d'un opérande (A.3) est suivi comme un autre.
//!
//! Le vérificateur est correct (*sound*) pour les fautes structurelles : toute faute qui ne dépend pas des valeurs et
//! qui arrive à l'exécution a été annoncée (test `every_structural_fault_seen_at_runtime_was_announced`).
#![forbid(unsafe_code)]
#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use std::collections::{BTreeSet, HashSet};

use pbc::{Fault, Instr, Op, Program, decode};
use vm::{ActKind, CALL_DEPTH, STACK_SIZE};

/// Une faute *possible* à cette adresse : au moins un chemin y mène, quelles que soient les valeurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Issue {
    pub pc: usize,
    pub fault: Fault,
}

/// Le résultat de l'analyse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// Les fautes possibles, triées par adresse puis dans l'ordre de B.5, sans doublon.
    pub issues: Vec<Issue>,
    /// La hauteur de pile maximale atteignable.
    pub max_stack: usize,
    /// La profondeur d'appel maximale atteignable.
    pub max_frames: usize,
    /// Le nombre d'états abstraits explorés.
    pub states: usize,
    /// Faux si la limite d'états a été atteinte : l'analyse s'est arrêtée avant d'avoir tout vu.
    pub complete: bool,
}

impl Verdict {
    /// Sûr : l'analyse a tout exploré, et aucun chemin ne mène à une faute structurelle.
    pub fn is_safe(&self) -> bool {
        self.complete && self.issues.is_empty()
    }
}

/// Les bornes de l'analyse.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Au-delà de ce nombre d'états, l'analyse s'arrête et le verdict est incomplet.
    pub max_states: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_states: 1_000_000,
        }
    }
}

/// Vérifie `program` avec les limites par défaut.
pub fn verify(program: &Program) -> Verdict {
    verify_with(program, Limits::default())
}

/// Explore les états atteignables depuis `pc = 0`, jusqu'au dernier ou jusqu'à `limits.max_states`.
pub fn verify_with(program: &Program, limits: Limits) -> Verdict {
    let analysis = Analysis::new(program);
    let mut search = Search::new(limits);
    let mut issues = BTreeSet::new();
    let mut next = Vec::new();

    search.discover(State {
        pc: 0,
        height: 0,
        returns: Vec::new(),
        acted: false,
    });
    while let Some(state) = search.todo.pop() {
        if let Some(fault) = analysis.step(&state, &mut next) {
            issues.insert(Issue {
                pc: state.pc,
                fault,
            });
        }
        for successor in next.drain(..) {
            search.discover(successor);
        }
        if !search.complete {
            break;
        }
    }

    Verdict {
        issues: issues.into_iter().collect(),
        max_stack: search.max_stack,
        max_frames: search.max_frames,
        states: search.seen.len(),
        complete: search.complete,
    }
}

/// Un état abstrait de la machine : tout ce dont dépendent les fautes structurelles, et rien des valeurs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct State {
    pc: usize,
    /// Le nombre de valeurs sur la pile.
    height: usize,
    /// Les adresses de retour des appels en cours, exactes : `RETURN` revient là où le `CALL` l'a dit.
    returns: Vec<usize>,
    /// Un `ACT` a déjà été tenté dans ce tour.
    acted: bool,
}

/// Les états déjà vus et ceux qui restent à explorer.
struct Search {
    limits: Limits,
    seen: HashSet<State>,
    todo: Vec<State>,
    max_stack: usize,
    max_frames: usize,
    complete: bool,
}

impl Search {
    fn new(limits: Limits) -> Search {
        Search {
            limits,
            seen: HashSet::new(),
            todo: Vec::new(),
            max_stack: 0,
            max_frames: 0,
            complete: true,
        }
    }

    /// Ajoute un état à explorer s'il est nouveau. À la limite d'états, l'analyse devient incomplète.
    fn discover(&mut self, state: State) {
        if !self.complete || self.seen.contains(&state) {
            return;
        }
        if self.seen.len() >= self.limits.max_states {
            self.complete = false;
            return;
        }
        self.max_stack = self.max_stack.max(state.height);
        self.max_frames = self.max_frames.max(state.returns.len());
        self.seen.insert(state.clone());
        self.todo.push(state);
    }
}

/// Ce que l'analyse sait du programme : son code, la taille de sa table de constantes, et ses marqueurs.
struct Analysis<'p> {
    code: &'p [u8],
    constants: usize,
    /// Tous les octets `0x43` du code, y compris dans les opérandes : les cibles possibles de `JMPX` et `CALLX`.
    markers: Vec<usize>,
}

impl<'p> Analysis<'p> {
    fn new(program: &'p Program) -> Analysis<'p> {
        let code = program.code();
        let markers = code
            .iter()
            .enumerate()
            .filter(|&(_, &byte)| byte == Op::JumpDest.byte())
            .map(|(pc, _)| pc)
            .collect();
        Analysis {
            code,
            constants: program.constants().len(),
            markers,
        }
    }

    /// Rejoue l'instruction à `state.pc` sans les valeurs, avec les vérifications de la machine dans le même ordre
    /// (B.2, D-005) : décodage, arité, opérandes statiques, débordement, puis pile d'appels et cible.
    ///
    /// Pousse dans `next` les états qui suivent, et rend la faute possible à cette adresse s'il y en a une. Une faute
    /// certaine arrête le chemin ; une faute possible (`BadJump` d'un `JZ` ou d'un `JMPX`) laisse les autres continuer.
    fn step(&self, state: &State, next: &mut Vec<State>) -> Option<Fault> {
        let pc = state.pc;
        let instr = match decode(self.code, pc) {
            Ok(instr) => instr,
            Err(fault) => return Some(fault),
        };
        let (pops, pushes) = instr.op().stack_effect();
        let Some(base) = state.height.checked_sub(usize::from(pops)) else {
            return Some(Fault::StackUnderflow);
        };
        if let Some(fault) = self.operand_fault(instr, state) {
            return Some(fault);
        }
        let height = base.saturating_add(usize::from(pushes));
        if height > STACK_SIZE {
            return Some(Fault::StackOverflow);
        }

        let size = instr.size();
        let after = State {
            pc: pc.saturating_add(size),
            height,
            returns: state.returns.clone(),
            acted: state.acted || instr.op() == Op::Act,
        };
        match instr {
            Instr::Halt => None,
            Instr::Jmp(d) => self.land(relative(pc, size, d), after, next),
            // Les deux chemins : la cible si le saut a lieu, l'instruction suivante sinon.
            Instr::Jz(d) | Instr::Jnz(d) => {
                let fault = self.land(relative(pc, size, d), after.clone(), next);
                next.push(after);
                fault
            }
            Instr::JmpX => self.land_anywhere(after, next),
            // `CALL` ne continue pas à l'instruction suivante : c'est le `RETURN` qui y ramène.
            Instr::Call(d) => match enter(after) {
                Ok(after) => self.land(relative(pc, size, d), after, next),
                Err(fault) => Some(fault),
            },
            Instr::CallX => match enter(after) {
                Ok(after) => self.land_anywhere(after, next),
                Err(fault) => Some(fault),
            },
            Instr::Return => {
                let mut after = after;
                match after.returns.pop() {
                    Some(address) => {
                        next.push(State {
                            pc: address,
                            ..after
                        });
                        None
                    }
                    None => Some(Fault::CallStackUnderflow),
                }
            }
            _ => {
                next.push(after);
                None
            }
        }
    }

    /// Les fautes que l'opérande immédiat décide seul : `CONST k` hors de la table, `ACT k` inconnu, deuxième `ACT`.
    fn operand_fault(&self, instr: Instr, state: &State) -> Option<Fault> {
        match instr {
            Instr::Const(k) if usize::from(k) >= self.constants => Some(Fault::BadConst),
            Instr::Act(k) if ActKind::from_code(k).is_none() => Some(Fault::BadAction),
            Instr::Act(_) if state.acted => Some(Fault::AlreadyActed),
            _ => None,
        }
    }

    /// Les deux conditions de B.4 : la cible est dans le code et porte `0x43`. Sinon, `BadJump`.
    fn land(&self, target: Option<usize>, after: State, next: &mut Vec<State>) -> Option<Fault> {
        match target.filter(|&target| self.code.get(target) == Some(&Op::JumpDest.byte())) {
            Some(target) => {
                next.push(State {
                    pc: target,
                    ..after
                });
                None
            }
            None => Some(Fault::BadJump),
        }
    }

    /// `JMPX` et `CALLX` : la cible est une valeur, donc chaque marqueur est possible, et `BadJump` aussi.
    fn land_anywhere(&self, after: State, next: &mut Vec<State>) -> Option<Fault> {
        for &target in &self.markers {
            next.push(State {
                pc: target,
                ..after.clone()
            });
        }
        Some(Fault::BadJump)
    }
}

/// Empile l'adresse de retour d'un appel, celle de l'instruction suivante, ou `CallStackOverflow`.
fn enter(mut after: State) -> Result<State, Fault> {
    if after.returns.len() >= CALL_DEPTH {
        return Err(Fault::CallStackOverflow);
    }
    after.returns.push(after.pc);
    Ok(after)
}

/// La cible `pc + taille + d` d'un saut relatif (B.4), comme la calcule la machine ; `None` si elle est négative.
fn relative(pc: usize, size: usize, d: i16) -> Option<usize> {
    let next = i64::try_from(pc.checked_add(size)?).ok()?;
    usize::try_from(next.checked_add(i64::from(d))?).ok()
}
