#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use corpus::{Case, Expectation, Replay, load_corpus};
use exec::client::Client;
use exec::frame::{FrameType, write_frame};
use exec::server::{ServerConfig, serve, serve_concurrent};

/// Enregistre tout ce que le serveur envoie.
struct Recorder {
    inner: TcpStream,
    received: Vec<u8>,
}

impl Read for Recorder {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.received.extend_from_slice(&buf[..n]);
        Ok(n)
    }
}

impl Write for Recorder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn start(concurrent: bool, config: ServerConfig) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        if concurrent {
            serve_concurrent(listener, &config)
        } else {
            serve(listener, &config)
        }
    });
    addr
}

fn cases() -> Vec<Case> {
    load_corpus(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus")).unwrap()
}

/// Octets reçus du serveur pour chaque exécution de chaque cas exécutable.
fn transcripts(addr: SocketAddr, cases: &[Case]) -> Vec<Vec<u8>> {
    let mut all = Vec::new();
    for case in cases {
        let Expectation::Runs(execs) = &case.expectation else {
            continue;
        };
        for exec in execs {
            let stream = TcpStream::connect(addr).unwrap();
            let mut client = Client::over(Recorder {
                inner: stream,
                received: Vec::new(),
            })
            .unwrap();
            let session = client.submit(&case.program).unwrap();
            let mut replay = Replay::new(&exec.answers);
            client
                .exec(&session.session, &exec.mem, exec.budget, &mut replay)
                .unwrap();
            all.push(client.into_inner().received);
        }
    }
    all
}

#[test]
fn transcripts_are_byte_identical_to_the_sequential_server() {
    let cases = cases();
    let sequential = transcripts(start(false, ServerConfig::default()), &cases);
    let concurrent = transcripts(start(true, ServerConfig::default()), &cases);
    assert_eq!(sequential.len(), 68);
    assert_eq!(sequential, concurrent);
}

#[test]
fn parallel_clients_receive_the_same_bytes() {
    let cases = cases();
    let reference = transcripts(start(false, ServerConfig::default()), &cases);
    let addr = start(true, ServerConfig::default());
    thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| transcripts(addr, &cases)))
            .collect();
        for handle in handles {
            assert_eq!(handle.join().unwrap(), reference);
        }
    });
}

#[test]
fn a_silent_client_does_not_block_anyone() {
    // Aucun délai de lecture : un serveur séquentiel resterait bloqué indéfiniment.
    let addr = start(
        true,
        ServerConfig {
            read_timeout: None,
            ..ServerConfig::default()
        },
    );
    let _silent = TcpStream::connect(addr).unwrap();
    let started = Instant::now();
    let client = Client::connect(addr).unwrap();
    assert_eq!(client.ready().proto, 1);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn a_client_stuck_mid_frame_does_not_block_anyone() {
    let addr = start(
        true,
        ServerConfig {
            read_timeout: None,
            ..ServerConfig::default()
        },
    );
    let mut stuck = TcpStream::connect(addr).unwrap();
    write_frame(&mut stuck, FrameType::Open, br#"{"proto":1}"#).unwrap();
    stuck
        .write_all(&[0x13, 0x00, 0x00, 0x00, 0x20, 0x50]) // SUBMIT commencé, jamais fini
        .unwrap();
    let started = Instant::now();
    let client = Client::connect(addr).unwrap();
    assert_eq!(client.ready().proto, 1);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn a_client_killed_mid_frame_bothers_nobody() {
    let addr = start(
        true,
        ServerConfig {
            read_timeout: None,
            ..ServerConfig::default()
        },
    );
    {
        let mut dying = TcpStream::connect(addr).unwrap();
        write_frame(&mut dying, FrameType::Open, br#"{"proto":1}"#).unwrap();
        dying
            .write_all(&[0x13, 0x00, 0x00, 0x00, 0x20, 0x50]) // SUBMIT coupé
            .unwrap();
    }
    assert_eq!(Client::connect(addr).unwrap().ready().proto, 1);
}
