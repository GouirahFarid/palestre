//! L'arène : le monde de l'annexe E, les tours, et le match de deux agents (D-010, D-011).
#![forbid(unsafe_code)]

mod config;
mod log;
mod map;
mod rng;
mod turn;
mod world;

pub use config::Config;
pub use log::{AgentTurn, AgentView, LogError, MatchLog, TurnLog, WorldSnapshot};
pub use map::{Map, MapError};
pub use rng::SplitMix64;
pub use turn::{Arena, play, winner};
pub use world::{AgentState, World};
