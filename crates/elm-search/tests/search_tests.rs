use elm_ast::parse;
use elm_search::query::parse_query;
use elm_search::search::search;

fn count(source: &str, query_str: &str) -> usize {
    let module = parse(source).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let query = parse_query(query_str).unwrap();
    search(&module, &query).len()
}

// ── returns ──────────────────────────────────────────────────────────

#[test]
fn returns_maybe() {
    let src = "\
module Main exposing (..)

get : Int -> Maybe String
get n = Nothing

set : Int -> String -> String
set n s = s
";
    assert_eq!(count(src, "returns Maybe"), 1);
}

#[test]
fn returns_no_match() {
    let src = "\
module Main exposing (..)

foo : Int -> String
foo n = \"\"
";
    assert_eq!(count(src, "returns String"), 1);
    assert_eq!(count(src, "returns Maybe"), 0);
}

// ── type ─────────────────────────────────────────────────────────────

#[test]
fn type_in_signature() {
    let src = "\
module Main exposing (..)

decode : Json.Decode.Decoder String -> String
decode d = \"\"
";
    assert_eq!(count(src, "type Decoder"), 1);
    assert_eq!(count(src, "type Int"), 0);
}

// ── case-on ──────────────────────────────────────────────────────────

#[test]
fn case_on_constructor() {
    let src = "\
module Main exposing (..)

f x =
    case x of
        Just v -> v
        Nothing -> 0
";
    assert_eq!(count(src, "case-on Just"), 1);
    assert_eq!(count(src, "case-on Nothing"), 1);
    assert_eq!(count(src, "case-on Err"), 0);
}

// ── update ───────────────────────────────────────────────────────────

#[test]
fn record_update_field() {
    let src = "\
module Main exposing (..)

f model = { model | name = \"new\", count = 0 }
";
    assert_eq!(count(src, "update .name"), 1);
    assert_eq!(count(src, "update .count"), 1);
    assert_eq!(count(src, "update .age"), 0);
}

// ── calls ────────────────────────────────────────────────────────────

#[test]
fn calls_to_module() {
    let src = "\
module Main exposing (..)

x = Http.get url
y = Http.post body
z = String.length s
";
    assert_eq!(count(src, "calls Http"), 2);
    assert_eq!(count(src, "calls String"), 1);
    assert_eq!(count(src, "calls Json"), 0);
}

// ── unused-args ──────────────────────────────────────────────────────

#[test]
fn unused_args_detected() {
    let src = "\
module Main exposing (..)

f x y = x + 1
";
    // `y` is unused.
    assert_eq!(count(src, "unused-args"), 1);
}

#[test]
fn no_unused_args() {
    let src = "\
module Main exposing (..)

f x y = x + y
";
    assert_eq!(count(src, "unused-args"), 0);
}

// ── lambda ───────────────────────────────────────────────────────────

#[test]
fn lambda_arity() {
    let src = "\
module Main exposing (..)

f = \\a b c -> a + b + c

g = \\x -> x
";
    assert_eq!(count(src, "lambda 3"), 1);
    assert_eq!(count(src, "lambda 1"), 2);
    assert_eq!(count(src, "lambda 4"), 0);
}

// ── uses ─────────────────────────────────────────────────────────────

#[test]
fn uses_name() {
    let src = "\
module Main exposing (..)

f x = List.map g x

g y = y + 1
";
    assert_eq!(count(src, "uses g"), 1); // reference in List.map g x
    assert_eq!(count(src, "uses map"), 1);
}

// ── def ──────────────────────────────────────────────────────────────

#[test]
fn def_pattern() {
    let src = "\
module Main exposing (..)

updateModel x = x

viewModel y = y

helper z = z
";
    assert_eq!(count(src, "def Model"), 2); // updateModel, viewModel
    assert_eq!(count(src, "def helper"), 1);
    assert_eq!(count(src, "def nope"), 0);
}

// ── expr ─────────────────────────────────────────────────────────────

#[test]
fn expr_kind_let() {
    let src = "\
module Main exposing (..)

f x =
    let
        y = 1
    in
    x + y
";
    assert_eq!(count(src, "expr let"), 1);
    assert_eq!(count(src, "expr case"), 0);
}

#[test]
fn expr_kind_lambda() {
    let src = "\
module Main exposing (..)

f = \\x -> x
g = List.map (\\y -> y + 1) list
";
    assert_eq!(count(src, "expr lambda"), 2);
}

// ── Query parsing ────────────────────────────────────────────────────

#[test]
fn parse_query_valid() {
    assert!(parse_query("returns Maybe").is_ok());
    assert!(parse_query("case-on Result").is_ok());
    assert!(parse_query("update .name").is_ok());
    assert!(parse_query("calls Http").is_ok());
    assert!(parse_query("unused-args").is_ok());
    assert!(parse_query("lambda 3").is_ok());
    assert!(parse_query("uses map").is_ok());
    assert!(parse_query("def update").is_ok());
    assert!(parse_query("expr let").is_ok());
}

#[test]
fn parse_query_invalid() {
    assert!(parse_query("invalid").is_err());
    assert!(parse_query("returns").is_err());
    assert!(parse_query("lambda abc").is_err());
}
