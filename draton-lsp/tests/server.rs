use draton_lsp::server::LspServer;
use serde_json::json;

#[test]
fn initialize_advertises_symbols_and_completion() {
    let mut server = LspServer::new();
    let responses = server
        .handle(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }))
        .expect("initialize response");

    let result = &responses[0]["result"]["capabilities"];
    assert_eq!(result["hoverProvider"], json!(true));
    assert_eq!(result["definitionProvider"], json!(true));
    assert_eq!(result["documentSymbolProvider"], json!(true));
    assert_eq!(result["workspaceSymbolProvider"], json!(true));
    assert!(result.get("completionProvider").is_some());
}

#[test]
fn document_symbols_and_completion_work_for_open_document() {
    let mut server = LspServer::new();
    let uri = "file:///tmp/sample.dt";
    let source = r#"
import { greet } from sample

@type {
    helper: (Int) -> Int
    main: () -> Int
}

fn helper(value) {
    return value + 1
}

fn main() {
    let count = helper(1)
    return count
}
"#;

    let responses = server
        .handle(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "draton",
                    "version": 1,
                    "text": source
                }
            }
        }))
        .expect("didOpen response");
    assert_eq!(responses.len(), 1, "expected diagnostics notification");

    let symbol_response = server
        .handle(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": { "uri": uri }
            }
        }))
        .expect("document symbols");
    let symbols = symbol_response[0]["result"]
        .as_array()
        .expect("symbol array");
    assert!(symbols.iter().any(|symbol| symbol["name"] == "helper"));
    assert!(symbols.iter().any(|symbol| symbol["name"] == "main"));

    let completion_response = server
        .handle(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 14, "character": 5 }
            }
        }))
        .expect("completion response");
    let completions = completion_response[0]["result"]
        .as_array()
        .expect("completion array");
    assert!(completions.iter().any(|item| item["label"] == "helper"));
    assert!(completions.iter().any(|item| item["label"] == "count"));
}
