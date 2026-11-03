//! Le trait `Env` et `run_with`, qui pilote une machine jusqu'au bout avec un environnement local.

use pbc::Fault;

use crate::{Action, Machine, Report, Step};

/// Un environnement qui répond toujours : l'arène, le rejeu du corpus, des réponses scriptées. Le serveur réseau, lui,
/// répond aux jetons de `Step` un par un, entre deux messages (D-004).
pub trait Env {
    /// La valeur du capteur `k`, ou la faute qui termine le tour (`BadSensor` pour un capteur inconnu, E.3).
    fn sense(&mut self, k: u8) -> Result<i64, Fault>;
    /// Tente l'action et dit si elle a réussi. Un échec n'est pas une faute (E.4).
    fn act(&mut self, action: Action) -> bool;
    fn rand(&mut self) -> i64;
}

/// Pilote `machine` jusqu'à la fin du tour en posant chaque rappel à `env`, dans l'ordre de l'exécution.
pub fn run_with<E: Env + ?Sized>(machine: Machine<'_>, env: &mut E) -> Report {
    let mut step = machine.run();
    loop {
        step = match step {
            Step::Done(report) => return report,
            Step::Sense(call) => {
                let answer = env.sense(call.k());
                call.answer(answer)
            }
            Step::Act(call) => {
                let ok = env.act(call.action());
                call.answer(ok)
            }
            Step::Rand(call) => call.answer(env.rand()),
        };
    }
}
