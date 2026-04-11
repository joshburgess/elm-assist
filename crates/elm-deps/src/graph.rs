use std::collections::{HashMap, HashSet};

/// Find all cycles in a directed graph using DFS.
pub fn find_cycles<'a>(graph: &HashMap<&'a str, Vec<&'a str>>) -> Vec<Vec<&'a str>> {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut on_stack: HashSet<&str> = HashSet::new();
    let mut path: Vec<&str> = Vec::new();
    let mut cycles: Vec<Vec<&str>> = Vec::new();

    let mut modules: Vec<&&str> = graph.keys().collect();
    modules.sort();

    for module in modules {
        if !visited.contains(*module) {
            dfs(
                module,
                graph,
                &mut visited,
                &mut on_stack,
                &mut path,
                &mut cycles,
            );
        }
    }

    let mut unique: Vec<Vec<&str>> = Vec::new();
    for cycle in cycles {
        let normalized = normalize_cycle(&cycle);
        if !unique.iter().any(|c| normalize_cycle(c) == normalized) {
            unique.push(cycle);
        }
    }

    unique
}

fn dfs<'a>(
    node: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    on_stack: &mut HashSet<&'a str>,
    path: &mut Vec<&'a str>,
    cycles: &mut Vec<Vec<&'a str>>,
) {
    visited.insert(node);
    on_stack.insert(node);
    path.push(node);

    if let Some(deps) = graph.get(node) {
        for dep in deps {
            if !visited.contains(dep) {
                dfs(dep, graph, visited, on_stack, path, cycles);
            } else if on_stack.contains(dep)
                && let Some(start) = path.iter().position(|n| n == dep)
            {
                let mut cycle: Vec<&str> = path[start..].to_vec();
                cycle.push(dep);
                cycles.push(cycle);
            }
        }
    }

    path.pop();
    on_stack.remove(node);
}

fn normalize_cycle<'a>(cycle: &[&'a str]) -> Vec<&'a str> {
    if cycle.len() <= 1 {
        return cycle.to_vec();
    }
    let core = &cycle[..cycle.len() - 1];
    let min_pos = core
        .iter()
        .enumerate()
        .min_by_key(|(_, n)| **n)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let mut normalized: Vec<&str> = core[min_pos..].to_vec();
    normalized.extend_from_slice(&core[..min_pos]);
    normalized.push(normalized[0]);
    normalized
}

/// Build a dependency graph from parsed Elm modules.
pub fn build_graph(modules: &[(String, Vec<String>)]) -> (HashMap<&str, Vec<&str>>, HashSet<&str>) {
    let project_modules: HashSet<&str> = modules.iter().map(|(n, _)| n.as_str()).collect();
    let graph: HashMap<&str, Vec<&str>> = modules
        .iter()
        .map(|(name, imports)| {
            let internal: Vec<&str> = imports
                .iter()
                .filter(|imp| project_modules.contains(imp.as_str()))
                .map(|s| s.as_str())
                .collect();
            (name.as_str(), internal)
        })
        .collect();
    (graph, project_modules)
}
