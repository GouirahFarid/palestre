//! Les réglages d'un match.

use std::num::NonZeroU16;

/// Les constantes de E.2, E.4 et E.5, plus ce que D-010 ajoute (énergie initiale, graine).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Gaz de chaque agent à chaque tour (E.2).
    pub gas_budget: NonZeroU16,
    /// Dernier tour de la partie (E.5).
    pub turns_max: u32,
    /// Énergie gagnée par charge mangée (E.4).
    pub eat_rate: i64,
    /// Énergie de départ des deux agents ; E.1 n'existe pas (D-010).
    pub initial_energy: i64,
    /// Graine du SplitMix64 qui répond aux `RAND` (D-010).
    pub seed: u64,
}

const GAS_BUDGET: NonZeroU16 = match NonZeroU16::new(1000) {
    Some(budget) => budget,
    None => NonZeroU16::MIN,
};

impl Default for Config {
    fn default() -> Config {
        Config {
            gas_budget: GAS_BUDGET,
            turns_max: 500,
            eat_rate: 10,
            initial_energy: 100,
            seed: 0,
        }
    }
}
