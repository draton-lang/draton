use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use draton_lexer::Lexer;

pub(crate) fn run(path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let result = Lexer::new(&source).tokenize();

    for token in result.tokens {
        println!(
            "{:?}\t{}\t{}:{}-{}",
            token.kind,
            escape_lexeme(&token.lexeme),
            token.span.line,
            token.span.col,
            token.span.end
        );
    }

    for error in result.errors {
        eprintln!("{error}");
    }

    Ok(())
}

fn escape_lexeme(lexeme: &str) -> String {
    lexeme.escape_default().to_string()
}
