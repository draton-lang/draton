use crate::document::DocumentStore;
use anyhow::Result;
use lsp_types::{
    HoverProviderCapability, InitializeResult, OneOf, PositionEncodingKind, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use serde_json::{json, to_value, Value};

pub struct LspServer {
    pub docs: DocumentStore,
    pub initialized: bool,
    pub shutdown_requested: bool,
}

impl LspServer {
    pub fn new() -> Self {
        Self {
            docs: DocumentStore::new(),
            initialized: false,
            shutdown_requested: false,
        }
    }

    pub fn handle(&mut self, msg: &Value) -> Result<Vec<Value>> {
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            return Ok(Vec::new());
        };
        let id = msg.get("id").cloned();

        let responses = match method {
            "initialize" => {
                self.initialized = true;
                vec![self.respond(
                    id,
                    to_value(InitializeResult {
                        capabilities: ServerCapabilities {
                            position_encoding: Some(PositionEncodingKind::UTF8),
                            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                                TextDocumentSyncKind::FULL,
                            )),
                            hover_provider: Some(HoverProviderCapability::Simple(true)),
                            definition_provider: Some(OneOf::Left(true)),
                            ..ServerCapabilities::default()
                        },
                        server_info: Some(ServerInfo {
                            name: "draton-lsp".to_string(),
                            version: Some("0.1.0".to_string()),
                        }),
                    })?,
                )]
            }
            "initialized" => Vec::new(),
            "shutdown" => {
                self.shutdown_requested = true;
                vec![self.respond(id, Value::Null)]
            }
            "exit" => {
                std::process::exit(if self.shutdown_requested { 0 } else { 1 });
            }
            _ => {
                if let Some(request_id) = id {
                    vec![self.respond(Some(request_id), Value::Null)]
                } else {
                    Vec::new()
                }
            }
        };

        Ok(responses)
    }

    fn respond(&self, id: Option<Value>, result: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id.unwrap_or(Value::Null),
            "result": result,
        })
    }
}
