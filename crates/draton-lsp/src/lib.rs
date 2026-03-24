pub mod analysis;
pub mod completion;
pub mod diagnostics;
pub mod document;
pub mod goto_def;
pub mod hover;
pub mod server;
pub mod symbols;

use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, BufRead, Read, Write};

pub fn run_stdio() -> Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut server = server::LspServer::new();

    loop {
        let Some(message) = read_message(&mut input)? else {
            break;
        };
        let responses = server.handle(&message)?;
        for response in responses {
            write_message(&mut output, &response)?;
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
                    .context("invalid Content-Length header")?,
            );
        }
    }

    let Some(content_length) = content_length else {
        return Ok(None);
    };

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    let message = serde_json::from_slice(&body).context("invalid LSP payload")?;
    Ok(Some(message))
}

fn write_message<W: Write>(writer: &mut W, value: &Value) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}
