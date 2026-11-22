//! Le déroulement d'un tour (E.2), les capteurs (E.3), les actions (E.4) et la fin de partie (E.5).

use pbc::{Fault, Program};
use vm::{ActKind, Action, Env, Machine, run_with};

use crate::config::Config;
use crate::log::{AgentTurn, AgentView, MatchLog, TurnLog, WorldSnapshot};
use crate::map::Map;
use crate::rng::SplitMix64;
use crate::world::{AgentState, World};

/// Les directions de `MOVE`, dans l'ordre des bits des masques 8, 9 et 13 : nord, est, sud, ouest.
const DIRECTIONS: [(i64, i64); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// Ce que `TAKE` ramasse au plus (E.4).
const TAKE_MAX: i64 = 5;

/// Un match en cours.
pub struct Arena<'p> {
    programs: [&'p Program; 2],
    world: World,
    config: Config,
    rng: SplitMix64,
    /// L'état au début du premier tour, pris au premier `play_turn`.
    start: Option<WorldSnapshot>,
    turns: Vec<TurnLog>,
}

impl<'p> Arena<'p> {
    pub fn new(programs: [&'p Program; 2], map: &Map, config: &Config) -> Arena<'p> {
        Arena {
            programs,
            world: World::new(map, config),
            config: config.clone(),
            rng: SplitMix64::new(config.seed),
            start: None,
            turns: Vec::new(),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Joue le tour suivant (E.2) et rend son journal.
    pub fn play_turn(&mut self) -> &TurnLog {
        if self.start.is_none() {
            self.start = Some(self.world.snapshot());
        }
        self.world.turn = self.world.turn.saturating_add(1);
        let mut agents = Vec::new();
        for me in 0..self.world.agents.len() {
            if !self.world.agents[me].alive {
                continue;
            }
            // 1. Instantané. Le générateur n'en fait pas partie : il appartient à l'environnement (D-010).
            let snapshot = self.world.clone();
            // 2 et 3. Une machine neuve, avec la mémoire de l'agent, jusqu'à HALT ou faute.
            let program = self.programs[me];
            let machine = Machine::new(program, self.world.agents[me].mem, self.config.gas_budget);
            let mut env = TurnEnv {
                world: &mut self.world,
                me,
                config: &self.config,
                rng: &mut self.rng,
            };
            let report = run_with(machine, &mut env);
            // 4. Une faute restaure l'agent et le monde ; l'action tentée reste dans le journal (D-011).
            if report.fault.is_some() {
                self.world = snapshot;
            } else {
                self.world.agents[me].mem = report.mem;
            }
            agents.push(AgentTurn {
                id: self.world.agents[me].id,
                gas_used: report.gas_used,
                fault: report.fault,
                act: report.act,
                trace: report.trace,
            });
            // 5 et 6. Le décrément du tour, même après une faute, puis la mort éventuelle.
            let agent = &mut self.world.agents[me];
            agent.energy = agent.energy.saturating_sub(1);
            if agent.energy <= 0 {
                agent.alive = false;
                let (x, y, carried) = (agent.x, agent.y, agent.carried);
                agent.carried = 0;
                let here = self.world.resource_at(x, y);
                self.world.set_resource(x, y, here.saturating_add(carried));
            }
        }
        let index = self.turns.len();
        self.turns.push(TurnLog {
            turn: self.world.turn,
            agents,
            after: self.world.snapshot(),
        });
        &self.turns[index]
    }

    /// `turn >= turns_max`, ou au plus un agent vivant. Testé après un tour entier (E.5).
    pub fn is_over(&self) -> bool {
        self.world.turn >= self.config.turns_max
            || self.world.agents.iter().filter(|a| a.alive).count() <= 1
    }

    pub fn finish(self) -> MatchLog {
        let final_state = self.world.snapshot();
        MatchLog {
            width: self.world.map().width(),
            height: self.world.map().height(),
            walls: self.world.walls(),
            start: self.start.unwrap_or_else(|| final_state.clone()),
            turns: self.turns,
            winner: winner(&final_state.agents),
            final_state,
        }
    }
}

/// Joue un match entier.
pub fn play(programs: [&Program; 2], map: &Map, config: &Config) -> MatchLog {
    let mut arena = Arena::new(programs, map, config);
    while !arena.is_over() {
        arena.play_turn();
    }
    arena.finish()
}

/// E.5 : le plus grand `carried`, puis la plus grande `energy` ; à égalité, match nul. L'identifiant ne départage
/// jamais.
pub fn winner(agents: &[AgentView]) -> Option<i64> {
    let score = |a: &AgentView| (a.carried, a.energy);
    let best = agents.iter().map(score).max()?;
    let mut leaders = agents.iter().filter(|a| score(a) == best);
    match (leaders.next(), leaders.next()) {
        (Some(leader), None) => Some(leader.id),
        _ => None,
    }
}

/// L'environnement d'un agent pendant son tour : le monde, vu depuis lui.
struct TurnEnv<'a> {
    world: &'a mut World,
    me: usize,
    config: &'a Config,
    rng: &'a mut SplitMix64,
}

impl TurnEnv<'_> {
    fn me(&self) -> &AgentState {
        &self.world.agents[self.me]
    }

    fn me_mut(&mut self) -> &mut AgentState {
        &mut self.world.agents[self.me]
    }

    /// L'autre agent : il n'y en a que deux.
    fn other(&self) -> &AgentState {
        &self.world.agents[1 - self.me]
    }

    fn living_agent_at(&self, x: i64, y: i64) -> bool {
        let other = self.other();
        other.alive && other.x == x && other.y == y
    }

    /// Un bit par voisin qui vérifie `test`, dans l'ordre nord, est, sud, ouest (E.3).
    fn mask(&self, test: impl Fn(i64, i64) -> bool) -> i64 {
        let (x, y) = (self.me().x, self.me().y);
        DIRECTIONS
            .iter()
            .zip(0..)
            .filter(|((dx, dy), _)| test(x + dx, y + dy))
            .map(|(_, bit)| 1_i64 << bit)
            .sum()
    }
}

impl Env for TurnEnv<'_> {
    fn sense(&mut self, k: u8) -> Result<i64, Fault> {
        let me = self.me();
        let other = self.other();
        let map = self.world.map();
        Ok(match k {
            0 => me.x,
            1 => me.y,
            2 => me.energy,
            3 => me.carried,
            4 => map.resource(me.x, me.y),
            5 => i64::from(self.world.turn),
            6 => me.id,
            7 if other.alive => (me.x - other.x).abs() + (me.y - other.y).abs(),
            7 => -1,
            8 => self.mask(|x, y| !map.in_bounds(x, y) || map.is_wall(x, y)),
            9 => self.mask(|x, y| map.resource(x, y) > 0),
            10 => map.width() - 1,
            11 => map.height() - 1,
            12 => i64::from(self.config.turns_max),
            13 => self.mask(|x, y| self.living_agent_at(x, y)),
            _ => return Err(Fault::BadSensor),
        })
    }

    fn act(&mut self, action: Action) -> bool {
        let (x, y) = (self.me().x, self.me().y);
        match action.kind {
            ActKind::Move => {
                let direction = usize::try_from(action.arg)
                    .ok()
                    .and_then(|d| DIRECTIONS.get(d));
                let Some(&(dx, dy)) = direction else {
                    return false;
                };
                let (to_x, to_y) = (x + dx, y + dy);
                let map = self.world.map();
                if !map.in_bounds(to_x, to_y)
                    || map.is_wall(to_x, to_y)
                    || self.living_agent_at(to_x, to_y)
                {
                    return false;
                }
                let me = self.me_mut();
                me.x = to_x;
                me.y = to_y;
                me.energy = me.energy.saturating_sub(1);
                true
            }
            // Jamais d'échec, même sur une case vide : E.4 n'en prévoit pas (D-011).
            ActKind::Take => {
                let here = self.world.resource_at(x, y);
                let quantity = here.min(TAKE_MAX);
                self.world.set_resource(x, y, here - quantity);
                let me = self.me_mut();
                me.carried = me.carried.saturating_add(quantity);
                true
            }
            ActKind::Drop => {
                if action.arg < 0 {
                    return false;
                }
                let quantity = action.arg.min(self.me().carried);
                self.me_mut().carried -= quantity;
                let here = self.world.resource_at(x, y);
                self.world.set_resource(x, y, here.saturating_add(quantity));
                true
            }
            ActKind::Eat => {
                if action.arg < 0 {
                    return false;
                }
                let eat_rate = self.config.eat_rate;
                let me = self.me_mut();
                let quantity = action.arg.min(me.carried);
                me.carried -= quantity;
                me.energy = me.energy.saturating_add(eat_rate.saturating_mul(quantity));
                true
            }
        }
    }

    /// Le flux du générateur, réinterprété en `i64` (complément à deux).
    fn rand(&mut self) -> i64 {
        self.rng.next_u64() as i64
    }
}
