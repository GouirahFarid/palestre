//! L'environnement de `palestre run` : des réponses données sur la ligne de commande, et le transcript des rappels.

use std::collections::{BTreeMap, VecDeque};
use std::str::FromStr;

use corpus::Call;
use pbc::Fault;
use vm::{Action, Env};

/// `SENSE k` répond la valeur de `--sense k=v`, ou `BadSensor` si `k` n'est pas fourni. `ACT` répond `--act`.
/// `RAND` consomme les `--rand` dans l'ordre, puis rend `0`.
pub struct Scripted {
    sense: BTreeMap<u8, i64>,
    act: bool,
    rand: VecDeque<i64>,
    calls: Vec<Call>,
}

impl Scripted {
    /// Pour un même capteur donné deux fois, la dernière valeur l'emporte.
    pub fn new(sense: &[(u8, i64)], act: bool, rand: &[i64]) -> Scripted {
        Scripted {
            sense: sense.iter().copied().collect(),
            act,
            rand: rand.iter().copied().collect(),
            calls: Vec::new(),
        }
    }

    /// Les rappels posés, dans l'ordre, avec la réponse donnée.
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }
}

impl Env for Scripted {
    fn sense(&mut self, k: u8) -> Result<i64, Fault> {
        let answer = self.sense.get(&k).copied().ok_or(Fault::BadSensor);
        self.calls.push(Call::Sense { k, answer });
        answer
    }

    fn act(&mut self, action: Action) -> bool {
        self.calls.push(Call::Act {
            action,
            ok: self.act,
        });
        self.act
    }

    fn rand(&mut self) -> i64 {
        let value = self.rand.pop_front().unwrap_or(0);
        self.calls.push(Call::Rand { value });
        value
    }
}

/// Lit `K=V`, pour `--mem` et `--sense`.
pub fn pair<K: FromStr, V: FromStr>(text: &str) -> Result<(K, V), String> {
    let (k, v) = text
        .split_once('=')
        .ok_or_else(|| format!("« {text} » : K=V attendu"))?;
    let k = k
        .trim()
        .parse()
        .map_err(|_| format!("« {k} » : indice invalide"))?;
    let v = v
        .trim()
        .parse()
        .map_err(|_| format!("« {v} » : valeur invalide"))?;
    Ok((k, v))
}
