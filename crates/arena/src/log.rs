//! Le journal d'un match : tout ce que le rejeu visuel (B5) affiche, sans recalculer les règles.
//!
//! `pbc` et `vm` restent sans `serde` (D-001) : le JSON passe par des structures miroir privées, où fautes et
//! actions sont écrites par leur nom.

use pbc::Fault;
use serde::{Deserialize, Serialize};
use vm::{ActKind, ActRecord, Action};

/// Un agent vu de l'extérieur, sans sa mémoire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentView {
    pub id: i64,
    pub x: i64,
    pub y: i64,
    pub energy: i64,
    pub carried: i64,
    pub alive: bool,
}

/// Les agents, et les ressources non nulles `(x, y, quantité)` triées par `(y, x)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub agents: Vec<AgentView>,
    pub resources: Vec<(i64, i64, i64)>,
}

/// Ce qu'un agent a fait pendant son tour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurn {
    pub id: i64,
    pub gas_used: u16,
    pub fault: Option<Fault>,
    /// L'action tentée, gardée même si l'agent a fauté ensuite (D-011).
    pub act: Option<ActRecord>,
    pub trace: Vec<i64>,
}

/// Un tour : les agents vivants, dans l'ordre où ils ont joué, puis l'état du monde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnLog {
    pub turn: u32,
    pub agents: Vec<AgentTurn>,
    pub after: WorldSnapshot,
}

/// Un match entier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchLog {
    pub width: i64,
    pub height: i64,
    pub walls: Vec<(i64, i64)>,
    pub start: WorldSnapshot,
    pub turns: Vec<TurnLog>,
    pub final_state: WorldSnapshot,
    /// `None` : match nul.
    pub winner: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("JSON invalide : {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

impl MatchLog {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&MatchJson::from(self))
    }

    pub fn from_json(text: &str) -> Result<MatchLog, LogError> {
        serde_json::from_str::<MatchJson>(text)?.into_log()
    }
}

#[derive(Serialize, Deserialize)]
struct MatchJson {
    width: i64,
    height: i64,
    walls: Vec<(i64, i64)>,
    start: WorldSnapshot,
    turns: Vec<TurnJson>,
    final_state: WorldSnapshot,
    winner: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct TurnJson {
    turn: u32,
    agents: Vec<AgentTurnJson>,
    after: WorldSnapshot,
}

#[derive(Serialize, Deserialize)]
struct AgentTurnJson {
    id: i64,
    gas_used: u16,
    fault: Option<String>,
    act: Option<ActJson>,
    trace: Vec<i64>,
}

#[derive(Serialize, Deserialize)]
struct ActJson {
    kind: String,
    arg: i64,
    ok: bool,
}

impl From<&MatchLog> for MatchJson {
    fn from(log: &MatchLog) -> MatchJson {
        MatchJson {
            width: log.width,
            height: log.height,
            walls: log.walls.clone(),
            start: log.start.clone(),
            turns: log
                .turns
                .iter()
                .map(|turn| TurnJson {
                    turn: turn.turn,
                    agents: turn.agents.iter().map(AgentTurnJson::from).collect(),
                    after: turn.after.clone(),
                })
                .collect(),
            final_state: log.final_state.clone(),
            winner: log.winner,
        }
    }
}

impl From<&AgentTurn> for AgentTurnJson {
    fn from(agent: &AgentTurn) -> AgentTurnJson {
        AgentTurnJson {
            id: agent.id,
            gas_used: agent.gas_used,
            fault: agent.fault.map(|fault| fault.as_str().to_string()),
            act: agent.act.map(|record| ActJson {
                kind: record.action.kind.name().to_string(),
                arg: record.action.arg,
                ok: record.ok,
            }),
            trace: agent.trace.clone(),
        }
    }
}

impl MatchJson {
    fn into_log(self) -> Result<MatchLog, LogError> {
        let turns = self
            .turns
            .into_iter()
            .map(|turn| {
                Ok(TurnLog {
                    turn: turn.turn,
                    agents: turn
                        .agents
                        .into_iter()
                        .map(AgentTurnJson::into_turn)
                        .collect::<Result<_, LogError>>()?,
                    after: turn.after,
                })
            })
            .collect::<Result<_, LogError>>()?;
        Ok(MatchLog {
            width: self.width,
            height: self.height,
            walls: self.walls,
            start: self.start,
            turns,
            final_state: self.final_state,
            winner: self.winner,
        })
    }
}

impl AgentTurnJson {
    fn into_turn(self) -> Result<AgentTurn, LogError> {
        let fault = self
            .fault
            .map(|name| {
                Fault::from_name(&name)
                    .ok_or_else(|| LogError::Invalid(format!("faute inconnue « {name} »")))
            })
            .transpose()?;
        let act = self
            .act
            .map(|act| {
                let kind = ActKind::from_name(&act.kind).ok_or_else(|| {
                    LogError::Invalid(format!("action inconnue « {} »", act.kind))
                })?;
                Ok::<_, LogError>(ActRecord {
                    action: Action { kind, arg: act.arg },
                    ok: act.ok,
                })
            })
            .transpose()?;
        Ok(AgentTurn {
            id: self.id,
            gas_used: self.gas_used,
            fault,
            act,
            trace: self.trace,
        })
    }
}
