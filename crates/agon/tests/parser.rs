#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use agon::parser::{parse, parse_expr};

fn expr(source: &str) -> String {
    parse_expr(source).unwrap().to_string()
}

#[test]
fn precedence_follows_rust() {
    assert_eq!(expr("1 + 2 * 3"), "(+ 1 (* 2 3))");
    assert_eq!(expr("a & 1 == 0"), "(== (& a 1) 0)");
    assert_eq!(expr("-x * y"), "(* (- x) y)");
    assert_eq!(expr("1 - 2 - 3"), "(- (- 1 2) 3)");
    assert_eq!(expr("a | b ^ c & d"), "(| a (^ b (& c d)))");
    assert_eq!(expr("x << 2 + 1"), "(<< x (+ 2 1))");
    assert_eq!(expr("mem[i + 1] >>> 3"), "(>>> (mem (+ i 1)) 3)");
    assert_eq!(expr("f(1, g(2), h())"), "(f 1 (g 2) (h))");
    assert_eq!(expr("!~(a)"), "(! (~ a))");
    assert_eq!(expr("a >> 1 >= b"), "(>= (>> a 1) b)");
}

#[test]
fn comparisons_do_not_chain() {
    assert!(parse_expr("a < b < c").is_err());
    assert!(parse_expr("a == b == c").is_err());
}

#[test]
fn statements_and_blocks() {
    let module = parse(
        "const N = 10;\n\
         fn main() {\n\
           let i = 0;\n\
           while i < N { i = i + 1; }\n\
           if i == N { trace(i); } else if i > N { halt; } else { mem[0] = i; }\n\
           return i;\n\
         }\n",
    )
    .unwrap();
    assert_eq!(
        module.to_string(),
        "(const N 10)\n(fn main () (let i 0) (while (< i N) (do (= i (+ i 1)))) \
         (if (== i N) (then (trace i)) (else (if (> i N) (then (halt)) (else (mem= 0 i))))) (return i))"
    );
}

#[test]
fn functions_take_parameters_and_may_return_nothing() {
    assert_eq!(
        parse("fn add(a, b) { return a + b; }").unwrap().to_string(),
        "(fn add (a b) (return (+ a b)))"
    );
    assert_eq!(
        parse("fn f() { return; }").unwrap().to_string(),
        "(fn f () (return))"
    );
}

#[test]
fn syntax_errors_point_at_the_offending_token() {
    let err = parse("fn main() {\n  let = 3;\n}\n").unwrap_err();
    assert_eq!((err.line, err.col), (2, 7));
    assert!(parse("fn main() { trace(1) }").is_err());
    assert!(parse("fn main() { 1 + ; }").is_err());
    assert!(parse("fn main() { let x = 1;").is_err());
    assert!(parse("let x = 1;").is_err());
}

#[test]
fn an_unfinished_file_points_at_its_end() {
    let err = parse("fn main() { let x = 1;").unwrap_err();
    assert_eq!((err.line, err.col), (1, 23));
    let err = parse("fn main() {\n").unwrap_err();
    assert_eq!((err.line, err.col), (2, 1));
}

#[test]
fn only_a_variable_or_a_memory_cell_can_be_assigned() {
    assert_eq!(
        parse("fn f() { mem[a] = 1; mem[a] + 1; x = mem[0]; }")
            .unwrap()
            .to_string(),
        "(fn f () (mem= a 1) (+ (mem a) 1) (= x (mem 0)))"
    );
    let err = parse("fn f() { a + b = 1; }").unwrap_err();
    assert_eq!((err.line, err.col), (1, 16));
    assert!(parse("fn f() { f() = 1; }").is_err());
}

#[test]
fn empty_items_print_as_empty_lists() {
    assert_eq!(
        parse("fn main() {}\nfn g(x) { if x {} while x {} }")
            .unwrap()
            .to_string(),
        "(fn main ())\n(fn g (x) (if x (then)) (while x (do)))"
    );
    assert_eq!(parse("").unwrap().to_string(), "");
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    assert!(parse_expr(&"(".repeat(100_000)).is_err());
    assert!(parse_expr(&"-".repeat(100_000)).is_err());
    assert!(parse(&format!("fn f() {{ {} }}", "if 1 { ".repeat(100_000))).is_err());
    // La limite est de 64 niveaux : 63 parenthèses passent, 64 non.
    assert_eq!(expr(&format!("{}1{}", "(".repeat(63), ")".repeat(63))), "1");
    assert!(parse_expr(&format!("{}1{}", "(".repeat(64), ")".repeat(64))).is_err());
    // Chaque opérateur d'une chaîne ajoute un étage à l'arbre : `1 + 1 + …` compte aussi.
    assert!(parse_expr(&format!("1{}", " + 1".repeat(63))).is_ok());
    assert!(parse_expr(&format!("1{}", " + 1".repeat(100_000))).is_err());
}

#[test]
fn parse_expr_wants_the_whole_input() {
    assert!(parse_expr("1 2").is_err());
    assert!(parse_expr("").is_err());
}
