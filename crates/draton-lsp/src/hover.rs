use crate::document::DocumentStore;
use serde_json::{json, Value};

pub fn hover(docs: &DocumentStore, uri: &str, line: usize, col: usize) -> Option<Value> {
    let doc = docs.get(uri)?;
    let analysis = doc.analysis.as_ref()?;

    let mut best = None::<&crate::analysis::SpanType>;
    for span_type in &analysis.span_type_map {
        let contains = position_in_span(line, col, span_type);
        if !contains {
            continue;
        }
        match best {
            None => best = Some(span_type),
            Some(current) => {
                let current_len = current.end_offset.saturating_sub(current.start_offset);
                let next_len = span_type.end_offset.saturating_sub(span_type.start_offset);
                if next_len <= current_len {
                    best = Some(span_type);
                }
            }
        }
    }

    best.map(|span_type| {
        json!({
            "contents": {
                "kind": "markdown",
                "value": format!("`{}`", span_type.type_str),
            }
        })
    })
}

fn position_in_span(line: usize, col: usize, span: &crate::analysis::SpanType) -> bool {
    let after_start = line > span.line || (line == span.line && col >= span.col);
    let before_end = line < span.end_line || (line == span.end_line && col < span.end_col);
    after_start && before_end
}
