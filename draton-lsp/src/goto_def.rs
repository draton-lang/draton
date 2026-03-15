use crate::document::DocumentStore;
use serde_json::{json, Value};

pub fn goto_definition(
    docs: &DocumentStore,
    uri: &str,
    line: usize,
    col: usize,
) -> Option<Value> {
    let doc = docs.get(uri)?;
    let analysis = doc.analysis.as_ref()?;
    let mut best = None::<&crate::analysis::DefEntry>;
    for entry in &analysis.def_map {
        let contains = position_in_entry(line, col, entry);
        if !contains {
            continue;
        }
        match best {
            None => best = Some(entry),
            Some(current) => {
                let current_len = current.end_offset.saturating_sub(current.start_offset);
                let next_len = entry.end_offset.saturating_sub(entry.start_offset);
                if next_len <= current_len {
                    best = Some(entry);
                }
            }
        }
    }

    best.map(|entry| {
        let target_uri = if entry.def_uri.is_empty() {
            uri.to_string()
        } else {
            entry.def_uri.clone()
        };
        json!({
            "uri": target_uri,
            "range": {
                "start": { "line": entry.def_line, "character": entry.def_col },
                "end": { "line": entry.def_line, "character": entry.def_col + 1 },
            }
        })
    })
}

fn position_in_entry(line: usize, col: usize, entry: &crate::analysis::DefEntry) -> bool {
    let after_start = line > entry.ref_line || (line == entry.ref_line && col >= entry.ref_col);
    let before_end =
        line < entry.ref_end_line || (line == entry.ref_end_line && col < entry.ref_end_col);
    after_start && before_end
}
