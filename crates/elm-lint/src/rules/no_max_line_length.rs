use elm_ast::span::{Position, Span};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports lines that exceed a maximum length (default: 120 characters).
pub struct NoMaxLineLength {
    pub max_length: usize,
}

impl Default for NoMaxLineLength {
    fn default() -> Self {
        Self { max_length: 120 }
    }
}

impl Rule for NoMaxLineLength {
    fn name(&self) -> &'static str {
        "NoMaxLineLength"
    }

    fn description(&self) -> &'static str {
        "Lines should not exceed the maximum length"
    }

    fn configure(&mut self, options: &toml::Value) -> Result<(), String> {
        if let Some(val) = options.get("max_length") {
            self.max_length = val
                .as_integer()
                .ok_or_else(|| "max_length must be an integer".to_string())?
                as usize;
        }
        Ok(())
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();
        let mut offset = 0usize;

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let len = line.len();
            if len > self.max_length {
                let line_num = (line_idx + 1) as u32;
                errors.push(LintError {
                    rule: self.name(),
                    severity: Severity::Warning,
                    message: format!(
                        "Line {} is {} characters long (maximum is {})",
                        line_num, len, self.max_length
                    ),
                    span: Span {
                        start: Position {
                            offset,
                            line: line_num,
                            column: 1,
                        },
                        end: Position {
                            offset: offset + len,
                            line: line_num,
                            column: (len + 1) as u32,
                        },
                    },
                    fix: None,
                });
            }
            // +1 for the newline character
            offset += len + 1;
        }

        errors
    }
}
