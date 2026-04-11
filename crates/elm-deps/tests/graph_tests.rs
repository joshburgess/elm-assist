use elm_deps::graph::{build_graph, find_cycles};
use std::collections::HashMap;

#[test]
fn no_cycles_in_linear_graph() {
    let graph: HashMap<&str, Vec<&str>> =
        HashMap::from([("A", vec!["B"]), ("B", vec!["C"]), ("C", vec![])]);
    let cycles = find_cycles(&graph);
    assert!(cycles.is_empty());
}

#[test]
fn detects_simple_cycle() {
    let graph: HashMap<&str, Vec<&str>> = HashMap::from([("A", vec!["B"]), ("B", vec!["A"])]);
    let cycles = find_cycles(&graph);
    assert_eq!(cycles.len(), 1);
    // Cycle should contain A and B.
    let cycle = &cycles[0];
    assert!(cycle.contains(&"A"));
    assert!(cycle.contains(&"B"));
}

#[test]
fn detects_triangle_cycle() {
    let graph: HashMap<&str, Vec<&str>> =
        HashMap::from([("A", vec!["B"]), ("B", vec!["C"]), ("C", vec!["A"])]);
    let cycles = find_cycles(&graph);
    assert_eq!(cycles.len(), 1);
}

#[test]
fn no_duplicate_cycles() {
    // A -> B -> A is the same cycle as B -> A -> B.
    let graph: HashMap<&str, Vec<&str>> = HashMap::from([("A", vec!["B"]), ("B", vec!["A"])]);
    let cycles = find_cycles(&graph);
    assert_eq!(cycles.len(), 1);
}

#[test]
fn detects_multiple_independent_cycles() {
    let graph: HashMap<&str, Vec<&str>> = HashMap::from([
        ("A", vec!["B"]),
        ("B", vec!["A"]),
        ("C", vec!["D"]),
        ("D", vec!["C"]),
    ]);
    let cycles = find_cycles(&graph);
    assert_eq!(cycles.len(), 2);
}

#[test]
fn empty_graph() {
    let graph: HashMap<&str, Vec<&str>> = HashMap::new();
    let cycles = find_cycles(&graph);
    assert!(cycles.is_empty());
}

#[test]
fn self_cycle() {
    let graph: HashMap<&str, Vec<&str>> = HashMap::from([("A", vec!["A"])]);
    let cycles = find_cycles(&graph);
    assert_eq!(cycles.len(), 1);
}

#[test]
fn build_graph_filters_internal() {
    let modules = vec![
        (
            "Main".to_string(),
            vec!["Html".to_string(), "Utils".to_string()],
        ),
        ("Utils".to_string(), vec!["String".to_string()]),
    ];
    let (graph, project) = build_graph(&modules);

    assert!(project.contains("Main"));
    assert!(project.contains("Utils"));

    // "Html" and "String" are external — filtered out.
    assert_eq!(graph["Main"], vec!["Utils"]);
    assert!(graph["Utils"].is_empty());
}
