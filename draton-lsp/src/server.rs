use crate::document::DocumentStore;
use anyhow::Result;
use lsp_types::{
    CompletionOptions, HoverProviderCapability, InitializeResult, OneOf, PositionEncodingKind,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
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
                            document_symbol_provider: Some(OneOf::Left(true)),
                            workspace_symbol_provider: Some(OneOf::Left(true)),
                            completion_provider: Some(CompletionOptions::default()),
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
            "textDocument/didOpen" => {
                let Some(uri) = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                let Some(text) = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("text"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                let version = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("version"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                self.docs.open(uri.to_string(), text.to_string(), version);
                vec![self.publish_diagnostics(uri)]
            }
            "textDocument/didChange" => {
                let Some(uri) = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                let version = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("version"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let Some(text) = msg
                    .get("params")
                    .and_then(|params| params.get("contentChanges"))
                    .and_then(Value::as_array)
                    .and_then(|changes| changes.last())
                    .and_then(|change| change.get("text"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                self.docs.update(uri.to_string(), text.to_string(), version);
                vec![self.publish_diagnostics(uri)]
            }
            "textDocument/didClose" => {
                let Some(uri) = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                self.docs.close(uri);
                vec![self.publish_empty_diagnostics(uri)]
            }
            "textDocument/hover" => {
                let Some(uri) = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                let Some(line) = msg
                    .get("params")
                    .and_then(|params| params.get("position"))
                    .and_then(|position| position.get("line"))
                    .and_then(Value::as_u64)
                else {
                    return Ok(Vec::new());
                };
                let Some(character) = msg
                    .get("params")
                    .and_then(|params| params.get("position"))
                    .and_then(|position| position.get("character"))
                    .and_then(Value::as_u64)
                else {
                    return Ok(Vec::new());
                };
                vec![self.respond(
                    id,
                    crate::hover::hover(&self.docs, uri, line as usize, character as usize)
                        .unwrap_or(Value::Null),
                )]
            }
            "textDocument/definition" => {
                let Some(uri) = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                let Some(line) = msg
                    .get("params")
                    .and_then(|params| params.get("position"))
                    .and_then(|position| position.get("line"))
                    .and_then(Value::as_u64)
                else {
                    return Ok(Vec::new());
                };
                let Some(character) = msg
                    .get("params")
                    .and_then(|params| params.get("position"))
                    .and_then(|position| position.get("character"))
                    .and_then(Value::as_u64)
                else {
                    return Ok(Vec::new());
                };
                vec![self.respond(
                    id,
                    crate::goto_def::goto_definition(
                        &self.docs,
                        uri,
                        line as usize,
                        character as usize,
                    )
                    .unwrap_or(Value::Null),
                )]
            }
            "textDocument/documentSymbol" => {
                let Some(uri) = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                vec![self.respond(
                    id,
                    crate::symbols::document_symbols(&self.docs, uri).unwrap_or(Value::Null),
                )]
            }
            "workspace/symbol" => {
                let query = msg
                    .get("params")
                    .and_then(|params| params.get("query"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                vec![self.respond(
                    id,
                    crate::symbols::workspace_symbols(&self.docs, query),
                )]
            }
            "textDocument/completion" => {
                let Some(uri) = msg
                    .get("params")
                    .and_then(|params| params.get("textDocument"))
                    .and_then(|doc| doc.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return Ok(Vec::new());
                };
                let Some(line) = msg
                    .get("params")
                    .and_then(|params| params.get("position"))
                    .and_then(|position| position.get("line"))
                    .and_then(Value::as_u64)
                else {
                    return Ok(Vec::new());
                };
                let Some(character) = msg
                    .get("params")
                    .and_then(|params| params.get("position"))
                    .and_then(|position| position.get("character"))
                    .and_then(Value::as_u64)
                else {
                    return Ok(Vec::new());
                };
                vec![self.respond(
                    id,
                    crate::completion::completion(
                        &self.docs,
                        uri,
                        line as usize,
                        character as usize,
                    )
                    .unwrap_or(Value::Null),
                )]
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

    fn publish_diagnostics(&self, uri: &str) -> Value {
        let diagnostics = self
            .docs
            .get(uri)
            .and_then(|doc| doc.analysis.as_ref())
            .map(crate::diagnostics::collect_diagnostics)
            .unwrap_or_else(crate::diagnostics::empty_diagnostics);
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": diagnostics,
            }
        })
    }

    fn publish_empty_diagnostics(&self, uri: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": crate::diagnostics::empty_diagnostics(),
            }
        })
    }
}
