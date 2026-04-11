use crate::rule::Edit;

/// Error returned when fix application fails.
#[derive(Debug)]
pub enum FixError {
    /// Two edits have overlapping byte ranges.
    OverlappingEdits,
    /// An edit's span is out of bounds for the source text.
    OutOfBounds,
}

impl std::fmt::Display for FixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixError::OverlappingEdits => write!(f, "overlapping edits"),
            FixError::OutOfBounds => write!(f, "edit span out of bounds"),
        }
    }
}

/// Apply a set of edits to source text, returning the modified source.
///
/// Edits are applied from the end of the file backwards so that byte offsets
/// remain valid as the string is mutated. Overlapping edits are rejected.
pub fn apply_fixes(source: &str, edits: &[Edit]) -> Result<String, FixError> {
    if edits.is_empty() {
        return Ok(source.to_string());
    }

    // Convert each edit to a (start_offset, end_offset, replacement) triple.
    let mut ops: Vec<(usize, usize, String)> = edits
        .iter()
        .map(|edit| match edit {
            Edit::Replace { span, replacement } => {
                (span.start.offset, span.end.offset, replacement.clone())
            }
            Edit::InsertAfter { span, text } => (span.end.offset, span.end.offset, text.clone()),
            Edit::Remove { span } => (span.start.offset, span.end.offset, String::new()),
        })
        .collect();

    // Sort by start offset descending (apply from end to start).
    // For equal start offsets, sort by end offset descending.
    ops.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    // Check for overlapping edits.
    for window in ops.windows(2) {
        let later = &window[0]; // higher offset (applied first)
        let earlier = &window[1]; // lower offset (applied second)
        // Earlier edit ends after later edit starts = overlap.
        if earlier.1 > later.0 {
            return Err(FixError::OverlappingEdits);
        }
    }

    // Check bounds.
    let len = source.len();
    for (start, end, _) in &ops {
        if *start > len || *end > len || *start > *end {
            return Err(FixError::OutOfBounds);
        }
    }

    // Apply edits from end to start.
    let mut result = source.to_string();
    for (start, end, replacement) in ops {
        result.replace_range(start..end, &replacement);
    }

    Ok(result)
}

/// Create a Remove edit that also consumes the trailing newline (if any),
/// so removing an import line doesn't leave a blank line.
pub fn remove_line(source: &str, edit: &Edit) -> Edit {
    let span = edit.span();
    let end = span.end.offset;

    // Extend past the trailing newline if there is one.
    let extended_end = if end < source.len() && source.as_bytes()[end] == b'\n' {
        end + 1
    } else {
        end
    };

    let mut extended_span = span;
    extended_span.end.offset = extended_end;

    Edit::Remove {
        span: extended_span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elm_ast::span::{Position, Span};

    fn span(start: usize, end: usize) -> Span {
        Span {
            start: Position {
                offset: start,
                line: 1,
                column: 1,
            },
            end: Position {
                offset: end,
                line: 1,
                column: 1,
            },
        }
    }

    #[test]
    fn single_replace() {
        let source = "hello world";
        let edits = vec![Edit::Replace {
            span: span(6, 11),
            replacement: "rust".into(),
        }];
        assert_eq!(apply_fixes(source, &edits).unwrap(), "hello rust");
    }

    #[test]
    fn single_remove() {
        let source = "hello world";
        let edits = vec![Edit::Remove { span: span(5, 11) }];
        assert_eq!(apply_fixes(source, &edits).unwrap(), "hello");
    }

    #[test]
    fn single_insert_after() {
        let source = "hello world";
        let edits = vec![Edit::InsertAfter {
            span: span(5, 5),
            text: " beautiful".into(),
        }];
        assert_eq!(
            apply_fixes(source, &edits).unwrap(),
            "hello beautiful world"
        );
    }

    #[test]
    fn multiple_non_overlapping() {
        let source = "aaa bbb ccc";
        let edits = vec![
            Edit::Replace {
                span: span(0, 3),
                replacement: "AAA".into(),
            },
            Edit::Replace {
                span: span(8, 11),
                replacement: "CCC".into(),
            },
        ];
        assert_eq!(apply_fixes(source, &edits).unwrap(), "AAA bbb CCC");
    }

    #[test]
    fn overlapping_edits_rejected() {
        let source = "hello world";
        let edits = vec![
            Edit::Replace {
                span: span(0, 7),
                replacement: "hi".into(),
            },
            Edit::Replace {
                span: span(5, 11),
                replacement: "there".into(),
            },
        ];
        assert!(matches!(
            apply_fixes(source, &edits),
            Err(FixError::OverlappingEdits)
        ));
    }

    #[test]
    fn out_of_bounds_rejected() {
        let source = "hello";
        let edits = vec![Edit::Remove { span: span(0, 100) }];
        assert!(matches!(
            apply_fixes(source, &edits),
            Err(FixError::OutOfBounds)
        ));
    }

    #[test]
    fn empty_edits() {
        let source = "hello";
        assert_eq!(apply_fixes(source, &[]).unwrap(), "hello");
    }

    #[test]
    fn remove_line_extends_past_newline() {
        let source = "line1\nline2\nline3";
        let edit = Edit::Remove { span: span(0, 5) };
        let extended = remove_line(source, &edit);
        let result = apply_fixes(source, &[extended]).unwrap();
        assert_eq!(result, "line2\nline3");
    }
}
