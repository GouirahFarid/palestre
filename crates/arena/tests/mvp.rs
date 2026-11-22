#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use arena::{Config, Map, play};
use vm::{ActKind, ActRecord, Action};

fn load(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn the_harvester_beats_the_idler_on_the_default_map() {
    let harvester = asm::assemble(&load("agents/harvester.asm")).unwrap();
    let idler = asm::assemble(&load("agents/idler.asm")).unwrap();
    let map = Map::parse(&load("maps/default.map")).unwrap();
    let log = play(
        [&harvester, &idler],
        &map,
        &Config {
            seed: 1,
            ..Config::default()
        },
    );
    assert_eq!(log.winner, Some(0));
    let moves = log.turns.iter().flat_map(|t| &t.agents);
    let took = moves.clone().any(|a| {
        a.id == 0
            && matches!(
                a.act,
                Some(ActRecord {
                    action: Action {
                        kind: ActKind::Take,
                        ..
                    },
                    ok: true
                })
            )
    });
    assert!(took, "le récolteur n'a jamais rien ramassé");
    assert!(
        moves.clone().all(|a| a.fault.is_none()),
        "un agent de démonstration ne doit jamais fauter"
    );
}
