//! `palestre` : la ligne de commande du projet.

mod run;

use std::fs;
use std::io::{self, Write};
use std::net::{TcpListener, ToSocketAddrs};
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use asm::AsmError;
use clap::{ArgAction, Parser, Subcommand};
use corpus::{Backend, Call, InProcess, Remote, check_case, load_corpus};
use exec::message::ResultMsg;
use exec::server::{ServerConfig, serve};
use pbc::Program;
use vm::{MEM_SIZE, Machine, run_with};

use crate::run::Scripted;

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
    /// Assemble un fichier texte (syntaxe de D-009) en `.pbc`.
    Asm {
        /// Source de l'assembleur.
        input: PathBuf,
        /// Fichier `.pbc` à écrire.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Affiche un `.pbc` sous forme de texte, qui se réassemble à l'identique.
    Disasm {
        /// Fichier `.pbc` à lire.
        input: PathBuf,
    },
    /// Exécute un tour en local et affiche `{"calls": […], "result": {…}}`, la forme d'une exécution de
    /// `expected.json`.
    Run {
        /// Fichier `.pbc` à exécuter.
        program: PathBuf,
        /// Budget de gaz du tour, de 1 à 65535.
        #[arg(long, default_value = "1000")]
        budget: NonZeroU16,
        /// Cellule de la mémoire initiale, `K=V` (répétable) ; les autres valent 0.
        #[arg(long = "mem", value_parser = run::pair::<u8, i64>)]
        mem: Vec<(u8, i64)>,
        /// Réponse du capteur K, `K=V` (répétable) ; un capteur absent répond `BadSensor`.
        #[arg(long = "sense", value_parser = run::pair::<u8, i64>)]
        sense: Vec<(u8, i64)>,
        /// Issue de toute action tentée.
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        act: bool,
        /// Valeur rendue par `RAND`, dans l'ordre (répétable) ; ensuite `RAND` rend 0.
        #[arg(long = "rand", allow_negative_numbers = true)]
        rand: Vec<i64>,
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
        Command::Asm { input, output } => asm(&input, &output),
        Command::Disasm { input } => disasm(&input),
        Command::Run {
            program,
            budget,
            mem,
            sense,
            act,
            rand,
        } => run(&program, budget, &mem, Scripted::new(&sense, act, &rand)),
    }
}

/// Une erreur s'affiche `fichier:ligne : message`, comme celles d'un compilateur.
fn asm(input: &Path, output: &Path) -> ExitCode {
    let source = match fs::read_to_string(input) {
        Ok(source) => source,
        Err(error) => return fail("asm", input, error),
    };
    match asm::assemble(&source) {
        Ok(program) => match fs::write(output, program.to_bytes()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => fail("asm", output, error),
        },
        // Ligne 0 : une erreur sur le programme entier (code vide, trop de constantes…).
        Err(AsmError { line: 0, kind }) => fail("asm", input, kind),
        Err(AsmError { line, kind }) => {
            eprintln!("{}:{line} : {kind}", input.display());
            ExitCode::FAILURE
        }
    }
}

fn disasm(input: &Path) -> ExitCode {
    match load("disasm", input) {
        Ok(program) => {
            print!("{}", asm::disassemble(&program));
            ExitCode::SUCCESS
        }
        Err(code) => code,
    }
}

fn run(path: &Path, budget: NonZeroU16, cells: &[(u8, i64)], mut env: Scripted) -> ExitCode {
    let program = match load("run", path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let mut mem = [0; MEM_SIZE];
    for &(k, v) in cells {
        if let Some(cell) = mem.get_mut(usize::from(k)) {
            *cell = v;
        }
    }
    let report = run_with(Machine::new(&program, mem, budget), &mut env);
    let calls: Vec<_> = env.calls().iter().map(Call::to_json).collect();
    let execution = serde_json::json!({"calls": calls, "result": ResultMsg::from_report(&report)});
    println!("{execution}");
    ExitCode::SUCCESS
}

/// Lit et charge un `.pbc` ; en cas d'échec, l'erreur est déjà affichée.
fn load(command: &str, path: &Path) -> Result<Program, ExitCode> {
    let bytes = fs::read(path).map_err(|error| fail(command, path, error))?;
    Program::load(&bytes).map_err(|error| fail(command, path, error))
}

fn fail(command: &str, path: &Path, error: impl std::fmt::Display) -> ExitCode {
    eprintln!("palestre {command} : {} : {error}", path.display());
    ExitCode::FAILURE
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
