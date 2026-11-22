#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use arena::Map;

#[test]
fn the_text_format() {
    let map = Map::parse("A.3\n.#.\n..B\n").unwrap();
    assert_eq!((map.width(), map.height()), (3, 3));
    assert!(map.is_wall(1, 1));
    assert!(!map.is_wall(0, 1));
    assert!(!map.is_wall(-1, 0));
    assert!(!map.in_bounds(3, 0));
    assert_eq!(map.resource(2, 0), 3);
    assert_eq!(map.resource(9, 9), 0);
    assert_eq!(map.starts(), [(0, 0), (2, 2)]);
}

#[test]
fn carriage_returns_and_a_final_newline_are_tolerated() {
    assert_eq!(Map::parse("A.\r\n.B\r\n").unwrap().width(), 2);
}

#[test]
fn malformed_maps_are_refused() {
    for bad in [
        "",
        "A.\n.\n",
        "A..\n...\n",
        "AA.\n..B\n",
        "A.B\n..B\n",
        "A.x\n..B\n",
    ] {
        assert!(Map::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_default_map_is_point_symmetric() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../maps/default.map"
    ))
    .unwrap();
    let map = Map::parse(&text).unwrap();
    let (w, h) = (map.width(), map.height());
    for y in 0..h {
        for x in 0..w {
            assert_eq!(
                map.is_wall(x, y),
                map.is_wall(w - 1 - x, h - 1 - y),
                "mur ({x}, {y})"
            );
            assert_eq!(
                map.resource(x, y),
                map.resource(w - 1 - x, h - 1 - y),
                "ressource ({x}, {y})"
            );
        }
    }
}
