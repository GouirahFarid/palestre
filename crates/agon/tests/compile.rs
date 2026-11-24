#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::VecDeque;
use std::num::NonZeroU16;

use pbc::Fault;
use proptest::prelude::*;
use proptest::sample::select;
use vm::{Action, Env, Machine, Memory, Report, run_with};

#[derive(Default)]
struct Script {
    senses: VecDeque<i64>,
    acts: VecDeque<bool>,
    rands: VecDeque<i64>,
    log: Vec<String>,
}

impl Env for Script {
    fn sense(&mut self, k: u8) -> Result<i64, Fault> {
        self.log.push(format!("SENSE {k}"));
        Ok(self.senses.pop_front().unwrap_or(0))
    }
    fn act(&mut self, action: Action) -> bool {
        self.log
            .push(format!("ACT {} {}", action.kind.name(), action.arg));
        self.acts.pop_front().unwrap_or(true)
    }
    fn rand(&mut self) -> i64 {
        self.log.push("RAND".to_string());
        self.rands.pop_front().unwrap_or(0)
    }
}

/// Compile, vérifie que le vérificateur statique (B2) accepte le programme, puis l'exécute sans faute.
fn run_with_env(source: &str, mem: Memory, env: &mut Script) -> Report {
    let program = agon::compile(source).unwrap();
    let asm = agon::compile_to_asm(source).unwrap();
    let verdict = verify::verify(&program);
    assert!(verdict.is_safe(), "{verdict:?}\n{asm}");
    let report = run_with(
        Machine::new(&program, mem, NonZeroU16::new(5000).unwrap()),
        env,
    );
    assert_eq!(report.fault, None, "{asm}");
    report
}

fn trace(source: &str) -> Vec<i64> {
    run_with_env(source, [0; 256], &mut Script::default()).trace
}

fn error(source: &str) -> String {
    agon::compile(source).unwrap_err().to_string()
}

#[test]
fn arithmetic_and_shifts() {
    assert_eq!(
        trace(
            "fn main() { trace(1 + 2 * 3); trace(-7 / 2); trace(-7 >> 1); trace(-1 >>> 63); trace(7 % 3 == 1); }"
        ),
        vec![7, -3, -4, 1, 1]
    );
}

#[test]
fn comparisons_and_logic_yield_0_or_1() {
    assert_eq!(
        trace("fn main() { trace(1 <= 1); trace(2 >= 3); trace(1 != 2); trace(!5); trace(~0); }"),
        vec![1, 0, 1, 0, -1]
    );
}

#[test]
fn locals_and_while() {
    let src =
        "fn main() { let s = 0; let i = 1; while i <= 10 { s = s + i; i = i + 1; } trace(s); }";
    assert_eq!(trace(src), vec![55]);
}

#[test]
fn if_else_chains() {
    let src = "fn sign(x) { if x < 0 { return -1; } else if x == 0 { return 0; } else { return 1; } }\n\
               fn main() { trace(sign(-5)); trace(sign(0)); trace(sign(9)); }";
    assert_eq!(trace(src), vec![-1, 0, 1]);
}

#[test]
fn functions_bind_parameters_in_order() {
    assert_eq!(
        trace("fn sq(x) { return x * x; } fn main() { trace(sq(3) + sq(4)); }"),
        vec![25]
    );
    assert_eq!(
        trace("fn sub(a, b) { return a - b; } fn main() { trace(sub(10, 3)); }"),
        vec![7]
    );
    assert_eq!(trace("fn f() { } fn main() { trace(f()); }"), vec![0]);
}

#[test]
fn memory_is_the_agents_persistent_memory() {
    let mut mem = [0; 256];
    mem[0] = 41;
    let report = run_with_env(
        "fn main() { mem[0] = mem[0] + 1; trace(mem[0]); }",
        mem,
        &mut Script::default(),
    );
    assert_eq!(report.trace, vec![42]);
    assert_eq!(report.mem[0], 42);
}

#[test]
fn computed_addresses_use_the_popped_forms() {
    let report = run_with_env(
        "fn main() { let i = 5; mem[i + 1] = 7; trace(mem[i + 1] + mem[6]); }",
        [0; 256],
        &mut Script::default(),
    );
    assert_eq!(report.trace, vec![14]);
    assert_eq!(report.mem[6], 7);
}

#[test]
fn large_literals_go_through_the_constant_table() {
    let program =
        agon::compile("fn main() { trace(1234567890123); trace(5); trace(1234567890123); }")
            .unwrap();
    assert_eq!(program.constants(), &[1_234_567_890_123]);
    assert_eq!(
        trace("fn main() { trace(1234567890123); trace(5); trace(1234567890123); }"),
        vec![1_234_567_890_123, 5, 1_234_567_890_123]
    );
}

#[test]
fn constants_are_folded() {
    assert_eq!(
        trace("const LIMIT = 4 * 1000; fn main() { trace(LIMIT); }"),
        vec![4000]
    );
    assert_eq!(
        trace("const A = 1 << 40; const B = A + 1; fn main() { trace(B - A); }"),
        vec![1]
    );
}

#[test]
fn builtins_map_to_instructions() {
    let mut env = Script {
        senses: [9].into(),
        rands: [4].into(),
        ..Script::default()
    };
    let src =
        "fn main() { let s = sense(8); let ok = act(0, s); trace(ok + rand()); trace(gas() > 0); }";
    let report = run_with_env(src, [0; 256], &mut env);
    assert_eq!(env.log, ["SENSE 8", "ACT MOVE 9", "RAND"]);
    assert_eq!(report.trace, vec![5, 1]);
}

#[test]
fn statements_leave_the_stack_balanced() {
    // 70 tours de boucle : une seule valeur laissée sur la pile à chaque tour déborderait les 64 emplacements.
    let src = "fn f() { return 1; }\n\
               fn main() { let i = 0; while i < 70 { f(); i + 1; trace(i); i = i + 1; } act(1, 0); trace(i); }";
    let report = run_with_env(src, [0; 256], &mut Script::default());
    assert_eq!(report.trace.len(), 71);
    assert_eq!(report.trace.last(), Some(&70));
}

/// Les instructions du texte assembleur, sans les commentaires ni les étiquettes.
fn instructions(source: &str) -> Vec<String> {
    agon::compile_to_asm(source)
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("    "))
        .map(|line| line.split(';').next().unwrap_or("").trim().to_string())
        .collect()
}

#[test]
fn act_as_a_statement_drops_its_outcome_and_trace_leaves_nothing() {
    // `ACT` empile toujours son issue (B.3, `arg -- ok`) : en position d'instruction, il faut la jeter. Une seule
    // action par tour : une issue oubliée ne se verrait jamais à l'exécution, d'où ce test sur le texte.
    let act = instructions("fn main() { act(1, 0); }");
    let at = act.iter().position(|instr| instr == "ACT 1").unwrap();
    assert_eq!(act.get(at + 1).map(String::as_str), Some("DROP"));
    // `TRACE` ne laisse rien : pas de 0 à empiler pour le jeter aussitôt.
    let trace = instructions("fn main() { trace(1); }");
    assert!(!trace.iter().any(|instr| instr == "DROP"), "{trace:?}");
}

#[test]
fn halt_ends_the_turn() {
    assert_eq!(trace("fn main() { trace(1); halt; trace(2); }"), vec![1]);
}

#[test]
fn semantic_errors_name_the_culprit() {
    let cases = [
        ("fn main() { trace(x); }", "x"),
        ("fn main() { foo(); }", "foo"),
        ("fn f(a) { return a; } fn main() { trace(f()); }", "f"),
        ("fn f() { return f(); } fn main() { f(); }", "récursi"),
        (
            "fn f() { return g(); } fn g() { return f(); } fn main() { f(); }",
            "récursi",
        ),
        ("fn main() { let k = 3; trace(sense(k)); }", "sense"),
        ("fn main() { act(4, 0); }", "act"),
        ("fn helper() { }", "main"),
        ("fn main() { } fn main() { }", "main"),
        ("fn main() { let x = 1; let x = 2; }", "x"),
        ("fn main() { let x = 1; x = ; }", "colonne"),
    ];
    for (source, needle) in cases {
        let err = error(source);
        assert!(err.contains(needle), "{source} → {err}");
    }
}

#[test]
fn errors_carry_the_source_line() {
    let err = agon::compile("fn main() {\n  trace(1);\n  trace(y);\n}").unwrap_err();
    assert_eq!(err.line, 3);
    let err = agon::compile("fn main() {\n  let = 1;\n}").unwrap_err();
    assert_eq!(err.line, 2);
    assert!(err.to_string().contains("colonne 7"), "{err}");
}

#[test]
fn a_variable_lives_until_the_end_of_its_block() {
    assert!(error("fn main() { if 1 { let x = 1; } trace(x); }").contains('x'));
    assert!(error("fn main() { let y = y + 1; }").contains('y'));
    assert!(error("fn main() { if 1 { let t = 1; } else { let t = 2; } }").contains('t'));
    assert_eq!(
        trace("fn main() { let a = 2; if a > 1 { let b = a * 10; trace(b); } }"),
        vec![20]
    );
}

#[test]
fn constants_and_builtins_are_protected_names() {
    assert!(error("const N = 1; fn main() { N = 2; }").contains('N'));
    assert!(error("const N = 1; fn main() { let N = 2; }").contains('N'));
    assert!(error("fn f(N) { } const N = 1; fn main() { }").contains('N'));
    assert!(error("fn sense() { } fn main() { }").contains("sense"));
    assert!(error("fn main() { trace(rand(1)); }").contains("rand"));
    assert!(error("const A = B; const B = 1; fn main() { }").contains('B'));
    assert!(error("const Z = 1 / 0; fn main() { }").contains("zéro"));
    assert!(error("const X = mem[0]; fn main() { }").contains('X'));
}

#[test]
fn the_compilers_cells_are_out_of_reach() {
    assert!(error("fn main() { mem[128] = 1; }").contains("128"));
    assert!(error("fn main() { trace(mem[255]); }").contains("255"));
    assert!(error("fn main() { trace(mem[0 - 1]); }").contains("mem"));
    let report = run_with_env(
        "const LAST = 127; fn main() { mem[LAST] = 3; }",
        [0; 256],
        &mut Script::default(),
    );
    assert_eq!(report.mem[127], 3);
}

#[test]
fn at_most_128_variables() {
    let lets: String = (0..128).map(|i| format!("let v{i} = {i}; ")).collect();
    assert_eq!(
        trace(&format!("fn main() {{ {lets} trace(v127); }}")),
        vec![127]
    );
    let lets: String = (0..129).map(|i| format!("let v{i} = {i}; ")).collect();
    assert!(error(&format!("fn main() {{ {lets} }}")).contains("128"));
}

/// `main` appelle `f1`, qui appelle `f2`… jusqu'à `fn`.
fn call_chain(n: usize) -> String {
    let mut source = String::from("fn main() { trace(f1()); }\n");
    for i in 1..n {
        source.push_str(&format!("fn f{i}() {{ return f{}() + 1; }}\n", i + 1));
    }
    source.push_str(&format!("fn f{n}() {{ return 1; }}\n"));
    source
}

#[test]
fn at_most_16_call_frames() {
    // `main` et quinze fonctions : seize cadres, la limite de B.1.
    assert_eq!(trace(&call_chain(15)), vec![15]);
    let err = error(&call_chain(16));
    assert!(err.contains("16"), "{err}");
    assert!(err.contains("main → f1"), "{err}");
}

#[test]
fn the_compiled_harvester_beats_the_idler() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let source = std::fs::read_to_string(format!("{root}/agents/harvester.agon")).unwrap();
    let harvester = agon::compile(&source).unwrap();
    assert!(verify::verify(&harvester).is_safe());
    let idler =
        asm::assemble(&std::fs::read_to_string(format!("{root}/agents/idler.asm")).unwrap())
            .unwrap();
    let map =
        arena::Map::parse(&std::fs::read_to_string(format!("{root}/maps/default.map")).unwrap())
            .unwrap();
    let log = arena::play(
        [&harvester, &idler],
        &map,
        &arena::Config {
            seed: 1,
            ..arena::Config::default()
        },
    );
    assert_eq!(log.winner, Some(0));
}

#[test]
fn the_compiled_harvester_plays_the_same_match_as_the_assembled_one() {
    // Même stratégie, mêmes décisions : les deux journaux ne diffèrent que par le gaz consommé.
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let read = |path: &str| std::fs::read_to_string(format!("{root}/{path}")).unwrap();
    let compiled = agon::compile(&read("agents/harvester.agon")).unwrap();
    let assembled = asm::assemble(&read("agents/harvester.asm")).unwrap();
    let idler = asm::assemble(&read("agents/idler.asm")).unwrap();
    let map = arena::Map::parse(&read("maps/default.map")).unwrap();
    for seed in [0, 1, 7] {
        let config = arena::Config {
            seed,
            ..arena::Config::default()
        };
        let logs = [&compiled, &assembled].map(|harvester| {
            let mut log = arena::play([harvester, &idler], &map, &config);
            for turn in &mut log.turns {
                for agent in &mut turn.agents {
                    agent.gas_used = 0;
                }
            }
            log
        });
        let [compiled_log, assembled_log] = &logs;
        assert_eq!(compiled_log, assembled_log, "graine {seed}");
    }
}

/// Un entier `i64` écrit en `agon`, où un littéral est toujours positif.
fn literal(n: i64) -> String {
    if n == i64::MIN {
        "(0 - 9223372036854775807 - 1)".to_string()
    } else if n < 0 {
        format!("(0 - {})", n.unsigned_abs())
    } else {
        n.to_string()
    }
}

const BINARY: [&str; 17] = [
    "+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">=", "&", "|", "^", "<<", ">>", ">>>",
];

proptest! {
    /// Une constante est calculée par le compilateur ; la même expression dans une fonction l'est par la machine.
    #[test]
    fn constant_folding_matches_the_machine(
        a in any::<i64>(),
        b in prop_oneof![any::<i64>(), -70i64..70],
        op in select(BINARY.to_vec()),
    ) {
        let expr = format!("{} {op} {}", literal(a), literal(b));
        let source = format!("const C = {expr}; fn main() {{ trace(C); trace({expr}); }}");
        if b == 0 && (op == "/" || op == "%") {
            prop_assert!(error(&source).contains("zéro"));
        } else {
            let values = trace(&source);
            prop_assert_eq!(values[0], values[1], "{}", source);
        }
    }

    #[test]
    fn unary_folding_matches_the_machine(a in any::<i64>(), op in select(vec!["-", "!", "~"])) {
        let expr = format!("{op}{}", literal(a));
        let source = format!("const C = {expr}; fn main() {{ trace(C); trace({expr}); }}");
        let values = trace(&source);
        prop_assert_eq!(values[0], values[1], "{}", source);
    }
}
