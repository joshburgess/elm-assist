//! Application state, messages, and update logic (TEA pattern).

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use elm_deps::graph::DepsStats;
use elm_lint::pipeline::LintResult;
use elm_lint::rule::LintError;

// ── Hit testing (set by view, read by mouse handler) ────────────────

/// Last-rendered table geometry for the active screen, recorded by the view.
/// The mouse handler reads this to map clicks to list rows accurately.
#[derive(Debug, Clone, Copy, Default)]
pub struct TableHitTest {
    /// Screen-space y of the first data row.
    pub data_top: u16,
    /// Number of visible data rows.
    pub visible_rows: u16,
    /// Scroll offset applied when rendering.
    pub scroll: usize,
}

// ── Screens ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Lint,
    FixReview,
    Deps,
    Unused,
    Search,
    Help,
}

// ── Input mode ───────────────────────────────────────────────────────

/// Which text input is currently active, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    LintFilter,
    SearchQuery,
}

// ── Application state (Model) ────────────────────────────────────────

pub struct AppState {
    pub screen: Screen,
    pub previous_screen: Screen,
    pub quit: bool,
    pub input_mode: InputMode,

    // Project info.
    pub src_dir: String,
    pub module_count: usize,
    pub file_count: usize,
    pub parse_error_count: usize,
    pub parse_errors: Vec<String>,

    // Lint state.
    pub lint: LintState,

    // Fix review state.
    pub fix_review: FixReviewState,

    // Deps state.
    pub deps: DepsState,

    // Unused state.
    pub unused: UnusedState,

    // Search state.
    pub search: SearchState,

    // Status bar.
    pub status_message: Option<String>,
    /// Monotonic counter bumped on every status_message write. `ClearStatusAfter`
    /// captures the current value so its delayed `ClearStatus` only clears the
    /// message it was scheduled for — not a newer one set in the meantime.
    pub status_gen: u64,
    pub loading: bool,

    /// Geometry of the currently rendered table (set each frame by view code).
    /// Cleared to default on every key/mouse event handled by a non-table screen.
    pub table_hit: Cell<TableHitTest>,
}

pub struct LintState {
    pub result: Option<LintResult>,
    pub selected_index: usize,
    pub filter_text: String,
    /// All errors flattened and cloned once per LintComplete; never mutated by filtering.
    pub base_errors: Vec<(String, LintError)>,
    /// Lowercase `path + '\0' + rule + '\0' + message` per base_error, built
    /// once per LintComplete. Keeps `apply_filter` allocation-free per
    /// keystroke on large lint results.
    pub filter_haystacks: Vec<String>,
    /// Indices into `base_errors` matching the current filter (or all of them).
    pub filtered: Vec<usize>,
    /// Whether source preview is visible (toggled with `p`).
    pub show_preview: bool,
}

impl LintState {
    pub fn visible_len(&self) -> usize {
        self.filtered.len()
    }

    pub fn visible_at(&self, i: usize) -> Option<&(String, LintError)> {
        self.filtered
            .get(i)
            .and_then(|&idx| self.base_errors.get(idx))
    }

    pub fn visible_iter(&self) -> impl Iterator<Item = &(String, LintError)> + '_ {
        self.filtered
            .iter()
            .filter_map(|&i| self.base_errors.get(i))
    }
}

pub struct FixReviewState {
    /// Fixable errors to review — one item per fix, in file/order.
    pub items: Vec<FixReviewItem>,
    /// Parallel mask: `accepted_mask[i] == true` means the user accepted
    /// `items[i]`. Written as the user presses `y`/`a`, read at flush time
    /// to group edits per file. This deferred-flush design is what makes
    /// multi-fix-per-file reviews safe: each fix's edits are merged into
    /// one `apply_fixes` call per file, so later fixes can't overwrite
    /// earlier ones.
    pub accepted_mask: Vec<bool>,
    pub current_index: usize,
    pub accepted_count: usize,
    pub skipped_count: usize,
    /// When true, next 'a' press will actually apply all remaining fixes.
    pub confirm_accept_all: bool,
}

#[derive(Clone)]
pub struct FixReviewItem {
    pub file_path: String,
    pub error: LintError,
    /// Shared across all items from the same file (cheap to clone via Arc).
    /// Used at flush time to apply all accepted edits against pristine source.
    pub original_source: Arc<String>,
    /// Pre-computed unified diff for the preview pane. Built once in
    /// `enter_fix_review` against the pristine source plus only this item's
    /// edits, so the user sees exactly what this fix does in isolation.
    pub diff: Arc<Vec<elm_lint::pipeline::DiffHunk>>,
}

pub struct DepsState {
    pub stats: Option<DepsStats>,
    pub graph_data: Option<Vec<(String, Vec<String>)>>,
    pub sub_view: DepsSubView,
    pub selected_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepsSubView {
    Tree,
    Stats,
    Cycles,
}

pub struct UnusedState {
    pub findings: Vec<UnusedFinding>,
    pub selected_index: usize,
    /// Source preview for the selected finding.
    pub preview: Option<SourcePreview>,
}

/// Cached source preview for a file:line.
pub struct SourcePreview {
    pub file_path: String,
    pub line: usize,
    pub source: Arc<String>,
}

#[derive(Debug, Clone)]
pub struct UnusedFinding {
    pub kind: &'static str,
    pub name: String,
    pub file_path: String,
    pub line: usize,
}

pub struct SearchState {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub selected_index: usize,
    /// Source preview for the selected result.
    pub preview: Option<SourcePreview>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub line: usize,
    pub context: String,
}

impl AppState {
    pub fn new(src_dir: String) -> Self {
        Self {
            screen: Screen::Dashboard,
            previous_screen: Screen::Dashboard,
            quit: false,
            input_mode: InputMode::Normal,
            src_dir,
            module_count: 0,
            file_count: 0,
            parse_error_count: 0,
            parse_errors: Vec::new(),
            lint: LintState {
                result: None,
                selected_index: 0,
                filter_text: String::new(),
                base_errors: Vec::new(),
                filter_haystacks: Vec::new(),
                filtered: Vec::new(),
                show_preview: true,
            },
            fix_review: FixReviewState {
                items: Vec::new(),
                accepted_mask: Vec::new(),
                current_index: 0,
                accepted_count: 0,
                skipped_count: 0,
                confirm_accept_all: false,
            },
            deps: DepsState {
                stats: None,
                graph_data: None,
                sub_view: DepsSubView::Stats,
                selected_index: 0,
            },
            unused: UnusedState {
                findings: Vec::new(),
                selected_index: 0,
                preview: None,
            },
            search: SearchState {
                query: String::new(),
                results: Vec::new(),
                selected_index: 0,
                preview: None,
            },
            status_message: Some("Loading project...".into()),
            status_gen: 1,
            loading: true,
            table_hit: Cell::new(TableHitTest::default()),
        }
    }
}

// ── Messages ─────────────────────────────────────────────────────────

pub enum Msg {
    /// Raw key event — interpreted by update() based on current mode.
    KeyPress(KeyEvent),

    /// Mouse event (clicks, scrolls).
    MouseEvent(MouseEvent),

    // Global.
    Quit,
    Tick,

    // File watcher — carries the paths that changed.
    FileChanged(Vec<String>),

    /// Display an error/warning in the status bar.
    StatusError(String),

    /// Display an informational/success message in the status bar.
    StatusInfo(String),

    /// Auto-clear the status message after a timeout. Carries the
    /// `status_gen` at the time it was scheduled so a stale timer can't
    /// clear a newer message.
    ClearStatus(u64),

    // Async results.
    LintComplete(LintResult),
    DepsComplete {
        stats: DepsStats,
        graph_data: Vec<(String, Vec<String>)>,
    },
    SearchComplete(Vec<SearchResult>),
    PreviewLoaded {
        file_path: String,
        line: usize,
        source: Arc<String>,
    },
    ProjectScanned {
        module_count: usize,
        file_count: usize,
        parse_error_count: usize,
        parse_errors: Vec<String>,
    },
}

// ── Commands (side effects) ──────────────────────────────────────────

pub enum Command {
    None,
    /// Run lint + deps in a single parse pass. Unused findings are derived
    /// from the lint result in the `LintComplete` handler.
    RunAnalyses,
    RunSearch(String),
    ScanProject,
    /// Apply a fix: (file_path, fixed_source).
    ApplyFix(String, String),
    /// Load source preview for a file:line.
    LoadPreview(String, usize),
    /// Schedule a ClearStatus message after a delay. Carries the generation
    /// counter to identify the message being cleared.
    ClearStatusAfter(u64),
    /// Export lint diagnostics to JSON file.
    ExportLintJson(Arc<Vec<(String, LintError)>>),
    Batch(Vec<Command>),
}

/// Set the status message and bump the generation counter. Callers that
/// also want auto-clear should return `Command::ClearStatusAfter(state.status_gen)`
/// afterwards so only this specific message is eligible to be cleared by
/// the resulting delayed `ClearStatus`.
fn set_status(state: &mut AppState, msg: impl Into<String>) {
    state.status_message = Some(msg.into());
    state.status_gen = state.status_gen.wrapping_add(1);
}

// ── Update ───────────────────────────────────────────────────────────

pub fn update(state: &mut AppState, msg: Msg) -> Command {
    match msg {
        Msg::Quit => {
            state.quit = true;
            Command::None
        }
        Msg::Tick => Command::None,

        Msg::KeyPress(key) => handle_key(state, key),

        Msg::MouseEvent(mouse) => handle_mouse(state, mouse),

        Msg::StatusError(msg) => {
            set_status(state, msg);
            state.loading = false;
            Command::ClearStatusAfter(state.status_gen)
        }

        Msg::StatusInfo(msg) => {
            set_status(state, msg);
            state.loading = false;
            Command::ClearStatusAfter(state.status_gen)
        }

        Msg::ClearStatus(msg_gen) => {
            // Only clear the exact message this timer was scheduled for,
            // and only if nothing is currently loading. A stale timer whose
            // generation has been superseded is a no-op.
            if msg_gen == state.status_gen && !state.loading {
                state.status_message = None;
            }
            Command::None
        }

        // File watcher triggered re-analysis.
        Msg::FileChanged(ref paths) => {
            let config_changed = paths.iter().any(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .is_some_and(|n| n == "elm-assist.toml")
            });
            if config_changed {
                // Config change — full rescan needed.
                set_status(state, "Config changed, re-scanning...");
                state.loading = true;
                Command::ScanProject
            } else {
                // .elm file change — skip ScanProject, re-run analyses directly.
                let n = paths.len();
                let label = if n == 1 {
                    paths[0].rsplit('/').next().unwrap_or(&paths[0]).to_string()
                } else {
                    format!("{n} files")
                };
                set_status(state, format!("Re-linting ({label} changed)..."));
                state.loading = true;
                Command::RunAnalyses
            }
        }

        // Async results.
        Msg::LintComplete(result) => {
            let summary = format!(
                "Lint: {} findings in {:.0}ms{}",
                result.total_errors,
                result.elapsed.as_secs_f64() * 1000.0,
                if result.cached { " (cached)" } else { "" },
            );

            // Capture previous selection anchor (file:line) to restore after rebuild.
            let prev_anchor = state
                .lint
                .visible_at(state.lint.selected_index)
                .map(|(path, err)| (path.clone(), err.span.start.line));
            let prev_index = state.lint.selected_index;

            state.lint.result = Some(result);

            // Rebuild base_errors from the fresh result, then reapply filter.
            rebuild_base_errors(state);
            apply_filter(state);

            // Derive unused findings from lint errors (rules prefixed "NoUnused").
            state.unused.findings = derive_unused_findings(&state.lint.base_errors);
            // Reset unused selection only if it's now out of range.
            if state.unused.selected_index >= state.unused.findings.len() {
                state.unused.selected_index = 0;
            }
            // Drop any stale unused preview — the currently-selected finding
            // may now be a different row, and keeping the old preview would
            // show content that doesn't match the highlighted row.
            state.unused.preview = None;

            // Restore selection near the same file:line. Fast path: the
            // common case under watch-mode rebuilds is that the lint set is
            // unchanged or changed far from the cursor, so the old index
            // still points at the same anchor — O(1). Only fall back to the
            // O(N) scan if that check fails.
            if let Some((prev_path, prev_line)) = prev_anchor {
                let still_valid = state
                    .lint
                    .visible_at(prev_index)
                    .is_some_and(|(p, e)| p == &prev_path && e.span.start.line == prev_line);
                state.lint.selected_index = if still_valid {
                    prev_index
                } else {
                    state
                        .lint
                        .visible_iter()
                        .position(|(path, err)| {
                            path == &prev_path && err.span.start.line == prev_line
                        })
                        .unwrap_or(0)
                };
            }
            state.loading = false;
            set_status(state, summary);
            Command::ClearStatusAfter(state.status_gen)
        }
        Msg::DepsComplete { stats, graph_data } => {
            state.deps.stats = Some(stats);
            state.deps.graph_data = Some(graph_data);
            Command::None
        }
        Msg::SearchComplete(results) => {
            let n = results.len();
            let file_count = results
                .iter()
                .map(|r| r.file_path.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len();
            state.search.results = results;
            state.search.selected_index = 0;
            state.search.preview = None;
            if n > 0 {
                set_status(state, format!("Search: {n} results in {file_count} files"));
            } else {
                set_status(state, "Search: no results found");
            }
            state.loading = false;
            Command::ClearStatusAfter(state.status_gen)
        }
        Msg::PreviewLoaded {
            file_path,
            line,
            source,
        } => {
            // Stale-check: only accept preview if it still matches the currently
            // selected item on its screen. If the user navigated away, drop it.
            let matches_current = match state.screen {
                Screen::Unused => state
                    .unused
                    .findings
                    .get(state.unused.selected_index)
                    .is_some_and(|f| f.file_path == file_path && f.line == line),
                Screen::Search => state
                    .search
                    .results
                    .get(state.search.selected_index)
                    .is_some_and(|r| r.file_path == file_path && r.line == line),
                _ => false,
            };
            if matches_current {
                let preview = Some(SourcePreview {
                    file_path,
                    line,
                    source,
                });
                match state.screen {
                    Screen::Unused => state.unused.preview = preview,
                    Screen::Search => state.search.preview = preview,
                    _ => {}
                }
            }
            Command::None
        }
        Msg::ProjectScanned {
            module_count,
            file_count,
            parse_error_count,
            parse_errors,
        } => {
            state.module_count = module_count;
            state.file_count = file_count;
            state.parse_error_count = parse_error_count;
            state.parse_errors = parse_errors;
            set_status(state, "Running lint...");
            Command::RunAnalyses
        }
    }
}

// ── Key handling ─────────────────────────────────────────────────────

fn handle_key(state: &mut AppState, key: KeyEvent) -> Command {
    match state.input_mode {
        InputMode::LintFilter => handle_filter_input(state, key),
        InputMode::SearchQuery => handle_search_input(state, key),
        InputMode::Normal => handle_normal_key(state, key),
    }
}

fn handle_mouse(state: &mut AppState, mouse: MouseEvent) -> Command {
    // Only act on screens that render a list/table.
    let is_list_screen = matches!(
        state.screen,
        Screen::Lint | Screen::Unused | Screen::Search | Screen::Deps
    );
    if !is_list_screen {
        return Command::None;
    }
    // For Deps, only the Tree sub-view has a clickable list.
    if state.screen == Screen::Deps && state.deps.sub_view != DepsSubView::Tree {
        return Command::None;
    }

    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            let hit = state.table_hit.get();
            if hit.visible_rows == 0 || mouse.row < hit.data_top {
                return Command::None;
            }
            let row_within = (mouse.row - hit.data_top) as usize;
            if row_within >= hit.visible_rows as usize {
                return Command::None;
            }
            let len = list_len(state);
            // Deps Tree inlines the selected module's imports as extra rows
            // below its header, so rows below the expansion map to higher
            // module indices. Account for that before mapping row → index.
            let target = if state.screen == Screen::Deps {
                let sel = state.deps.selected_index;
                let n_imports = state
                    .deps
                    .graph_data
                    .as_ref()
                    .and_then(|g| g.get(sel))
                    .map(|(_, imps)| imps.len())
                    .unwrap_or(0);
                if sel < hit.scroll {
                    hit.scroll + row_within
                } else {
                    let sel_row = sel - hit.scroll;
                    if row_within <= sel_row {
                        hit.scroll + row_within
                    } else if row_within <= sel_row + n_imports {
                        // Click on an expanded import row — ignore.
                        return Command::None;
                    } else {
                        hit.scroll + row_within - n_imports
                    }
                }
            } else {
                hit.scroll + row_within
            };
            if target < len
                && let Some(idx) = selected_index_mut(state)
            {
                *idx = target;
            }
            Command::None
        }
        MouseEventKind::ScrollDown => {
            select_next(state);
            Command::None
        }
        MouseEventKind::ScrollUp => {
            select_prev(state);
            Command::None
        }
        _ => Command::None,
    }
}

fn handle_filter_input(state: &mut AppState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            state.input_mode = InputMode::Normal;
            // Keep filter text and results as-is.
        }
        KeyCode::Char(c) => {
            state.lint.filter_text.push(c);
            apply_filter(state);
        }
        KeyCode::Backspace => {
            state.lint.filter_text.pop();
            if state.lint.filter_text.is_empty() {
                state.input_mode = InputMode::Normal;
            }
            apply_filter(state);
        }
        // Arrow keys still navigate the list while filtering.
        KeyCode::Down => select_next(state),
        KeyCode::Up => select_prev(state),
        _ => {}
    }
    Command::None
}

fn handle_search_input(state: &mut AppState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            Command::None
        }
        KeyCode::Enter => {
            state.input_mode = InputMode::Normal;
            let query = state.search.query.clone();
            if query.is_empty() {
                Command::None
            } else {
                set_status(state, format!("Searching: {query}..."));
                Command::RunSearch(query)
            }
        }
        KeyCode::Tab => {
            complete_search_prefix(&mut state.search.query);
            Command::None
        }
        KeyCode::Char(c) => {
            state.search.query.push(c);
            Command::None
        }
        KeyCode::Backspace => {
            state.search.query.pop();
            Command::None
        }
        _ => Command::None,
    }
}

/// Complete a search query type prefix on Tab. All prefixes are ASCII.
fn complete_search_prefix(query: &mut String) {
    const PREFIXES: &[&str] = &[
        "returns:",
        "type:",
        "case-on:",
        "update:",
        "calls:",
        "unused-args:",
        "lambda:",
        "uses:",
        "def:",
        "expr:",
    ];

    // Only complete if the query has no colon yet (prefix not yet typed).
    if query.contains(':') {
        return;
    }

    let input = query.to_lowercase();
    let matches: Vec<&&str> = PREFIXES.iter().filter(|p| p.starts_with(&input)).collect();

    match matches.as_slice() {
        [] => {}
        [only] => *query = only.to_string(),
        _ => {
            // Longest common prefix. All prefixes are ASCII, so byte == char.
            let first = matches[0];
            let lcp_len = (0..first.len())
                .take_while(|&i| {
                    let b = first.as_bytes()[i];
                    matches.iter().all(|m| m.as_bytes().get(i) == Some(&b))
                })
                .count();
            if lcp_len > query.len() {
                *query = first[..lcp_len].to_string();
            }
        }
    }
}

fn handle_normal_key(state: &mut AppState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => {
            if state.screen == Screen::FixReview {
                return cancel_fix_review(state);
            }
            state.quit = true;
            Command::None
        }
        KeyCode::Esc => {
            if state.screen == Screen::FixReview {
                return cancel_fix_review(state);
            }
            // Clear filter if active, otherwise go back.
            if !state.lint.filter_text.is_empty() && state.screen == Screen::Lint {
                state.lint.filter_text.clear();
                apply_filter(state);
            } else {
                std::mem::swap(&mut state.previous_screen, &mut state.screen);
            }
            Command::None
        }

        // Screen navigation. Disabled during FixReview so the user must
        // exit via q/Esc, which routes through exit_fix_review and fires
        // RunAnalyses when fixes were accepted.
        KeyCode::Char('1') if state.screen != Screen::FixReview => {
            switch_screen(state, Screen::Dashboard);
            Command::None
        }
        KeyCode::Char('2') if state.screen != Screen::FixReview => {
            switch_screen(state, Screen::Lint);
            Command::None
        }
        KeyCode::Char('3') if state.screen != Screen::FixReview => {
            switch_screen(state, Screen::Deps);
            Command::None
        }
        KeyCode::Char('4') if state.screen != Screen::FixReview => {
            switch_screen(state, Screen::Unused);
            Command::None
        }
        KeyCode::Char('5') if state.screen != Screen::FixReview => {
            switch_screen(state, Screen::Search);
            Command::None
        }
        KeyCode::Char('?') if state.screen != Screen::FixReview => {
            switch_screen(state, Screen::Help);
            Command::None
        }

        // List navigation.
        KeyCode::Down | KeyCode::Char('j') => {
            select_next(state);
            reload_preview_if_open(state)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            select_prev(state);
            reload_preview_if_open(state)
        }
        KeyCode::PageDown => {
            page_down(state);
            reload_preview_if_open(state)
        }
        KeyCode::PageUp => {
            page_up(state);
            reload_preview_if_open(state)
        }
        KeyCode::Home | KeyCode::Char('g') => {
            select_first(state);
            reload_preview_if_open(state)
        }
        KeyCode::End | KeyCode::Char('G') => {
            select_last(state);
            reload_preview_if_open(state)
        }

        // Enter fix review from lint screen.
        KeyCode::Char('f') if state.screen == Screen::Lint => {
            enter_fix_review(state);
            Command::None
        }

        // Toggle source preview (lint screen).
        KeyCode::Char('p') if state.screen == Screen::Lint => {
            state.lint.show_preview = !state.lint.show_preview;
            Command::None
        }

        // Export lint diagnostics to JSON (lint screen).
        KeyCode::Char('e') if state.screen == Screen::Lint => {
            if state.lint.visible_len() == 0 {
                set_status(state, "No diagnostics to export.");
                return Command::ClearStatusAfter(state.status_gen);
            }
            set_status(state, "Exporting diagnostics...");
            // Materialize the currently-visible (filtered) view into an owned Vec.
            let payload: Vec<(String, LintError)> = state.lint.visible_iter().cloned().collect();
            Command::ExportLintJson(Arc::new(payload))
        }

        // Enter filter mode (lint screen).
        KeyCode::Char('/') if state.screen == Screen::Lint => {
            state.input_mode = InputMode::LintFilter;
            Command::None
        }

        // Enter search input mode (search screen).
        KeyCode::Char('/') if state.screen == Screen::Search => {
            state.input_mode = InputMode::SearchQuery;
            Command::None
        }

        // Fix review actions.
        KeyCode::Char('y') if state.screen == Screen::FixReview => accept_fix(state),
        KeyCode::Char('n') if state.screen == Screen::FixReview => skip_fix(state),
        KeyCode::Char('a') if state.screen == Screen::FixReview => accept_all_fixes(state),

        // Toggle source preview on Enter (unused/search screens).
        KeyCode::Enter if state.screen == Screen::Unused || state.screen == Screen::Search => {
            match state.screen {
                Screen::Unused => {
                    if state.unused.preview.is_some() {
                        state.unused.preview = None;
                        return Command::None;
                    }
                    if let Some(finding) = state.unused.findings.get(state.unused.selected_index) {
                        return Command::LoadPreview(finding.file_path.clone(), finding.line);
                    }
                }
                Screen::Search => {
                    if state.search.preview.is_some() {
                        state.search.preview = None;
                        return Command::None;
                    }
                    if let Some(result) = state.search.results.get(state.search.selected_index) {
                        return Command::LoadPreview(result.file_path.clone(), result.line);
                    }
                }
                _ => {}
            }
            Command::None
        }

        // Manual re-run all analyses. Blocked while already loading to
        // avoid racing a second ScanProject/RunAnalyses against the first.
        KeyCode::Char('r') if state.screen != Screen::FixReview && !state.loading => {
            set_status(state, "Re-running all analyses...");
            state.loading = true;
            Command::ScanProject
        }

        // Deps sub-view toggle.
        KeyCode::Tab if state.screen == Screen::Deps => {
            state.deps.sub_view = match state.deps.sub_view {
                DepsSubView::Tree => DepsSubView::Stats,
                DepsSubView::Stats => DepsSubView::Cycles,
                DepsSubView::Cycles => DepsSubView::Tree,
            };
            Command::None
        }

        _ => Command::None,
    }
}

// ── Navigation helpers ───────────────────────────────────────────────

fn switch_screen(state: &mut AppState, screen: Screen) {
    state.previous_screen = state.screen;
    state.screen = screen;
}

fn select_next(state: &mut AppState) {
    let len = list_len(state);
    if len == 0 {
        return;
    }
    if let Some(idx) = selected_index_mut(state) {
        *idx = (*idx + 1).min(len - 1);
    }
}

fn select_prev(state: &mut AppState) {
    if let Some(idx) = selected_index_mut(state) {
        *idx = idx.saturating_sub(1);
    }
}

fn page_down(state: &mut AppState) {
    let len = list_len(state);
    if len == 0 {
        return;
    }
    if let Some(idx) = selected_index_mut(state) {
        *idx = (*idx + 20).min(len - 1);
    }
}

fn page_up(state: &mut AppState) {
    if let Some(idx) = selected_index_mut(state) {
        *idx = idx.saturating_sub(20);
    }
}

fn select_first(state: &mut AppState) {
    if let Some(idx) = selected_index_mut(state) {
        *idx = 0;
    }
}

fn select_last(state: &mut AppState) {
    let len = list_len(state);
    if len == 0 {
        return;
    }
    if let Some(idx) = selected_index_mut(state) {
        *idx = len - 1;
    }
}

/// When a source preview is visible on Unused or Search, re-issue a
/// LoadPreview command for whatever is now selected. Keeps the preview
/// in sync with the cursor without forcing the user to press Enter.
fn reload_preview_if_open(state: &AppState) -> Command {
    match state.screen {
        Screen::Unused if state.unused.preview.is_some() => state
            .unused
            .findings
            .get(state.unused.selected_index)
            .map(|f| Command::LoadPreview(f.file_path.clone(), f.line))
            .unwrap_or(Command::None),
        Screen::Search if state.search.preview.is_some() => state
            .search
            .results
            .get(state.search.selected_index)
            .map(|r| Command::LoadPreview(r.file_path.clone(), r.line))
            .unwrap_or(Command::None),
        _ => Command::None,
    }
}

fn list_len(state: &AppState) -> usize {
    match state.screen {
        Screen::Lint | Screen::FixReview => state.lint.visible_len(),
        Screen::Unused => state.unused.findings.len(),
        Screen::Search => state.search.results.len(),
        Screen::Deps => state.deps.graph_data.as_ref().map_or(0, |d| d.len()),
        _ => 0,
    }
}

fn selected_index_mut(state: &mut AppState) -> Option<&mut usize> {
    match state.screen {
        Screen::Lint | Screen::FixReview => Some(&mut state.lint.selected_index),
        Screen::Unused => Some(&mut state.unused.selected_index),
        Screen::Search => Some(&mut state.search.selected_index),
        Screen::Deps => Some(&mut state.deps.selected_index),
        Screen::Dashboard | Screen::Help => None,
    }
}

// ── Fix review helpers ───────────────────────────────────────────────

fn enter_fix_review(state: &mut AppState) {
    use elm_lint::fix::apply_fixes;

    let Some(ref result) = state.lint.result else {
        return;
    };

    let mut items = Vec::new();
    let mut paths: Vec<&String> = result.file_errors.keys().collect();
    paths.sort();

    for path in paths {
        let Some(source) = result.sources.get(path) else {
            continue;
        };
        // Wrap once per file — all items from this file share the same Arc.
        let source_arc = Arc::new(source.clone());
        for err in &result.file_errors[path] {
            let Some(ref fix) = err.fix else {
                continue;
            };
            // Compute the per-item preview diff against pristine source.
            // The `fixed` here is NEVER written to disk — the actual writes
            // happen in `flush_accepted_fixes`, which merges every accepted
            // item's edits into one `apply_fixes` call per file. That
            // deferred design is the whole point: writing incrementally
            // would cause later same-file fixes to overwrite earlier ones.
            if let Ok(fixed) = apply_fixes(source, &fix.edits) {
                let diff = Arc::new(elm_lint::pipeline::compute_diff(source, &fixed));
                items.push(FixReviewItem {
                    file_path: path.clone(),
                    error: err.clone(),
                    original_source: Arc::clone(&source_arc),
                    diff,
                });
            }
        }
    }

    if items.is_empty() {
        set_status(state, "No fixable errors.");
        return;
    }

    let n = items.len();
    state.fix_review = FixReviewState {
        items,
        accepted_mask: vec![false; n],
        current_index: 0,
        accepted_count: 0,
        skipped_count: 0,
        confirm_accept_all: false,
    };
    state.previous_screen = state.screen;
    state.screen = Screen::FixReview;
}

fn accept_fix(state: &mut AppState) -> Command {
    state.fix_review.confirm_accept_all = false;
    let idx = state.fix_review.current_index;
    let total = state.fix_review.items.len();
    if idx >= total {
        return finish_fix_review(state);
    }

    state.fix_review.accepted_mask[idx] = true;
    state.fix_review.accepted_count += 1;
    state.fix_review.current_index += 1;

    if state.fix_review.current_index >= total {
        return finish_fix_review(state);
    }
    Command::None
}

fn skip_fix(state: &mut AppState) -> Command {
    state.fix_review.confirm_accept_all = false;
    state.fix_review.skipped_count += 1;
    state.fix_review.current_index += 1;

    if state.fix_review.current_index >= state.fix_review.items.len() {
        return finish_fix_review(state);
    }
    Command::None
}

fn accept_all_fixes(state: &mut AppState) -> Command {
    let idx = state.fix_review.current_index;
    let total = state.fix_review.items.len();
    if idx >= total {
        return finish_fix_review(state);
    }

    // First press: show dry-run summary. Second press: apply.
    if !state.fix_review.confirm_accept_all {
        let fix_count = total - idx;
        let file_count = state.fix_review.items[idx..]
            .iter()
            .map(|i| i.file_path.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        set_status(
            state,
            format!(
                "Will apply {fix_count} fixes across {file_count} files. Press 'a' again to confirm."
            ),
        );
        state.fix_review.confirm_accept_all = true;
        return Command::None;
    }
    state.fix_review.confirm_accept_all = false;

    // Mark every remaining item as accepted, then flush.
    let additional = total - idx;
    for m in &mut state.fix_review.accepted_mask[idx..] {
        *m = true;
    }
    state.fix_review.accepted_count += additional;
    state.fix_review.current_index = total;
    finish_fix_review(state)
}

/// Called when the review reaches its end via y/n/a. Emits the completion
/// status, flushes any accepted fixes, and returns to the lint screen.
fn finish_fix_review(state: &mut AppState) -> Command {
    let msg = format!(
        "Fix review complete: {} accepted, {} skipped",
        state.fix_review.accepted_count, state.fix_review.skipped_count
    );
    set_status(state, msg);
    let cmd = flush_accepted_fixes(state);
    exit_fix_review(state);
    cmd
}

/// Called when the user bails on review via q/Esc. Partial accepts ARE
/// still flushed — cancelling cancels the *review*, not the fixes the user
/// already said yes to.
fn cancel_fix_review(state: &mut AppState) -> Command {
    let msg = format!(
        "Fix review cancelled: {} accepted, {} skipped",
        state.fix_review.accepted_count, state.fix_review.skipped_count
    );
    set_status(state, msg);
    let cmd = flush_accepted_fixes(state);
    exit_fix_review(state);
    cmd
}

/// Group edits from accepted items by file, apply each group in a single
/// `apply_fixes` pass against pristine source, and return a batch that
/// writes every file then re-runs analyses. Returns `Command::None` if
/// nothing was accepted.
fn flush_accepted_fixes(state: &mut AppState) -> Command {
    use elm_lint::fix::apply_fixes;

    if state.fix_review.accepted_count == 0 {
        return Command::None;
    }

    let mut per_file: HashMap<String, (Arc<String>, Vec<elm_lint::rule::Edit>)> = HashMap::new();
    for (i, item) in state.fix_review.items.iter().enumerate() {
        if !state.fix_review.accepted_mask[i] {
            continue;
        }
        // Invariant: every FixReviewItem has error.fix = Some(_),
        // enforced in enter_fix_review.
        let fix = item
            .error
            .fix
            .as_ref()
            .expect("FixReviewItem invariant: fix is Some");
        let entry = per_file
            .entry(item.file_path.clone())
            .or_insert_with(|| (Arc::clone(&item.original_source), Vec::new()));
        entry.1.extend(fix.edits.iter().cloned());
    }

    let mut cmds: Vec<Command> = Vec::with_capacity(per_file.len() + 1);
    for (path, (source, edits)) in per_file {
        if let Ok(fixed) = apply_fixes(&source, &edits) {
            cmds.push(Command::ApplyFix(path, fixed));
        }
    }
    cmds.push(Command::RunAnalyses);
    Command::Batch(cmds)
}

fn exit_fix_review(state: &mut AppState) {
    // Fix review always returns to the lint browser. Force the screen
    // unconditionally rather than asserting — release builds would lose
    // any debug_assert, and S1 previously left this state corrupted when
    // number keys bypassed the exit path.
    state.screen = Screen::Lint;
}

// ── Data helpers ─────────────────────────────────────────────────────

/// Rebuild the base (unfiltered) flat error list from the current LintResult.
/// This is the only place that clones LintError — filter changes only rebuild indices.
fn rebuild_base_errors(state: &mut AppState) {
    state.lint.base_errors = match state.lint.result {
        Some(ref result) => {
            let mut paths: Vec<&String> = result.file_errors.keys().collect();
            paths.sort_unstable();
            let mut flat = Vec::with_capacity(result.file_errors.values().map(Vec::len).sum());
            for path in paths {
                for err in &result.file_errors[path] {
                    flat.push((path.clone(), err.clone()));
                }
            }
            flat
        }
        None => Vec::new(),
    };

    // Precompute a lowercase haystack per error so `apply_filter` can
    // substring-match without allocating on each keystroke.
    state.lint.filter_haystacks = state
        .lint
        .base_errors
        .iter()
        .map(|(path, err)| {
            let mut h = String::with_capacity(path.len() + err.rule.len() + err.message.len() + 2);
            h.push_str(path);
            h.push('\0');
            h.push_str(err.rule);
            h.push('\0');
            h.push_str(&err.message);
            h.make_ascii_lowercase();
            h
        })
        .collect();
}

/// Derive `UnusedFinding`s from lint errors whose rule starts with "NoUnused".
fn derive_unused_findings(base_errors: &[(String, LintError)]) -> Vec<UnusedFinding> {
    let mut findings: Vec<UnusedFinding> = base_errors
        .iter()
        .filter(|(_, err)| err.rule.starts_with("NoUnused"))
        .map(|(path, err)| UnusedFinding {
            kind: err.rule,
            name: err.message.clone(),
            file_path: path.clone(),
            line: err.span.start.line as usize,
        })
        .collect();
    findings.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line.cmp(&b.line)));
    findings
}

/// Reapply the current filter to base_errors, producing a fresh index list.
/// Cheap: uses precomputed lowercase haystacks — no per-keystroke allocations
/// beyond the lowercased filter string and the index Vec itself.
fn apply_filter(state: &mut AppState) {
    let filter = state.lint.filter_text.to_lowercase();
    state.lint.filtered = if filter.is_empty() {
        (0..state.lint.base_errors.len()).collect()
    } else {
        state
            .lint
            .filter_haystacks
            .iter()
            .enumerate()
            .filter_map(|(i, h)| if h.contains(&filter) { Some(i) } else { None })
            .collect()
    };
    state.lint.selected_index = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn dummy_error(rule: &'static str, message: &str, line: u32) -> LintError {
        LintError {
            rule,
            severity: elm_lint::rule::Severity::Error,
            message: message.to_string(),
            span: elm_ast::span::Span {
                start: elm_ast::span::Position {
                    offset: 0,
                    line,
                    column: 1,
                },
                end: elm_ast::span::Position {
                    offset: 10,
                    line,
                    column: 11,
                },
            },
            fix: None,
        }
    }

    fn dummy_fix_item(file: &str, mut error: LintError) -> FixReviewItem {
        use elm_ast::span::{Position, Span};
        use elm_lint::rule::{Edit, Fix};
        // Attach a benign fix so `flush_accepted_fixes` can unwrap
        // `item.error.fix` without hitting the invariant panic. The
        // edit replaces the full "old" source with "new".
        let span = Span {
            start: Position {
                offset: 0,
                line: 1,
                column: 1,
            },
            end: Position {
                offset: 3,
                line: 1,
                column: 4,
            },
        };
        error.fix = Some(Fix {
            edits: vec![Edit::Replace {
                span,
                replacement: "new".into(),
            }],
        });
        FixReviewItem {
            file_path: file.into(),
            error,
            original_source: Arc::new("old".into()),
            diff: Arc::new(Vec::new()),
        }
    }

    fn state_with_errors(errors: Vec<(&str, LintError)>) -> AppState {
        let mut state = AppState::new("src".into());
        state.screen = Screen::Lint;

        let mut file_errors: HashMap<String, Vec<LintError>> = HashMap::new();
        for (path, err) in &errors {
            file_errors
                .entry(path.to_string())
                .or_default()
                .push(err.clone());
        }

        let sources = file_errors
            .keys()
            .map(|k| (k.clone(), String::new()))
            .collect();

        state.lint.result = Some(LintResult {
            file_errors,
            sources,
            total_errors: 0,
            total_warnings: 0,
            total_fixable: 0,
            cached: false,
            elapsed: std::time::Duration::ZERO,
            parse_error_count: 0,
            files_linted: 0,
            rules_active: 0,
        });

        // Populate base_errors + filter_haystacks through the production path
        // so tests exercise the real code.
        rebuild_base_errors(&mut state);
        state.lint.filtered = (0..state.lint.base_errors.len()).collect();

        state
    }

    // ── Screen navigation ──────────────────────────────────────────

    #[test]
    fn number_keys_switch_screens() {
        let mut state = AppState::new("src".into());

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('2'))));
        assert_eq!(state.screen, Screen::Lint);

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('3'))));
        assert_eq!(state.screen, Screen::Deps);

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('4'))));
        assert_eq!(state.screen, Screen::Unused);

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('5'))));
        assert_eq!(state.screen, Screen::Search);

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('1'))));
        assert_eq!(state.screen, Screen::Dashboard);
    }

    #[test]
    fn esc_goes_back_to_previous_screen() {
        let mut state = AppState::new("src".into());
        assert_eq!(state.screen, Screen::Dashboard);

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('2'))));
        assert_eq!(state.screen, Screen::Lint);
        assert_eq!(state.previous_screen, Screen::Dashboard);

        update(&mut state, Msg::KeyPress(key(KeyCode::Esc)));
        assert_eq!(state.screen, Screen::Dashboard);
    }

    #[test]
    fn help_toggle() {
        let mut state = AppState::new("src".into());

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('?'))));
        assert_eq!(state.screen, Screen::Help);

        update(&mut state, Msg::KeyPress(key(KeyCode::Esc)));
        assert_eq!(state.screen, Screen::Dashboard);
    }

    #[test]
    fn q_quits_from_dashboard() {
        let mut state = AppState::new("src".into());
        update(&mut state, Msg::KeyPress(key(KeyCode::Char('q'))));
        assert!(state.quit);
    }

    // ── Input modes ────────────────────────────────────────────────

    #[test]
    fn slash_enters_filter_mode_on_lint_screen() {
        let mut state = AppState::new("src".into());
        state.screen = Screen::Lint;

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('/'))));
        assert_eq!(state.input_mode, InputMode::LintFilter);
    }

    #[test]
    fn slash_enters_search_mode_on_search_screen() {
        let mut state = AppState::new("src".into());
        state.screen = Screen::Search;

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('/'))));
        assert_eq!(state.input_mode, InputMode::SearchQuery);
    }

    #[test]
    fn filter_input_appends_and_esc_exits() {
        let mut state =
            state_with_errors(vec![("src/A.elm", dummy_error("NoDebug", "debug log", 1))]);
        state.input_mode = InputMode::LintFilter;

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('d'))));
        assert_eq!(state.lint.filter_text, "d");
        assert_eq!(state.input_mode, InputMode::LintFilter);

        update(&mut state, Msg::KeyPress(key(KeyCode::Esc)));
        assert_eq!(state.input_mode, InputMode::Normal);
        // Filter text preserved after Esc.
        assert_eq!(state.lint.filter_text, "d");
    }

    #[test]
    fn backspace_to_empty_exits_filter_mode() {
        let mut state =
            state_with_errors(vec![("src/A.elm", dummy_error("NoDebug", "debug log", 1))]);
        state.input_mode = InputMode::LintFilter;
        state.lint.filter_text = "x".into();

        update(&mut state, Msg::KeyPress(key(KeyCode::Backspace)));
        assert_eq!(state.lint.filter_text, "");
        assert_eq!(state.input_mode, InputMode::Normal);
    }

    #[test]
    fn search_input_accumulates_and_enter_submits() {
        let mut state = AppState::new("src".into());
        state.screen = Screen::Search;
        state.input_mode = InputMode::SearchQuery;

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('d'))));
        update(&mut state, Msg::KeyPress(key(KeyCode::Char('e'))));
        update(&mut state, Msg::KeyPress(key(KeyCode::Char('f'))));
        assert_eq!(state.search.query, "def");

        let cmd = update(&mut state, Msg::KeyPress(key(KeyCode::Enter)));
        assert_eq!(state.input_mode, InputMode::Normal);
        assert!(matches!(cmd, Command::RunSearch(q) if q == "def"));
    }

    // ── List navigation ────────────────────────────────────────────

    #[test]
    fn select_next_wraps_at_end() {
        let mut state = state_with_errors(vec![
            ("src/A.elm", dummy_error("R1", "msg1", 1)),
            ("src/B.elm", dummy_error("R2", "msg2", 2)),
        ]);

        assert_eq!(state.lint.selected_index, 0);

        update(&mut state, Msg::KeyPress(key(KeyCode::Down)));
        assert_eq!(state.lint.selected_index, 1);

        // At the end, stays at last item.
        update(&mut state, Msg::KeyPress(key(KeyCode::Down)));
        assert_eq!(state.lint.selected_index, 1);
    }

    #[test]
    fn select_prev_stops_at_zero() {
        let mut state = state_with_errors(vec![("src/A.elm", dummy_error("R1", "msg1", 1))]);

        update(&mut state, Msg::KeyPress(key(KeyCode::Up)));
        assert_eq!(state.lint.selected_index, 0);
    }

    #[test]
    fn select_last_and_first() {
        let mut state = state_with_errors(vec![
            ("src/A.elm", dummy_error("R1", "msg1", 1)),
            ("src/B.elm", dummy_error("R2", "msg2", 2)),
            ("src/C.elm", dummy_error("R3", "msg3", 3)),
        ]);

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('G'))));
        assert_eq!(state.lint.selected_index, 2);

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('g'))));
        assert_eq!(state.lint.selected_index, 0);
    }

    #[test]
    fn navigation_on_empty_list_does_not_panic() {
        let mut state = state_with_errors(vec![]);

        update(&mut state, Msg::KeyPress(key(KeyCode::Down)));
        assert_eq!(state.lint.selected_index, 0);

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('G'))));
        assert_eq!(state.lint.selected_index, 0);
    }

    // ── Filter rebuilding ──────────────────────────────────────────

    #[test]
    fn filter_narrows_results() {
        let mut state = state_with_errors(vec![
            ("src/A.elm", dummy_error("NoDebug", "found debug log", 1)),
            ("src/B.elm", dummy_error("NoUnused", "unused import", 2)),
        ]);
        state.input_mode = InputMode::LintFilter;

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('d'))));
        update(&mut state, Msg::KeyPress(key(KeyCode::Char('e'))));
        update(&mut state, Msg::KeyPress(key(KeyCode::Char('b'))));
        update(&mut state, Msg::KeyPress(key(KeyCode::Char('u'))));
        update(&mut state, Msg::KeyPress(key(KeyCode::Char('g'))));

        // Only the debug error should match.
        assert_eq!(state.lint.visible_len(), 1);
        assert_eq!(state.lint.visible_at(0).unwrap().1.rule, "NoDebug");
    }

    #[test]
    fn filter_resets_selection_index() {
        let mut state = state_with_errors(vec![
            ("src/A.elm", dummy_error("NoDebug", "debug", 1)),
            ("src/B.elm", dummy_error("NoUnused", "unused", 2)),
        ]);
        state.lint.selected_index = 1;
        state.input_mode = InputMode::LintFilter;

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('x'))));
        assert_eq!(state.lint.selected_index, 0);
    }

    // ── Fix review state machine ───────────────────────────────────

    /// Construct a FixReviewState with `items` and a matching-length mask
    /// initialized from `accepted` (or all-false if None).
    fn fix_review_state(items: Vec<FixReviewItem>, accepted: Option<Vec<bool>>) -> FixReviewState {
        let n = items.len();
        let accepted_mask = accepted.unwrap_or_else(|| vec![false; n]);
        assert_eq!(accepted_mask.len(), n);
        let accepted_count = accepted_mask.iter().filter(|&&b| b).count();
        FixReviewState {
            items,
            accepted_mask,
            current_index: 0,
            accepted_count,
            skipped_count: 0,
            confirm_accept_all: false,
        }
    }

    #[test]
    fn fix_review_skip_advances() {
        let mut state = AppState::new("src".into());
        state.screen = Screen::FixReview;
        state.previous_screen = Screen::Lint;
        state.fix_review = fix_review_state(
            vec![
                dummy_fix_item("a.elm", dummy_error("R", "msg", 1)),
                dummy_fix_item("b.elm", dummy_error("R", "msg", 2)),
            ],
            None,
        );

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('n'))));
        assert_eq!(state.fix_review.current_index, 1);
        assert_eq!(state.fix_review.skipped_count, 1);
    }

    #[test]
    fn fix_review_accept_defers_apply_until_finish() {
        // New design: accept_fix just marks the mask and returns None.
        // The write is deferred until the review finishes, so later
        // same-file accepts can't clobber earlier ones.
        let mut state = AppState::new("src".into());
        state.screen = Screen::FixReview;
        state.previous_screen = Screen::Lint;
        state.fix_review = fix_review_state(
            vec![
                dummy_fix_item("a.elm", dummy_error("R", "msg", 1)),
                dummy_fix_item("b.elm", dummy_error("R", "msg", 2)),
            ],
            None,
        );

        let cmd = update(&mut state, Msg::KeyPress(key(KeyCode::Char('y'))));
        assert_eq!(state.fix_review.accepted_count, 1);
        assert_eq!(state.fix_review.current_index, 1);
        assert!(state.fix_review.accepted_mask[0]);
        assert!(!state.fix_review.accepted_mask[1]);
        assert!(matches!(cmd, Command::None));
    }

    #[test]
    fn fix_review_last_accept_exits_and_relints() {
        let mut state = AppState::new("src".into());
        state.screen = Screen::FixReview;
        state.previous_screen = Screen::Lint;
        state.fix_review = fix_review_state(
            vec![dummy_fix_item("a.elm", dummy_error("R", "msg", 1))],
            None,
        );

        let cmd = update(&mut state, Msg::KeyPress(key(KeyCode::Char('y'))));
        assert_eq!(state.screen, Screen::Lint);
        assert!(matches!(cmd, Command::Batch(_)));
    }

    #[test]
    fn fix_review_last_skip_exits() {
        let mut state = AppState::new("src".into());
        state.screen = Screen::FixReview;
        state.previous_screen = Screen::Lint;
        state.fix_review = fix_review_state(
            vec![dummy_fix_item("a.elm", dummy_error("R", "msg", 1))],
            None,
        );

        update(&mut state, Msg::KeyPress(key(KeyCode::Char('n'))));
        assert_eq!(state.screen, Screen::Lint);
        assert_eq!(state.fix_review.skipped_count, 1);
    }

    #[test]
    fn fix_review_q_exits_and_flushes_if_accepted() {
        // User accepted item 0, then hit `q` on item 1. Cancel should still
        // flush the previously-accepted fix.
        let mut state = AppState::new("src".into());
        state.screen = Screen::FixReview;
        state.previous_screen = Screen::Lint;
        state.fix_review = fix_review_state(
            vec![
                dummy_fix_item("a.elm", dummy_error("R", "msg", 1)),
                dummy_fix_item("b.elm", dummy_error("R", "msg", 2)),
            ],
            Some(vec![true, false]),
        );
        state.fix_review.current_index = 1;

        let cmd = update(&mut state, Msg::KeyPress(key(KeyCode::Char('q'))));
        assert_eq!(state.screen, Screen::Lint);
        // Cancel with partial accepts flushes the accepted items — a Batch
        // that wraps ApplyFix(es) plus the trailing RunAnalyses.
        assert!(matches!(cmd, Command::Batch(_)));
    }

    #[test]
    fn fix_review_multi_fix_same_file_merges_edits() {
        // Regression: accepting two fixes on the same file in the
        // incremental flow must NOT issue two separate ApplyFix commands.
        // The pre-refactor behaviour was: each FixReviewItem's fixed_source
        // was computed from pristine source + that item's edits alone, so
        // writing them one at a time meant the second ApplyFix silently
        // overwrote the first. The deferred-flush design merges them into
        // a single per-file apply_fixes call.
        use elm_ast::span::{Position, Span};
        use elm_lint::rule::{Edit, Fix};

        // Use a real source string so apply_fixes succeeds on merge.
        let source = "abcdefghij".to_string();

        let span1 = Span {
            start: Position {
                offset: 0,
                line: 1,
                column: 1,
            },
            end: Position {
                offset: 3,
                line: 1,
                column: 4,
            },
        };
        let span2 = Span {
            start: Position {
                offset: 5,
                line: 1,
                column: 6,
            },
            end: Position {
                offset: 8,
                line: 1,
                column: 9,
            },
        };

        let mut err1 = dummy_error("R", "msg1", 1);
        err1.span = span1;
        err1.fix = Some(Fix {
            edits: vec![Edit::Replace {
                span: span1,
                replacement: "XXX".into(),
            }],
        });
        let mut err2 = dummy_error("R", "msg2", 1);
        err2.span = span2;
        err2.fix = Some(Fix {
            edits: vec![Edit::Replace {
                span: span2,
                replacement: "YYY".into(),
            }],
        });

        let item1 = FixReviewItem {
            file_path: "same.elm".into(),
            error: err1,
            original_source: Arc::new(source.clone()),
            diff: Arc::new(Vec::new()),
        };
        let item2 = FixReviewItem {
            file_path: "same.elm".into(),
            error: err2,
            original_source: Arc::new(source),
            diff: Arc::new(Vec::new()),
        };

        let mut state = AppState::new("src".into());
        state.screen = Screen::FixReview;
        state.previous_screen = Screen::Lint;
        state.fix_review = fix_review_state(vec![item1, item2], None);

        // Accept both in sequence.
        update(&mut state, Msg::KeyPress(key(KeyCode::Char('y'))));
        let cmd = update(&mut state, Msg::KeyPress(key(KeyCode::Char('y'))));

        let Command::Batch(cmds) = cmd else {
            panic!("expected Batch, got different command");
        };
        let applies: Vec<&Command> = cmds
            .iter()
            .filter(|c| matches!(c, Command::ApplyFix(_, _)))
            .collect();
        assert_eq!(
            applies.len(),
            1,
            "two accepted fixes on the same file must collapse to a single ApplyFix"
        );
        // And that one ApplyFix must contain BOTH edits merged.
        if let Command::ApplyFix(_, fixed) = applies[0] {
            assert!(
                fixed.contains("XXX") && fixed.contains("YYY"),
                "merged apply_fixes output must include both replacements, got: {fixed:?}"
            );
        }
    }

    // ── Msg handlers ───────────────────────────────────────────────

    #[test]
    fn lint_complete_populates_state() {
        let mut state = AppState::new("src".into());
        state.loading = true;

        let mut file_errors = HashMap::new();
        file_errors.insert(
            "src/A.elm".to_string(),
            vec![dummy_error("NoDebug", "debug", 1)],
        );

        let result = LintResult {
            file_errors,
            sources: HashMap::new(),
            total_errors: 1,
            total_warnings: 0,
            total_fixable: 0,
            cached: false,
            elapsed: std::time::Duration::ZERO,
            parse_error_count: 0,
            files_linted: 1,
            rules_active: 1,
        };

        update(&mut state, Msg::LintComplete(result));
        assert!(!state.loading);
        assert_eq!(state.lint.visible_len(), 1);
        assert!(state.lint.result.is_some());
    }

    #[test]
    fn status_error_sets_message() {
        let mut state = AppState::new("src".into());
        state.loading = true;

        update(&mut state, Msg::StatusError("something broke".to_string()));
        assert_eq!(state.status_message.as_deref(), Some("something broke"));
        assert!(!state.loading);
    }

    #[test]
    fn file_changed_triggers_relint() {
        let mut state = AppState::new("src".into());
        let cmd = update(&mut state, Msg::FileChanged(vec!["src/Main.elm".into()]));
        assert!(state.loading);
        assert!(matches!(cmd, Command::RunAnalyses));
        // .elm changes skip ScanProject, go straight to the unified analyses pass.
        assert!(state.status_message.as_ref().unwrap().contains("Main.elm"));
    }

    #[test]
    fn search_tab_completes_unique_prefix() {
        let mut q = "ret".to_string();
        complete_search_prefix(&mut q);
        assert_eq!(q, "returns:");
    }

    #[test]
    fn search_tab_completes_longest_common_prefix() {
        // "un" matches only "unused-args:" — unique match.
        let mut q = "un".to_string();
        complete_search_prefix(&mut q);
        assert_eq!(q, "unused-args:");
    }

    #[test]
    fn search_tab_noop_after_colon() {
        let mut q = "returns:Maybe".to_string();
        complete_search_prefix(&mut q);
        assert_eq!(q, "returns:Maybe");
    }

    #[test]
    fn config_changed_triggers_full_rescan() {
        let mut state = AppState::new("src".into());
        let cmd = update(&mut state, Msg::FileChanged(vec!["elm-assist.toml".into()]));
        assert!(state.loading);
        assert!(matches!(cmd, Command::ScanProject));
        assert!(
            state
                .status_message
                .as_ref()
                .unwrap()
                .contains("Config changed")
        );
    }

    // ── Deps sub-view cycling ──────────────────────────────────────

    #[test]
    fn tab_cycles_deps_sub_views() {
        let mut state = AppState::new("src".into());
        state.screen = Screen::Deps;
        assert_eq!(state.deps.sub_view, DepsSubView::Stats);

        update(&mut state, Msg::KeyPress(key(KeyCode::Tab)));
        assert_eq!(state.deps.sub_view, DepsSubView::Cycles);

        update(&mut state, Msg::KeyPress(key(KeyCode::Tab)));
        assert_eq!(state.deps.sub_view, DepsSubView::Tree);

        update(&mut state, Msg::KeyPress(key(KeyCode::Tab)));
        assert_eq!(state.deps.sub_view, DepsSubView::Stats);
    }
}
