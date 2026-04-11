use std::collections::HashMap;

use tower_lsp::lsp_types::Url;

use elm_lint::rule::{LintContext, LintError};

use crate::state::{ServerState, uri_to_file_path};

/// Lint a single document using the current server state.
/// Returns the list of lint errors (empty if the file doesn't parse).
pub fn lint_document(state: &ServerState, uri: &Url) -> Vec<LintError> {
    let Some(doc) = state.documents.get(uri) else {
        return Vec::new();
    };
    let Some(module) = &doc.module else {
        return Vec::new();
    };

    let file_path = uri_to_file_path(uri);
    let active_rules = state.active_rules();

    let ctx = LintContext {
        module,
        source: &doc.source,
        file_path: &file_path,
        project_modules: &state.all_module_names,
        module_info: doc.module_info.as_ref(),
        project: state.project_context.as_ref(),
    };

    let mut all_errors = Vec::new();
    for rule in &active_rules {
        let mut errors = rule.check(&ctx);
        let severity = state
            .config
            .severity_for(rule.name())
            .unwrap_or(rule.default_severity());
        for err in &mut errors {
            err.severity = severity;
        }
        all_errors.extend(errors);
    }

    all_errors
}

/// Re-lint all known documents. Used after project context rebuild.
/// Returns a map of URI → lint errors for documents that have errors.
pub fn lint_all_open(state: &ServerState) -> HashMap<Url, Vec<LintError>> {
    let uris: Vec<Url> = state.documents.keys().cloned().collect();
    let mut results = HashMap::new();

    for uri in uris {
        let errors = lint_document(state, &uri);
        results.insert(uri, errors);
    }

    results
}
