//! `palestre` : la ligne de commande du projet.

mod run;

use std::fs;
use std::io::{self, Write};
use std::net::{TcpListener, ToSocketAddrs};
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use agon::CompileError;
use arena::{Config, Map};
use asm::AsmError;
use clap::{ArgAction, Parser, Subcommand};
use corpus::{Backend, Call, InProcess, Remote, check_case, load_corpus};
use exec::message::ResultMsg;
use exec::server::{ServerConfig, serve, serve_concurrent};
use pbc::Program;
use verify::Limits;
use vm::{MEM_SIZE, Machine, run_with};

use crate::run::Scripted;

/// La carte par défaut (D-010), embarquée pour que `palestre match` marche depuis n'importe quel dossier.
const DEFAULT_MAP: &str = include_str!("../../../maps/default.map");

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
    /// Sert le protocole d'exécution distante (annexe K) : une connexion après l'autre, ou toutes à la fois.
    Exec {
        /// Adresse d'écoute, par exemple 127.0.0.1:9000 (port 0 : un port libre choisi par le système).
        #[arg(long)]
        listen: String,
        /// Sert les connexions en même temps, un thread par connexion (D-013).
        #[arg(long)]
        concurrent: bool,
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
    /// Compile un source `agon` (D-015, D-016) en `.pbc`.
    Compile {
        /// Source `agon`.
        input: PathBuf,
        /// Fichier `.pbc` à écrire.
        #[arg(short, long)]
        output: PathBuf,
        /// Affiche aussi le texte assembleur généré, commenté par lignes du source.
        #[arg(long)]
        emit_asm: bool,
    },
    /// Vérifie un `.pbc` avant toute exécution (D-014) : sauts, pile et appels, sur tous les chemins.
    Verify {
        /// Fichier `.pbc` à vérifier.
        program: PathBuf,
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
    /// Joue un match entre deux agents `.pbc` (annexe E) et affiche le vainqueur.
    Match {
        /// Programme de l'agent 0, qui part de la case `A`.
        a: PathBuf,
        /// Programme de l'agent 1, qui part de la case `B`.
        b: PathBuf,
        /// Carte à jouer (format de D-010) ; sans elle, `maps/default.map`.
        #[arg(long)]
        map: Option<PathBuf>,
        /// Graine du générateur qui répond aux `RAND`.
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Nombre maximal de tours (E.5).
        #[arg(long, default_value_t = 500)]
        turns: u32,
        /// Énergie de départ des deux agents.
        #[arg(long, default_value_t = 100)]
        energy: i64,
        /// Fichier où écrire le journal du match, en JSON.
        #[arg(long)]
        log: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Exec {
            listen,
            concurrent,
            sessions_max,
            read_timeout,
        } => exec(
            &listen,
            concurrent,
            &ServerConfig {
                sessions_max,
                read_timeout: (read_timeout > 0).then(|| Duration::from_secs(read_timeout)),
            },
        ),
        Command::Corpus { dir, server } => corpus(&dir, server.as_deref()),
        Command::Asm { input, output } => asm(&input, &output),
        Command::Disasm { input } => disasm(&input),
        Command::Compile {
            input,
            output,
            emit_asm,
        } => compile(&input, &output, emit_asm),
        Command::Verify { program } => verify_program(&program),
        Command::Run {
            program,
            budget,
            mem,
            sense,
            act,
            rand,
        } => run(&program, budget, &mem, Scripted::new(&sense, act, &rand)),
        Command::Match {
            a,
            b,
            map,
            seed,
            turns,
            energy,
            log,
        } => play_match(
            [&a, &b],
            map.as_deref(),
            &Config {
                seed,
                turns_max: turns,
                initial_energy: energy,
                ..Config::default()
            },
            log.as_deref(),
        ),
    }
}

/// Le bilan du match : tours joués, état final de chaque agent, vainqueur ou match nul.
fn play_match(
    agents: [&Path; 2],
    map: Option<&Path>,
    config: &Config,
    log: Option<&Path>,
) -> ExitCode {
    let [a, b] = agents;
    let (a, b) = match (load("match", a), load("match", b)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(code), _) | (_, Err(code)) => return code,
    };
    let text = match map {
        Some(path) => match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => return fail("match", path, error),
        },
        None => DEFAULT_MAP.to_string(),
    };
    let map = match Map::parse(&text) {
        Ok(map) => map,
        Err(error) => return fail("match", map.unwrap_or(Path::new("carte par défaut")), error),
    };
    let result = arena::play([&a, &b], &map, config);
    if let Some(path) = log {
        let written = result
            .to_json()
            .map_err(io::Error::other)
            .and_then(|json| fs::write(path, json));
        if let Err(error) = written {
            return fail("match", path, error);
        }
    }
    println!("tours joués : {}", result.turns.len());
    for agent in &result.final_state.agents {
        let faults = result
            .turns
            .iter()
            .flat_map(|turn| &turn.agents)
            .filter(|played| played.id == agent.id && played.fault.is_some())
            .count();
        let state = if agent.alive { "vivant" } else { "mort" };
        println!(
            "agent {} : ({}, {}) énergie {}, porté {}, {state}, {faults} faute(s)",
            agent.id, agent.x, agent.y, agent.energy, agent.carried
        );
    }
    match result.winner {
        Some(id) => println!("vainqueur : agent {id}"),
        None => println!("match nul"),
    }
    ExitCode::SUCCESS
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

/// Une erreur s'affiche `fichier:ligne : message`, comme celles de `palestre asm` ; rien n'est écrit.
fn compile(input: &Path, output: &Path, emit_asm: bool) -> ExitCode {
    let source = match fs::read_to_string(input) {
        Ok(source) => source,
        Err(error) => return fail("compile", input, error),
    };
    if emit_asm {
        match agon::compile_to_asm(&source) {
            Ok(text) => print!("{text}"),
            Err(error) => return compile_error(input, &error),
        }
    }
    match agon::compile(&source) {
        Ok(program) => match fs::write(output, program.to_bytes()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => fail("compile", output, error),
        },
        Err(error) => compile_error(input, &error),
    }
}

/// La ligne 0 désigne le programme entier : il n'y a pas de ligne à montrer.
fn compile_error(input: &Path, error: &CompileError) -> ExitCode {
    match error.line {
        0 => fail("compile", input, &error.message),
        line => {
            eprintln!("{}:{line} : {}", input.display(), error.message);
            ExitCode::FAILURE
        }
    }
}

/// Une ligne par faute possible (`0x0008 StackUnderflow`), puis les bornes. Le code de sortie vaut 1 dès que le
/// programme n'est pas sûr, y compris quand l'analyse n'a pas pu tout explorer.
fn verify_program(path: &Path) -> ExitCode {
    let program = match load("verify", path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let verdict = verify::verify(&program);
    for issue in &verdict.issues {
        println!("{:#06x} {}", issue.pc, issue.fault);
    }
    println!(
        "pile max : {}, appels max : {}, états : {}",
        verdict.max_stack, verdict.max_frames, verdict.states
    );
    if !verdict.complete {
        println!(
            "analyse incomplète : limite de {} états atteinte",
            Limits::default().max_states
        );
    }
    if verdict.is_safe() {
        println!("sûr : aucune faute structurelle possible");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
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

fn exec(listen: &str, concurrent: bool, config: &ServerConfig) -> ExitCode {
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
    let served = if concurrent {
        serve_concurrent(listener, config)
    } else {
        serve(listener, config)
    };
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("palestre exec : {error}");
            ExitCode::FAILURE
        }
    }
}
