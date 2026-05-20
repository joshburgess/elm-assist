use elm_deps::graph::{build_graph, find_cycles};
use std::collections::HashMap;
use test_better::prelude::*;

#[test]
fn no_cycles_in_linear_graph() -> TestResult {
    let graph: HashMap<&str, Vec<&str>> =
        HashMap::from([("A", vec!["B"]), ("B", vec!["C"]), ("C", vec![])]);
    let cycles = find_cycles(&graph);
    check!(cycles.is_empty()).satisfies(is_true())?;
    Ok(())
}

#[test]
fn detects_simple_cycle() -> TestResult {
    let graph: HashMap<&str, Vec<&str>> = HashMap::from([("A", vec!["B"]), ("B", vec!["A"])]);
    let cycles = find_cycles(&graph);
    check!(cycles.len()).satisfies(eq(1))?;
    // Cycle should contain A and B.
    let cycle = &cycles[0];
    check!(cycle.contains(&"A")).satisfies(is_true())?;
    check!(cycle.contains(&"B")).satisfies(is_true())?;
    Ok(())
}

#[test]
fn detects_triangle_cycle() -> TestResult {
    let graph: HashMap<&str, Vec<&str>> =
        HashMap::from([("A", vec!["B"]), ("B", vec!["C"]), ("C", vec!["A"])]);
    let cycles = find_cycles(&graph);
    check!(cycles.len()).satisfies(eq(1))?;
    Ok(())
}

#[test]
fn no_duplicate_cycles() -> TestResult {
    // A -> B -> A is the same cycle as B -> A -> B.
    let graph: HashMap<&str, Vec<&str>> = HashMap::from([("A", vec!["B"]), ("B", vec!["A"])]);
    let cycles = find_cycles(&graph);
    check!(cycles.len()).satisfies(eq(1))?;
    Ok(())
}

#[test]
fn detects_multiple_independent_cycles() -> TestResult {
    let graph: HashMap<&str, Vec<&str>> = HashMap::from([
        ("A", vec!["B"]),
        ("B", vec!["A"]),
        ("C", vec!["D"]),
        ("D", vec!["C"]),
    ]);
    let cycles = find_cycles(&graph);
    check!(cycles.len()).satisfies(eq(2))?;
    Ok(())
}

#[test]
fn empty_graph() -> TestResult {
    let graph: HashMap<&str, Vec<&str>> = HashMap::new();
    let cycles = find_cycles(&graph);
    check!(cycles.is_empty()).satisfies(is_true())?;
    Ok(())
}

#[test]
fn self_cycle() -> TestResult {
    let graph: HashMap<&str, Vec<&str>> = HashMap::from([("A", vec!["A"])]);
    let cycles = find_cycles(&graph);
    check!(cycles.len()).satisfies(eq(1))?;
    Ok(())
}

#[test]
fn build_graph_filters_internal() -> TestResult {
    let modules = vec![
        (
            "Main".to_string(),
            vec!["Html".to_string(), "Utils".to_string()],
        ),
        ("Utils".to_string(), vec!["String".to_string()]),
    ];
    let (graph, project) = build_graph(&modules);

    check!(project.contains("Main")).satisfies(is_true())?;
    check!(project.contains("Utils")).satisfies(is_true())?;

    // "Html" and "String" are external — filtered out.
    check!(graph["Main"].as_slice()).satisfies(eq(vec!["Utils"].as_slice()))?;
    check!(graph["Utils"].is_empty()).satisfies(is_true())?;
    Ok(())
}
