#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use corpus::{Remote, check_case, load_corpus};
use exec::server::{ServerConfig, serve};

#[test]
fn the_public_corpus_passes_against_our_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || serve(listener, &ServerConfig::default()));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let cases = load_corpus(&root).unwrap();
    let mut backend = Remote::new(addr);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|c| check_case(c, &mut backend).err())
        .map(|f| f.to_string())
        .collect();
    assert!(
        failures.is_empty(),
        "{} cas en échec :\n{}",
        failures.len(),
        failures.join("\n")
    );
}
