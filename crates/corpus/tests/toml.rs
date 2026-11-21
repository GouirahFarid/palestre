#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use corpus::toml::{Value, parse};

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

#[test]
fn root_keys_strings_and_comments() {
    let doc = parse("description = \"a \\\"quoted\\\" word\" # commentaire\ncategory = \"gas\"\n")
        .unwrap();
    assert_eq!(
        doc.root,
        vec![
            ("description".to_string(), s("a \"quoted\" word")),
            ("category".to_string(), s("gas"))
        ]
    );
    assert!(doc.arrays.is_empty());
}

#[test]
fn array_of_tables_with_inline_values() {
    let src = "[[exec]]\nbudget = 12\nmem = { 3 = 42, 18 = -9 }\n\
               answers = [{ sense = 0 }, { act = true }, { sense_fault = \"BadSensor\" }]\n\n[[exec]]\n";
    let doc = parse(src).unwrap();
    assert_eq!(doc.arrays.len(), 2);
    let (name, first) = &doc.arrays[0];
    assert_eq!(name, "exec");
    assert_eq!(first[0], ("budget".to_string(), Value::Int(12)));
    assert_eq!(
        first[1],
        (
            "mem".to_string(),
            Value::Table(vec![
                ("3".to_string(), Value::Int(42)),
                ("18".to_string(), Value::Int(-9))
            ])
        )
    );
    assert_eq!(
        first[2].1,
        Value::Array(vec![
            Value::Table(vec![("sense".to_string(), Value::Int(0))]),
            Value::Table(vec![("act".to_string(), Value::Bool(true))]),
            Value::Table(vec![("sense_fault".to_string(), s("BadSensor"))]),
        ])
    );
    assert!(doc.arrays[1].1.is_empty());
}

#[test]
fn arrays_may_span_lines_with_a_trailing_comma() {
    let doc = parse("answers = [\n  { rand = 7 },\n  { rand = -1 }, # fin\n]\n").unwrap();
    assert_eq!(
        doc.root[0].1,
        Value::Array(vec![
            Value::Table(vec![("rand".to_string(), Value::Int(7))]),
            Value::Table(vec![("rand".to_string(), Value::Int(-1))]),
        ])
    );
}

#[test]
fn integer_limits() {
    let doc = parse("a = -9223372036854775808\nb = 9223372036854775807\n").unwrap();
    assert_eq!(doc.root[0].1, Value::Int(i64::MIN));
    assert_eq!(doc.root[1].1, Value::Int(i64::MAX));
}

#[test]
fn errors_carry_a_line_number() {
    assert_eq!(parse("a = 1\nb = ?\n").unwrap_err().line, 2);
    assert!(parse("a = \"unterminated\n").is_err());
    assert!(parse("[[exec]\n").is_err());
    assert!(parse("a = 99999999999999999999\n").is_err());
    assert!(parse("a = 1.5\n").is_err());
    assert!(parse("[table]\n").is_err());
    assert!(parse("a = 1 b = 2\n").is_err());
}
