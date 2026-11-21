//! `palestre` : la ligne de commande du projet.

use std::io::{self, Write};
use std::net::TcpListener;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
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
