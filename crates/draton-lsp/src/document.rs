use crate::analysis::{analyze, AnalysisResult};
use std::collections::HashMap;

pub struct Document {
    pub text: String,
    pub version: i64,
    pub analysis: Option<AnalysisResult>,
}

pub struct DocumentStore {
    docs: HashMap<String, Document>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    pub fn open(&mut self, uri: String, text: String, version: i64) {
        let analysis = analyze(&text);
        self.docs.insert(
            uri,
            Document {
                text,
                version,
                analysis: Some(analysis),
            },
        );
    }

    pub fn update(&mut self, uri: String, text: String, version: i64) {
        self.open(uri, text, version);
    }

    pub fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }

    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.docs.get(uri)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Document)> {
        self.docs.iter()
    }
}
