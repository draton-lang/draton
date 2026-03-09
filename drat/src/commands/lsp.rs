use std::io::{self, BufRead, Read, Write};

use anyhow::{Context, Result};
use serde_json::{json, Value};

pub(crate) fn run() -> Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();

    loop {
        let Some(message) = read_message(&mut input)? else {
            break;
        };
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();
        match method {
            Some("initialize") => {
                write_message(
                    &mut output,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "capabilities": {
                                "textDocumentSync": 1,
                                "documentFormattingProvider": true
                            }
                        }
                    }),
                )?;
            }
            Some("shutdown") => {
                write_message(&mut output, &json!({"jsonrpc":"2.0","id":id,"result":null}))?;
            }
            Some("exit") => break,
            Some("textDocument/formatting") => {
                write_message(&mut output, &json!({"jsonrpc":"2.0","id":id,"result":[]}))?;
            }
            Some(_) => {
                if let Some(id) = id {
                    write_message(&mut output, &json!({"jsonrpc":"2.0","id":id,"result":null}))?;
                }
            }
            None => {}
        }
    }
    Ok(())
}

fn read_message<R: BufRead + Read>(reader: &mut R) -> Result<Option<Value>> {
    let mut content_length = None::<usize>;
    let mut header = String::new();
    loop {
        header.clear();
        let read = reader.read_line(&mut header)?;
        if read == 0 {
            return Ok(None);
        }
        let line = header.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .context("header Content-Length khong hop le")?,
            );
        }
    }
    let Some(content_length) = content_length else {
        return Ok(None);
    };
    let mut buf = vec![0u8; content_length];
    reader.read_exact(&mut buf)?;
    let value = serde_json::from_slice(&buf).context("payload LSP khong hop le")?;
    Ok(Some(value))
}

fn write_message<W: Write>(writer: &mut W, value: &Value) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}
