//! `palestre` : la ligne de commande du projet.

use std::io::{self, Write};
use std::net::{TcpListener, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use corpus::{Backend, InProcess, Remote, check_case, load_corpus};
use exec::server::{ServerConfig, serve};

#[derive(Parser)]
#[command(
    name = "palestre",
    version,
    about = "Machine virtuelle PBC v1 et ses outils"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sert le protocole d'exécution distante (annexe K), une connexion après l'autre.
    Exec {
        /// Adresse d'écoute, par exemple 127.0.0.1:9000 (port 0 : un port libre choisi par le système).
        #[arg(long)]
        listen: String,
        /// Nombre de sessions qu'une connexion peut ouvrir, annoncé dans READY.
        #[arg(long, default_value_t = 16)]
        sessions_max: u32,
        /// Secondes d'attente d'un client muet avant de le déconnecter ; 0 pour attendre sans limite.
        #[arg(long, default_value_t = 30)]
        read_timeout: u64,
    },
    /// Rejoue les cas d'un corpus (`corpus/FORMAT.md`), dans notre VM ou contre un serveur `palestre exec`.
    Corpus {
        /// Dossier du corpus, qui contient `edge/` et `nominal/`.
        dir: PathBuf,
        /// Adresse d'un serveur `palestre exec` (hôte:port) ; sans elle, les cas tournent dans notre VM.
        #[arg(long)]
        server: Option<String>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Exec {
            listen,
            sessions_max,
            read_timeout,
        } => exec(
            &listen,
            &ServerConfig {
                sessions_max,
                read_timeout: (read_timeout > 0).then(|| Duration::from_secs(read_timeout)),
            },
        ),
        Command::Corpus { dir, server } => corpus(&dir, server.as_deref()),
    }
}

/// Une ligne `OK`/`KO` par cas, puis le bilan. Le code de sortie vaut 1 dès qu'un cas échoue.
fn corpus(dir: &Path, server: Option<&str>) -> ExitCode {
    let cases = match load_corpus(dir) {
        Ok(cases) => cases,
        Err(error) => {
            eprintln!("palestre corpus : {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut backend: Box<dyn Backend> = match server {
        None => Box::new(InProcess),
        Some(server) => match server.to_socket_addrs().map(|mut addrs| addrs.next()) {
            Ok(Some(addr)) => Box::new(Remote::new(addr)),
            Ok(None) => {
                eprintln!("palestre corpus : {server} ne désigne aucune adresse");
                return ExitCode::FAILURE;
            }
            Err(error) => {
                eprintln!("palestre corpus : adresse {server} : {error}");
                return ExitCode::FAILURE;
            }
        },
    };
    let mut passed = 0usize;
    for case in &cases {
        match check_case(case, backend.as_mut()) {
            Ok(()) => {
                passed += 1;
                println!("OK  {}", case.name);
            }
            Err(failure) => println!("KO  {failure}"),
        }
    }
    println!("{passed}/{} cas conformes", cases.len());
    if passed == cases.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn exec(listen: &str, config: &ServerConfig) -> ExitCode {
    let listener = match TcpListener::bind(listen) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("palestre exec : impossible d'écouter sur {listen} : {error}");
            return ExitCode::FAILURE;
        }
    };
    // L'adresse effective, utile avec le port 0 : c'est elle que lisent les tests.
    let announced = listener.local_addr().and_then(|addr| {
        let mut stdout = io::stdout();
        writeln!(stdout, "palestre exec : écoute sur {addr}")?;
        stdout.flush()
    });
    if let Err(error) = announced {
        eprintln!("palestre exec : {error}");
        return ExitCode::FAILURE;
    }
    match serve(listener, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("palestre exec : {error}");
            ExitCode::FAILURE
        }
    }
}
