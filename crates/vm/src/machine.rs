//! La boucle d'un tour (B.2), les instructions de pile et l'arithmétique (§0, B.6).

use std::num::NonZeroU16;

use pbc::{Fault, Instr, Program, decode};

use crate::{ActRecord, Memory, Report, STACK_SIZE};

/// L'état d'un agent pendant un tour. La pile est un tableau de taille fixe et la trace est préallouée dans `new` :
/// rien n'est alloué pendant l'exécution (README §3).
pub struct Machine<'p> {
    program: &'p Program,
    stack: [i64; STACK_SIZE],
    /// Nombre de valeurs sur la pile ; le sommet est `stack[sp - 1]`.
    sp: usize,
    mem: Memory,
    pc: usize,
    budget: u16,
    gas_left: u16,
    trace: Vec<i64>,
    act: Option<ActRecord>,
}

/// Où en est l'exécution quand `run` rend la main. Les rappels `SENSE`, `ACT` et `RAND` s'y ajoutent en T07 (D-004).
pub enum Step {
    Done(Report),
}

impl Step {
    /// `Some(report)` si l'exécution est terminée.
    pub fn done(self) -> Option<Report> {
        match self {
            Step::Done(report) => Some(report),
        }
    }
}

/// Ce que l'exécution d'une instruction demande à la boucle.
enum Flow {
    Next,
    Halt,
}

impl<'p> Machine<'p> {
    /// Un tour neuf (B.1) : pile vide, `pc = 0`, gaz égal au budget.
    pub fn new(program: &'p Program, mem: Memory, budget: NonZeroU16) -> Machine<'p> {
        Machine {
            program,
            stack: [0; STACK_SIZE],
            sp: 0,
            mem,
            pc: 0,
            budget: budget.get(),
            gas_left: budget.get(),
            // TRACE coûte 1 : un tour ne peut pas produire plus de `budget` entrées (B.3).
            trace: Vec::with_capacity(usize::from(budget.get())),
            act: None,
        }
    }

    /// Exécute jusqu'à `HALT` ou une faute, en appliquant B.2 à chaque pas.
    pub fn run(mut self) -> Step {
        loop {
            // Étapes 1 à 3 : décoder. Une faute de décodage ne coûte rien.
            let instr = match decode(self.program.code(), self.pc) {
                Ok(instr) => instr,
                Err(fault) => return self.finish(Some(fault)),
            };
            // Étape 4 : prélever le gaz, ou refuser l'instruction sans l'exécuter ni la facturer.
            let Some(left) = self.gas_left.checked_sub(instr.gas()) else {
                return self.finish(Some(Fault::OutOfGas));
            };
            self.gas_left = left;
            // Étapes 5 et 6 : exécuter (une faute ici a déjà payé son gaz), puis avancer.
            match self.exec(instr) {
                Ok(Flow::Next) => self.pc = self.pc.saturating_add(instr.size()),
                Ok(Flow::Halt) => return self.finish(None),
                Err(fault) => return self.finish(Some(fault)),
            }
        }
    }

    /// Exécute une instruction dont le gaz est déjà prélevé, en suivant D-005 : arité d'abord, puis les étapes
    /// propres à l'instruction, puis l'empilement du résultat.
    fn exec(&mut self, instr: Instr) -> Result<Flow, Fault> {
        match instr {
            Instr::Halt => return Ok(Flow::Halt),
            Instr::Const(k) => {
                let value = self.constant(usize::from(k))?;
                self.push(value)?;
            }
            Instr::PushI16(n) => self.push(i64::from(n))?,
            Instr::Drop => {
                self.pop()?;
            }
            Instr::Dup => {
                let a = self.pop()?;
                self.push(a)?;
                self.push(a)?;
            }
            Instr::Swap => {
                self.need(2)?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push(a)?;
            }
            Instr::Over => {
                self.need(2)?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a)?;
                self.push(b)?;
                self.push(a)?;
            }
            Instr::Rot => {
                // a b c -- b c a
                self.need(3)?;
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push(c)?;
                self.push(a)?;
            }
            Instr::ConstX => {
                let k = self.pop()?;
                // Un index négatif ou trop grand n'est jamais tronqué : c'est `BadConst` (cas 048, 048b).
                let index = usize::try_from(k).map_err(|_| Fault::BadConst)?;
                let value = self.constant(index)?;
                self.push(value)?;
            }
            Instr::Trace => {
                let value = self.pop()?;
                // Dans la capacité préallouée : au plus `budget` TRACE par tour.
                self.trace.push(value);
            }
            // B.7 : le gaz restant, une fois le coût de GAS lui-même prélevé.
            Instr::Gas => self.push(i64::from(self.gas_left))?,
            // Toute l'arithmétique enveloppe (§0) ; la division tronque vers zéro, comme en Rust (B.3).
            Instr::Add => self.binary(|a, b| Ok(a.wrapping_add(b)))?,
            Instr::Sub => self.binary(|a, b| Ok(a.wrapping_sub(b)))?,
            Instr::Mul => self.binary(|a, b| Ok(a.wrapping_mul(b)))?,
            Instr::Div => self.binary(div)?,
            Instr::Mod => self.binary(rem)?,
            Instr::Neg => self.unary(i64::wrapping_neg)?,
            Instr::Eq => self.binary(|a, b| Ok(i64::from(a == b)))?,
            Instr::Lt => self.binary(|a, b| Ok(i64::from(a < b)))?,
            Instr::Gt => self.binary(|a, b| Ok(i64::from(a > b)))?,
            // NOT est logique (0 ou 1), BNOT est le complément à un (B.6).
            Instr::Not => self.unary(|a| i64::from(a == 0))?,
            Instr::BNot => self.unary(|a| !a)?,
            Instr::And => self.binary(|a, b| Ok(a & b))?,
            Instr::Or => self.binary(|a, b| Ok(a | b))?,
            Instr::Xor => self.binary(|a, b| Ok(a ^ b))?,
            Instr::Shl => self.binary(|a, n| Ok(shl(a, n)))?,
            Instr::Shr => self.binary(|a, n| Ok(shr(a, n)))?,
            Instr::Sar => self.binary(|a, n| Ok(sar(a, n)))?,
            // Provisoire : les autres familles d'instructions arrivent en T06 et T07.
            _ => return Err(Fault::BadOpcode),
        }
        Ok(Flow::Next)
    }

    /// `a b -- f(a, b)` : `b` est au sommet. Les deux valeurs sont vérifiées avant d'en dépiler aucune, et
    /// l'empilement ne peut pas déborder puisqu'on vient d'en retirer deux (D-005).
    fn binary(&mut self, f: impl Fn(i64, i64) -> Result<i64, Fault>) -> Result<(), Fault> {
        self.need(2)?;
        let b = self.pop()?;
        let a = self.pop()?;
        self.push(f(a, b)?)
    }

    /// `a -- f(a)`.
    fn unary(&mut self, f: impl Fn(i64) -> i64) -> Result<(), Fault> {
        let a = self.pop()?;
        self.push(f(a))
    }

    fn constant(&self, index: usize) -> Result<i64, Fault> {
        self.program
            .constants()
            .get(index)
            .copied()
            .ok_or(Fault::BadConst)
    }

    /// `StackUnderflow` s'il y a moins de `n` valeurs : l'arité se vérifie avant tout déplacement (cas 046b).
    fn need(&self, n: usize) -> Result<(), Fault> {
        if self.sp < n {
            Err(Fault::StackUnderflow)
        } else {
            Ok(())
        }
    }

    fn pop(&mut self) -> Result<i64, Fault> {
        let top = self.sp.checked_sub(1).ok_or(Fault::StackUnderflow)?;
        let value = *self.stack.get(top).ok_or(Fault::StackUnderflow)?;
        self.sp = top;
        Ok(value)
    }

    fn push(&mut self, value: i64) -> Result<(), Fault> {
        let slot = self.stack.get_mut(self.sp).ok_or(Fault::StackOverflow)?;
        *slot = value;
        self.sp = self.sp.saturating_add(1);
        Ok(())
    }

    fn finish(self, fault: Option<Fault>) -> Step {
        Step::Done(Report {
            mem: self.mem,
            gas_used: self.budget.saturating_sub(self.gas_left),
            fault,
            act: self.act,
            trace: self.trace,
        })
    }
}

/// `DIV` : le quotient tronqué vers zéro. Une fois `b == 0` écarté, `checked_div` n'échoue que pour
/// `i64::MIN / -1`, qui enveloppe en `i64::MIN` (§0). La faute arrive après le prélèvement du gaz : elle est payée.
fn div(a: i64, b: i64) -> Result<i64, Fault> {
    if b == 0 {
        return Err(Fault::DivByZero);
    }
    Ok(a.checked_div(b).unwrap_or(i64::MIN))
}

/// `MOD` : le reste, du signe de `a`. Le seul échec restant de `checked_rem`, `i64::MIN % -1`, vaut 0.
fn rem(a: i64, b: i64) -> Result<i64, Fault> {
    if b == 0 {
        return Err(Fault::DivByZero);
    }
    Ok(a.checked_rem(b).unwrap_or(0))
}

/// Le compte d'un décalage s'il est dans `0..=63` (B.6). Il faut tester la plage avant de décaler :
/// `wrapping_shl` réduirait le compte modulo 64, et `1 SHL 64` vaudrait 1 au lieu de 0.
fn shift_count(n: i64) -> Option<u32> {
    u32::try_from(n).ok().filter(|&n| n < 64)
}

/// `SHL` logique : le motif de bits est décalé comme un `u64` (B.6), et vaut 0 hors de `0..=63`.
fn shl(a: i64, n: i64) -> i64 {
    shift_count(n)
        .and_then(|n| (a as u64).checked_shl(n))
        .map_or(0, |v| v as i64)
}

/// `SHR` logique : il ne propage pas le signe, et vaut 0 hors de `0..=63`.
fn shr(a: i64, n: i64) -> i64 {
    shift_count(n)
        .and_then(|n| (a as u64).checked_shr(n))
        .map_or(0, |v| v as i64)
}

/// `SAR` arithmétique : il propage le signe et arrondit vers moins l'infini. Hors de `0..=63`, il rend le
/// remplissage de signe : 0 si `a >= 0`, -1 sinon (B.6).
fn sar(a: i64, n: i64) -> i64 {
    let fill = if a < 0 { -1 } else { 0 };
    shift_count(n)
        .and_then(|n| a.checked_shr(n))
        .unwrap_or(fill)
}
