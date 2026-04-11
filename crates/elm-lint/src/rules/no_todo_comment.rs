use elm_ast::span::{Position, Span};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports TODO and FIXME comments in source code.
pub struct NoTodoComment;

impl Rule for NoTodoComment {
    fn name(&self) -> &'static str {
        "NoTodoComment"
    }

    fn description(&self) -> &'static str {
        "TODO/FIXME comments should be resolved before shipping"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();
        let patterns = ["TODO", "FIXME"];

        for (line_idx, line) in ctx.source.lines().enumerate() {
            for pattern in &patterns {
                if let Some(col) = line.find(pattern) {
                    let line_start_offset = ctx.source.as_bytes()
                        .iter()
                        .enumerate()
                        .scan(0usize, |current_line, (offset, &byte)| {
                            if *current_line == line_idx {
                                return Some(Some(offset));
                            }
                            if byte == b'\n' {
                                *current_line += 1;
                            }
                            Some(None)
                        })
                        .flatten()
                        .next()
                        .unwrap_or(0);

                    let start_offset = line_start_offset + col;
                    let end_offset = start_offset + pattern.len();

                    errors.push(LintError {
                        rule: "NoTodoComment",
                    severity: Severity::Warning,
                        message: format!("Found `{pattern}` comment"),
                        span: Span {
                            start: Position {
                                offset: start_offset,
                                line: (line_idx + 1) as u32,
                                column: (col + 1) as u32,
                            },
                            end: Position {
                                offset: end_offset,
                                line: (line_idx + 1) as u32,
                                column: (col + 1 + pattern.len()) as u32,
                            },
                        },
                        fix: None,
                    });
                }
            }
        }
        errors
    }
}
