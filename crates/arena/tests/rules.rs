#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use arena::{AgentView, Arena, Config, Map, MatchLog, SplitMix64, play, winner};
use pbc::{Fault, Program};
use vm::{ActKind, ActRecord, Action};

fn agent(src: &str) -> Program {
    asm::assemble(src).unwrap()
}

fn idle() -> Program {
    agent("HALT\n")
}

fn one_turn(map: &str, src: &str) -> MatchLog {
    play(
        [&agent(src), &idle()],
        &Map::parse(map).unwrap(),
        &Config {
            turns_max: 1,
            ..Config::default()
        },
    )
}

const ALL_SENSORS: &str = "SENSE 0\nTRACE\nSENSE 1\nTRACE\nSENSE 2\nTRACE\nSENSE 3\nTRACE\nSENSE 4\nTRACE\n\
SENSE 5\nTRACE\nSENSE 6\nTRACE\nSENSE 7\nTRACE\nSENSE 8\nTRACE\nSENSE 9\nTRACE\nSENSE 10\nTRACE\n\
SENSE 11\nTRACE\nSENSE 12\nTRACE\nSENSE 13\nTRACE\nHALT\n";

#[test]
fn sensors_read_the_world() {
    // A en (0,0), une ressource 3 à l'est, B en (2,2), un mur en (1,1). turns_max = 1.
    let log = one_turn("A3.\n.#.\n..B\n", ALL_SENSORS);
    // x y énergie porté ici tour id distance voisinage ressources x_max y_max turns_max agents
    assert_eq!(
        log.turns[0].agents[0].trace,
        vec![0, 0, 100, 0, 0, 1, 0, 4, 9, 2, 2, 2, 1, 0]
    );
}

#[test]
fn walls_and_agents_in_the_masks() {
    // B à l'est de A, un mur au sud de A.
    let log = one_turn("AB\n#.\n", "SENSE 8\nTRACE\nSENSE 13\nTRACE\nHALT\n");
    assert_eq!(log.turns[0].agents[0].trace, vec![1 | 4 | 8, 2]);
}

#[test]
fn an_unknown_sensor_is_a_fault() {
    let log = one_turn("A..\n..B\n", "SENSE 14\nHALT\n");
    assert_eq!(log.turns[0].agents[0].fault, Some(Fault::BadSensor));
}

#[test]
fn a_move_costs_one_extra_energy() {
    let log = one_turn("A..\n..B\n", "PUSHI16 1\nACT 0\nTRACE\nHALT\n");
    let a = log.final_state.agents[0];
    assert_eq!((a.x, a.y, a.energy), (1, 0, 98));
    assert_eq!(log.turns[0].agents[0].trace, vec![1]);
}

#[test]
fn blocked_moves_fail_without_extra_cost() {
    // bord nord, mur à l'est, agent à l'est, direction invalide
    for (map, dir) in [
        ("A..\n..B\n", 0),
        ("A#.\n..B\n", 1),
        ("AB.\n...\n", 1),
        ("A..\n..B\n", 7),
    ] {
        let log = one_turn(map, &format!("PUSHI16 {dir}\nACT 0\nTRACE\nHALT\n"));
        let a = log.final_state.agents[0];
        assert_eq!((a.x, a.y, a.energy), (0, 0, 99), "{map:?} direction {dir}");
        assert_eq!(log.turns[0].agents[0].trace, vec![0]);
    }
}

#[test]
fn take_picks_at_most_five() {
    let taker = agent(
        "SENSE 4\nJZ bouge\nPUSHI16 0\nACT 1\nHALT\nbouge:\nJUMPDEST\nPUSHI16 1\nACT 0\nHALT\n",
    );
    let config = Config {
        turns_max: 2,
        ..Config::default()
    };
    let log = play(
        [&taker, &idle()],
        &Map::parse("A7.\n..B\n").unwrap(),
        &config,
    );
    assert_eq!(log.final_state.agents[0].carried, 5);
    assert_eq!(log.final_state.resources, vec![(1, 0, 2)]);
}

#[test]
fn drop_and_eat_clamp_to_what_is_carried() {
    let map = Map::parse("A..\n..B\n").unwrap();
    let idler = idle();

    let dropper = agent("PUSHI16 10\nACT 2\nTRACE\nHALT\n");
    let mut arena = Arena::new([&dropper, &idler], &map, &Config::default());
    arena.world_mut().agents[0].carried = 3;
    arena.play_turn();
    assert_eq!(
        (
            arena.world().agents[0].carried,
            arena.world().resource_at(0, 0)
        ),
        (0, 3)
    );

    let eater = agent("PUSHI16 2\nACT 3\nTRACE\nHALT\n");
    let mut arena = Arena::new([&eater, &idler], &map, &Config::default());
    arena.world_mut().agents[0].carried = 3;
    arena.play_turn();
    let a = &arena.world().agents[0];
    assert_eq!((a.carried, a.energy), (1, 100 + 2 * 10 - 1));

    let negative = agent("PUSHI16 -1\nACT 2\nTRACE\nHALT\n");
    let mut arena = Arena::new([&negative, &idler], &map, &Config::default());
    arena.world_mut().agents[0].carried = 3;
    let turn = arena.play_turn().clone();
    assert_eq!(turn.agents[0].trace, vec![0]);
    assert_eq!(arena.world().agents[0].carried, 3);
}

#[test]
fn a_fault_restores_the_agent_and_the_world() {
    // D-011 : mémoire écrite, déplacement réussi, puis faute → tout revient à l'instantané, sauf l'enregistrement.
    let faulty = agent("PUSHI16 7\nSTORE 0\nPUSHI16 1\nACT 0\nDROP\nDROP\n");
    let idler = idle();
    let mut arena = Arena::new(
        [&faulty, &idler],
        &Map::parse("A..\n..B\n").unwrap(),
        &Config::default(),
    );
    let turn = arena.play_turn().clone();
    let a = &arena.world().agents[0];
    assert_eq!((a.x, a.y, a.energy, a.mem[0]), (0, 0, 99, 0));
    assert_eq!(turn.agents[0].fault, Some(Fault::StackUnderflow));
    assert_eq!(
        turn.agents[0].act,
        Some(ActRecord {
            action: Action {
                kind: ActKind::Move,
                arg: 1
            },
            ok: true
        })
    );
}

#[test]
fn memory_persists_between_turns() {
    let counter = agent("LOAD 0\nPUSHI16 1\nADD\nDUP\nSTORE 0\nTRACE\nHALT\n");
    let config = Config {
        turns_max: 3,
        ..Config::default()
    };
    let log = play(
        [&counter, &idle()],
        &Map::parse("A..\n..B\n").unwrap(),
        &config,
    );
    let traces: Vec<Vec<i64>> = log
        .turns
        .iter()
        .map(|t| t.agents[0].trace.clone())
        .collect();
    assert_eq!(traces, vec![vec![1], vec![2], vec![3]]);
}

#[test]
fn agents_play_in_increasing_id_order() {
    let who = agent("SENSE 6\nTRACE\nHALT\n");
    let log = play(
        [&who, &who],
        &Map::parse("A..\n..B\n").unwrap(),
        &Config {
            turns_max: 1,
            ..Config::default()
        },
    );
    let ids: Vec<i64> = log.turns[0].agents.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![0, 1]);
    assert_eq!(log.turns[0].agents[1].trace, vec![1]);
}

#[test]
fn a_fault_still_costs_the_turn_energy() {
    let log = one_turn("A..\n..B\n", "DROP\n");
    assert_eq!(log.final_state.agents[0].energy, 99);
}

#[test]
fn energy_runs_out_and_the_match_ends() {
    let idler = idle();
    let config = Config {
        initial_energy: 2,
        ..Config::default()
    };
    let log = play(
        [&idler, &idler],
        &Map::parse("A..\n..B\n").unwrap(),
        &config,
    );
    assert_eq!(log.turns.len(), 2);
    assert!(log.final_state.agents.iter().all(|a| !a.alive));
    assert_eq!(log.winner, None);
}

#[test]
fn a_dying_agent_drops_what_it_carries() {
    let idler = idle();
    let config = Config {
        initial_energy: 1,
        ..Config::default()
    };
    let mut arena = Arena::new(
        [&idler, &idler],
        &Map::parse("A..\n..B\n").unwrap(),
        &config,
    );
    arena.world_mut().agents[0].carried = 4;
    arena.play_turn();
    let a = &arena.world().agents[0];
    assert!(!a.alive);
    assert_eq!(a.carried, 0);
    assert_eq!(arena.world().resource_at(0, 0), 4);
}

#[test]
fn winner_by_carried_then_energy_never_by_id() {
    let base = AgentView {
        id: 0,
        x: 0,
        y: 0,
        energy: 5,
        carried: 3,
        alive: true,
    };
    assert_eq!(
        winner(&[
            base,
            AgentView {
                id: 1,
                carried: 2,
                energy: 50,
                ..base
            }
        ]),
        Some(0)
    );
    assert_eq!(
        winner(&[
            base,
            AgentView {
                id: 1,
                energy: 6,
                ..base
            }
        ]),
        Some(1)
    );
    assert_eq!(winner(&[base, AgentView { id: 1, ..base }]), None);
}

#[test]
fn splitmix64_matches_the_reference_sequence() {
    let mut rng = SplitMix64::new(0);
    assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
}

#[test]
fn rand_draws_from_the_seeded_generator() {
    let log = one_turn("A..\n..B\n", "RAND\nTRACE\nHALT\n");
    assert_eq!(
        log.turns[0].agents[0].trace,
        vec![0xE220_A839_7B1D_CDAF_u64 as i64]
    );
}

#[test]
fn matches_are_deterministic_and_their_log_round_trips() {
    let walker = agent("RAND\nPUSHI16 3\nAND\nACT 0\nHALT\n");
    let map = Map::parse("A....\n.#.#.\n....B\n").unwrap();
    let config = Config {
        turns_max: 30,
        seed: 7,
        ..Config::default()
    };
    let first = play([&walker, &walker], &map, &config);
    assert_eq!(play([&walker, &walker], &map, &config), first);
    assert_eq!(
        MatchLog::from_json(&first.to_json().unwrap()).unwrap(),
        first
    );
}
