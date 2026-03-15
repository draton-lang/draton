use crate::document::DocumentStore;
use serde_json::Value;

pub fn goto_definition(
    _docs: &DocumentStore,
    _uri: &str,
    _line: usize,
    _col: usize,
) -> Option<Value> {
    None
}
