use crate::document::DocumentStore;
use serde_json::Value;

pub fn hover(_docs: &DocumentStore, _uri: &str, _line: usize, _col: usize) -> Option<Value> {
    None
}
