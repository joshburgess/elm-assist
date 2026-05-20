use elm_ast::parse;
use elm_search::query::parse_query;
use elm_search::search::search;
use test_better::prelude::*;

fn count(source: &str, query_str: &str) -> usize {
    let module = parse(source).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let query = parse_query(query_str).unwrap();
    search(&module, &query).len()
}

// ── returns ──────────────────────────────────────────────────────────

#[test]
fn returns_maybe() -> TestResult {
    let src = "\
module Main exposing (..)

get : Int -> Maybe String
get n = Nothing

set : Int -> String -> String
set n s = s
";
    check!(count(src, "returns Maybe")).satisfies(eq(1))?;
    Ok(())
}

#[test]
fn returns_no_match() -> TestResult {
    let src = "\
module Main exposing (..)

foo : Int -> String
foo n = \"\"
";
    check!(count(src, "returns String")).satisfies(eq(1))?;
    check!(count(src, "returns Maybe")).satisfies(eq(0))?;
    Ok(())
}

// ── type ─────────────────────────────────────────────────────────────

#[test]
fn type_in_signature() -> TestResult {
    let src = "\
module Main exposing (..)

decode : Json.Decode.Decoder String -> String
decode d = \"\"
";
    check!(count(src, "type Decoder")).satisfies(eq(1))?;
    check!(count(src, "type Int")).satisfies(eq(0))?;
    Ok(())
}

// ── case-on ──────────────────────────────────────────────────────────

#[test]
fn case_on_constructor() -> TestResult {
    let src = "\
module Main exposing (..)

f x =
    case x of
        Just v -> v
        Nothing -> 0
";
    check!(count(src, "case-on Just")).satisfies(eq(1))?;
    check!(count(src, "case-on Nothing")).satisfies(eq(1))?;
    check!(count(src, "case-on Err")).satisfies(eq(0))?;
    Ok(())
}

// ── update ───────────────────────────────────────────────────────────

#[test]
fn record_update_field() -> TestResult {
    let src = "\
module Main exposing (..)

f model = { model | name = \"new\", count = 0 }
";
    check!(count(src, "update .name")).satisfies(eq(1))?;
    check!(count(src, "update .count")).satisfies(eq(1))?;
    check!(count(src, "update .age")).satisfies(eq(0))?;
    Ok(())
}

// ── calls ────────────────────────────────────────────────────────────

#[test]
fn calls_to_module() -> TestResult {
    let src = "\
module Main exposing (..)

x = Http.get url
y = Http.post body
z = String.length s
";
    check!(count(src, "calls Http")).satisfies(eq(2))?;
    check!(count(src, "calls String")).satisfies(eq(1))?;
    check!(count(src, "calls Json")).satisfies(eq(0))?;
    Ok(())
}

// ── unused-args ──────────────────────────────────────────────────────

#[test]
fn unused_args_detected() -> TestResult {
    let src = "\
module Main exposing (..)

f x y = x + 1
";
    // `y` is unused.
    check!(count(src, "unused-args")).satisfies(eq(1))?;
    Ok(())
}

#[test]
fn no_unused_args() -> TestResult {
    let src = "\
module Main exposing (..)

f x y = x + y
";
    check!(count(src, "unused-args")).satisfies(eq(0))?;
    Ok(())
}

// ── lambda ───────────────────────────────────────────────────────────

#[test]
fn lambda_arity() -> TestResult {
    let src = "\
module Main exposing (..)

f = \\a b c -> a + b + c

g = \\x -> x
";
    check!(count(src, "lambda 3")).satisfies(eq(1))?;
    check!(count(src, "lambda 1")).satisfies(eq(2))?;
    check!(count(src, "lambda 4")).satisfies(eq(0))?;
    Ok(())
}

// ── uses ─────────────────────────────────────────────────────────────

#[test]
fn uses_name() -> TestResult {
    let src = "\
module Main exposing (..)

f x = List.map g x

g y = y + 1
";
    check!(count(src, "uses g")).satisfies(eq(1))?; // reference in List.map g x
    check!(count(src, "uses map")).satisfies(eq(1))?;
    Ok(())
}

// ── def ──────────────────────────────────────────────────────────────

#[test]
fn def_pattern() -> TestResult {
    let src = "\
module Main exposing (..)

updateModel x = x

viewModel y = y

helper z = z
";
    check!(count(src, "def Model")).satisfies(eq(2))?; // updateModel, viewModel
    check!(count(src, "def helper")).satisfies(eq(1))?;
    check!(count(src, "def nope")).satisfies(eq(0))?;
    Ok(())
}

// ── expr ─────────────────────────────────────────────────────────────

#[test]
fn expr_kind_let() -> TestResult {
    let src = "\
module Main exposing (..)

f x =
    let
        y = 1
    in
    x + y
";
    check!(count(src, "expr let")).satisfies(eq(1))?;
    check!(count(src, "expr case")).satisfies(eq(0))?;
    Ok(())
}

#[test]
fn expr_kind_lambda() -> TestResult {
    let src = "\
module Main exposing (..)

f = \\x -> x
g = List.map (\\y -> y + 1) list
";
    check!(count(src, "expr lambda")).satisfies(eq(2))?;
    Ok(())
}

// ── Query parsing ────────────────────────────────────────────────────

#[test]
fn parse_query_valid() -> TestResult {
    check!(parse_query("returns Maybe").is_ok()).satisfies(is_true())?;
    check!(parse_query("case-on Result").is_ok()).satisfies(is_true())?;
    check!(parse_query("update .name").is_ok()).satisfies(is_true())?;
    check!(parse_query("calls Http").is_ok()).satisfies(is_true())?;
    check!(parse_query("unused-args").is_ok()).satisfies(is_true())?;
    check!(parse_query("lambda 3").is_ok()).satisfies(is_true())?;
    check!(parse_query("uses map").is_ok()).satisfies(is_true())?;
    check!(parse_query("def update").is_ok()).satisfies(is_true())?;
    check!(parse_query("expr let").is_ok()).satisfies(is_true())?;
    Ok(())
}

#[test]
fn parse_query_invalid() -> TestResult {
    check!(parse_query("invalid").is_err()).satisfies(is_true())?;
    check!(parse_query("returns").is_err()).satisfies(is_true())?;
    check!(parse_query("lambda abc").is_err()).satisfies(is_true())?;
    Ok(())
}
