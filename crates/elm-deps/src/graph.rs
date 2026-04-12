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

/// Dependency graph statistics.
#[derive(Debug, Clone)]
pub struct DepsStats {
    pub total_modules: usize,
    pub total_edges: usize,
    pub avg_imports: f64,
    pub leaf_count: usize,
    pub root_count: usize,
    /// Modules with most imports (afferent coupling), sorted descending.
    pub most_imports: Vec<(String, usize)>,
    /// Modules most depended on (efferent coupling), sorted descending.
    pub most_depended_on: Vec<(String, usize)>,
    pub cycle_count: usize,
    pub cycles: Vec<Vec<String>>,
}

/// Compute dependency statistics from an internal graph.
pub fn compute_stats(graph: &HashMap<&str, Vec<&str>>) -> DepsStats {
    let total_modules = graph.len();
    let total_edges: usize = graph.values().map(|v| v.len()).sum();
    let avg_imports = if total_modules > 0 {
        total_edges as f64 / total_modules as f64
    } else {
        0.0
    };

    // Afferent coupling: modules with most imports.
    let mut import_counts: Vec<(&str, usize)> =
        graph.iter().map(|(m, deps)| (*m, deps.len())).collect();
    import_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));

    // Efferent coupling: modules most depended on.
    let mut depended_on: HashMap<&str, usize> = HashMap::new();
    for deps in graph.values() {
        for dep in deps {
            *depended_on.entry(dep).or_default() += 1;
        }
    }
    let mut dep_counts: Vec<(&str, usize)> = depended_on.into_iter().collect();
    dep_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));

    // Leaf modules (no internal imports).
    let leaf_count = graph.values().filter(|deps| deps.is_empty()).count();

    // Root modules (not imported by anyone).
    let all_imported: HashSet<&str> = graph.values().flat_map(|v| v.iter().copied()).collect();
    let root_count = graph.keys().filter(|m| !all_imported.contains(**m)).count();

    let cycles = find_cycles(graph);

    DepsStats {
        total_modules,
        total_edges,
        avg_imports,
        leaf_count,
        root_count,
        most_imports: import_counts
            .into_iter()
            .map(|(m, c)| (m.to_string(), c))
            .collect(),
        most_depended_on: dep_counts
            .into_iter()
            .map(|(m, c)| (m.to_string(), c))
            .collect(),
        cycle_count: cycles.len(),
        cycles: cycles
            .into_iter()
            .map(|c| c.into_iter().map(|s| s.to_string()).collect())
            .collect(),
    }
}
