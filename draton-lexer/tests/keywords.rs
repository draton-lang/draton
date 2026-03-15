use draton_lexer::{Lexer, TokenKind};
use pretty_assertions::assert_eq;

#[test]
fn lexes_all_keywords() {
    let source = "let mut fn return if elif else for while in match class layer extends implements interface enum error pub import from as spawn chan const lambda";
    let result = Lexer::new(source).tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            TokenKind::Let,
            TokenKind::Mut,
            TokenKind::Fn,
            TokenKind::Return,
            TokenKind::If,
            TokenKind::Elif,
            TokenKind::Else,
            TokenKind::For,
            TokenKind::While,
            TokenKind::In,
            TokenKind::Match,
            TokenKind::Class,
            TokenKind::Layer,
            TokenKind::Extends,
            TokenKind::Implements,
            TokenKind::Interface,
            TokenKind::Enum,
            TokenKind::Error,
            TokenKind::Pub,
            TokenKind::Import,
            TokenKind::From,
            TokenKind::As,
            TokenKind::Spawn,
            TokenKind::Chan,
            TokenKind::Const,
            TokenKind::Lambda,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_all_at_keywords() {
    let source =
        "@type @unsafe @pointer @asm @comptime @if @gc_config @panic_handler @oom_handler @extern";
    let result = Lexer::new(source).tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            TokenKind::AtType,
            TokenKind::AtUnsafe,
            TokenKind::AtPointer,
            TokenKind::AtAsm,
            TokenKind::AtComptime,
            TokenKind::AtIf,
            TokenKind::AtGcConfig,
            TokenKind::AtPanicHandler,
            TokenKind::AtOomHandler,
            TokenKind::AtExtern,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn does_not_confuse_keyword_prefixes_with_keywords() {
    let source = "letter mutable fnord returning iffy elsewhere formal whiley inside matchbox classy layered extendsable implementsort interfaceable enumerate erroring public imports assert spawnable channel constant lambdax @typewriter";
    let result = Lexer::new(source).tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        {
            let mut expected = vec![TokenKind::Ident; 24];
            expected.extend([TokenKind::At, TokenKind::Ident, TokenKind::Eof]);
            expected
        }
    );
}
