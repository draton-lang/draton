use draton_ast::{BinOp, Expr, FStrPart, MatchArmBody, UnOp};
use draton_lexer::Lexer;
use draton_parser::Parser;
use pretty_assertions::assert_eq;

fn parse_expr(source: &str) -> Expr {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let (expr, errors) = Parser::new(lexed.tokens).parse_expression_only();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);
    expr.expect("expression should parse")
}

#[test]
fn parses_every_literal_family() {
    assert!(matches!(parse_expr("42"), Expr::IntLit(42, _)));
    assert!(matches!(parse_expr("3.14"), Expr::FloatLit(value, _) if (value - 3.14).abs() < 1e-9));
    assert!(matches!(parse_expr("0xFF"), Expr::IntLit(255, _)));
    assert!(matches!(parse_expr("0b1010"), Expr::IntLit(10, _)));
    assert!(matches!(parse_expr("\"hello\""), Expr::StrLit(value, _) if value == "hello"));
    assert!(matches!(parse_expr("true"), Expr::BoolLit(true, _)));
    assert!(matches!(parse_expr("false"), Expr::BoolLit(false, _)));
    assert!(matches!(parse_expr("None"), Expr::NoneLit(_)));
}

#[test]
fn respects_binary_precedence() {
    let expr = parse_expr("1 + 2 * 3");
    match expr {
        Expr::BinOp(lhs, BinOp::Add, rhs, _) => {
            assert!(matches!(*lhs, Expr::IntLit(1, _)));
            match *rhs {
                Expr::BinOp(lhs, BinOp::Mul, rhs, _) => {
                    assert!(matches!(*lhs, Expr::IntLit(2, _)));
                    assert!(matches!(*rhs, Expr::IntLit(3, _)));
                }
                other => panic!("expected multiply rhs, got {other:?}"),
            }
        }
        other => panic!("expected add, got {other:?}"),
    }
}

#[test]
fn parses_unary_operators() {
    assert!(matches!(parse_expr("!flag"), Expr::UnOp(UnOp::Not, _, _)));
    assert!(matches!(parse_expr("&value"), Expr::UnOp(UnOp::Ref, _, _)));
    assert!(matches!(parse_expr("*ptr"), Expr::UnOp(UnOp::Deref, _, _)));
}

#[test]
fn parses_method_chains() {
    let expr = parse_expr("[1, 2, 3].filter(lambda x => x > 1).map(lambda x => x * 2)");
    match expr {
        Expr::MethodCall(target, name, args, _) => {
            assert_eq!(name, "map");
            assert_eq!(args.len(), 1);
            assert!(matches!(args[0], Expr::Lambda(_, _, _)));
            assert!(matches!(*target, Expr::MethodCall(_, _, _, _)));
        }
        other => panic!("expected chained method call, got {other:?}"),
    }
}

#[test]
fn parses_lambda_expressions() {
    let expr = parse_expr("lambda x, y => x + y");
    match expr {
        Expr::Lambda(params, body, _) => {
            assert_eq!(params, vec!["x".to_string(), "y".to_string()]);
            assert!(matches!(*body, Expr::BinOp(_, BinOp::Add, _, _)));
        }
        other => panic!("expected lambda, got {other:?}"),
    }
}

#[test]
fn splits_fstring_interpolations() {
    let expr = parse_expr("f\"hi {name} v{version}\"");
    match expr {
        Expr::FStrLit(parts, _) => {
            assert_eq!(parts.len(), 4);
            assert_eq!(parts[0], FStrPart::Literal("hi ".to_string()));
            assert!(matches!(&parts[1], FStrPart::Interp(Expr::Ident(name, _)) if name == "name"));
            assert_eq!(parts[2], FStrPart::Literal(" v".to_string()));
            assert!(
                matches!(&parts[3], FStrPart::Interp(Expr::Ident(name, _)) if name == "version")
            );
        }
        other => panic!("expected f-string, got {other:?}"),
    }
}

#[test]
fn parses_match_expressions() {
    let expr = parse_expr("match status { 200 => print(\"OK\"), _ => print(\"unknown\") }");
    match expr {
        Expr::Match(subject, arms, _) => {
            assert!(matches!(*subject, Expr::Ident(name, _) if name == "status"));
            assert_eq!(arms.len(), 2);
            assert!(matches!(arms[0].body, MatchArmBody::Expr(_)));
        }
        other => panic!("expected match, got {other:?}"),
    }
}

#[test]
fn parses_nullish_result_and_cast_expressions() {
    assert!(matches!(parse_expr("a ?? b"), Expr::Nullish(_, _, _)));
    assert!(matches!(parse_expr("Ok(x)"), Expr::Ok(_, _)));
    assert!(matches!(parse_expr("Err(x)"), Expr::Err(_, _)));
    assert!(matches!(parse_expr("x as Int"), Expr::Cast(_, _, _)));
}
