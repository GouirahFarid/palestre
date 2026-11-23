#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use agon::lexer::{Token, lex};

fn tokens(source: &str) -> Vec<Token> {
    lex(source).unwrap().into_iter().map(|s| s.token).collect()
}

#[test]
fn keywords_identifiers_and_numbers() {
    assert_eq!(
        tokens("fn main let x_1 = 0x1F;"),
        vec![
            Token::Fn,
            Token::Ident("main".to_string()),
            Token::Let,
            Token::Ident("x_1".to_string()),
            Token::Assign,
            Token::Int(31),
            Token::Semi,
        ]
    );
}

#[test]
fn longest_operator_wins() {
    assert_eq!(
        tokens(">>> >> >= > << <= == != = !"),
        vec![
            Token::Shr,
            Token::Sar,
            Token::Ge,
            Token::Gt,
            Token::Shl,
            Token::Le,
            Token::EqEq,
            Token::NotEq,
            Token::Assign,
            Token::Bang,
        ]
    );
}

#[test]
fn comments_are_skipped_and_positions_tracked() {
    let spanned = lex("1 // commentaire\n  x").unwrap();
    assert_eq!(spanned.len(), 2);
    assert_eq!((spanned[1].line, spanned[1].col), (2, 3));
}

#[test]
fn lexical_errors() {
    assert!(lex("@").is_err());
    assert!(lex("99999999999999999999").is_err());
    assert!(lex("0x").is_err());
    assert_eq!(lex("a\n  $").unwrap_err().line, 2);
}

#[test]
fn the_largest_literals_still_fit() {
    assert_eq!(tokens("9223372036854775807"), vec![Token::Int(i64::MAX)]);
    assert_eq!(tokens("0x7FFFFFFFFFFFFFFF"), vec![Token::Int(i64::MAX)]);
    assert!(lex("9223372036854775808").is_err());
    assert!(lex("0x8000000000000000").is_err());
}

#[test]
fn a_number_glued_to_letters_is_malformed() {
    assert!(lex("12ab").is_err());
    assert!(lex("0x1G").is_err());
}

#[test]
fn double_ampersand_says_what_to_write_instead() {
    let err = lex("a && b").unwrap_err();
    assert_eq!((err.line, err.col), (1, 3));
    assert!(err.message.contains('&'), "{}", err.message);
    assert!(lex("a || b").is_err());
}

#[test]
fn columns_count_characters_not_bytes() {
    let err = lex("// é\n  é").unwrap_err();
    assert_eq!((err.line, err.col), (2, 3));
}
