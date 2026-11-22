//! L'état du monde pendant un match : les deux agents, le tour, et les ressources qui bougent.

use vm::{MEM_SIZE, Memory};

use crate::config::Config;
use crate::log::{AgentView, WorldSnapshot};
use crate::map::Map;

/// Un agent : sa position, son énergie, ce qu'il porte, et sa mémoire qui persiste d'un tour à l'autre (B.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentState {
    pub id: i64,
    pub x: i64,
    pub y: i64,
    pub energy: i64,
    pub carried: i64,
    pub alive: bool,
    pub mem: Memory,
}

/// Tout ce qu'un instantané de E.2 doit restaurer : les agents et les ressources. Les murs ne bougent pas, mais ils
/// voyagent avec la carte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    pub agents: [AgentState; 2],
    /// Le tour en cours, ou le dernier joué ; 0 avant le premier.
    pub turn: u32,
    map: Map,
}

impl World {
    pub(crate) fn new(map: &Map, config: &Config) -> World {
        let agent = |id: i64, (x, y): (i64, i64)| AgentState {
            id,
            x,
            y,
            energy: config.initial_energy,
            carried: 0,
            alive: true,
            mem: [0; MEM_SIZE],
        };
        let [a, b] = map.starts();
        World {
            agents: [agent(0, a), agent(1, b)],
            turn: 0,
            map: map.clone(),
        }
    }

    pub fn map(&self) -> &Map {
        &self.map
    }

    pub fn resource_at(&self, x: i64, y: i64) -> i64 {
        self.map.resource(x, y)
    }

    pub fn set_resource(&mut self, x: i64, y: i64, quantity: i64) {
        self.map.set_resource(x, y, quantity);
    }

    /// Ce que le journal garde : les agents, et les ressources non nulles triées par `(y, x)`.
    pub(crate) fn snapshot(&self) -> WorldSnapshot {
        let agents = self
            .agents
            .iter()
            .map(|a| AgentView {
                id: a.id,
                x: a.x,
                y: a.y,
                energy: a.energy,
                carried: a.carried,
                alive: a.alive,
            })
            .collect();
        let resources = self
            .cells()
            .filter_map(|(x, y)| {
                let quantity = self.map.resource(x, y);
                (quantity > 0).then_some((x, y, quantity))
            })
            .collect();
        WorldSnapshot { agents, resources }
    }

    /// Les cases murées, triées par `(y, x)`.
    pub(crate) fn walls(&self) -> Vec<(i64, i64)> {
        self.cells()
            .filter(|&(x, y)| self.map.is_wall(x, y))
            .collect()
    }

    /// Toutes les cases, rangée par rangée.
    fn cells(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        (0..self.map.height()).flat_map(move |y| (0..self.map.width()).map(move |x| (x, y)))
    }
}
