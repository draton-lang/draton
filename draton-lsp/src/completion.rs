use crate::document::DocumentStore;
use draton_ast::{Block, ClassMember, Expr, FnDef, Item, Program, Stmt};
use lsp_types::{CompletionItem, CompletionItemKind};
use serde_json::{json, to_value, Value};
use std::collections::BTreeMap;

pub fn completion(docs: &DocumentStore, uri: &str, line: usize, col: usize) -> Option<Value> {
    let doc = docs.get(uri)?;
    let analysis = doc.analysis.as_ref()?;
    let program = analysis.ast_program.as_ref()?;
    let offset = position_to_offset(&doc.text, line, col);
    let items = build_completion_items(program, offset);
    to_value(items).ok().or_else(|| Some(json!([])))
}

fn build_completion_items(program: &Program, offset: usize) -> Vec<CompletionItem> {
    let mut items = BTreeMap::<String, CompletionItem>::new();
    add_keywords(&mut items);
    add_top_level_items(program, &mut items);
    collect_local_items(program, offset, &mut items);
    items.into_values().collect()
}

fn add_keywords(items: &mut BTreeMap<String, CompletionItem>) {
    for keyword in [
        "let",
        "return",
        "if",
        "elif",
        "else",
        "match",
        "class",
        "layer",
        "interface",
        "@type",
        "fn",
        "import",
    ] {
        items.insert(
            keyword.to_string(),
            CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("keyword".to_string()),
                ..CompletionItem::default()
            },
        );
    }
}

fn add_top_level_items(program: &Program, items: &mut BTreeMap<String, CompletionItem>) {
    for item in &program.items {
        match item {
            Item::Fn(def) | Item::PanicHandler(def) | Item::OomHandler(def) => {
                items.insert(def.name.clone(), function_item(&def.name));
            }
            Item::Class(def) => {
                items.insert(
                    def.name.clone(),
                    CompletionItem {
                        label: def.name.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some("class".to_string()),
                        ..CompletionItem::default()
                    },
                );
                for member in &def.members {
                    match member {
                        ClassMember::Field(field) => {
                            items.insert(
                                field.name.clone(),
                                CompletionItem {
                                    label: field.name.clone(),
                                    kind: Some(CompletionItemKind::FIELD),
                                    detail: Some(def.name.clone()),
                                    ..CompletionItem::default()
                                },
                            );
                        }
                        ClassMember::Method(method) => {
                            items.insert(method.name.clone(), method_item(&method.name, &def.name));
                        }
                        ClassMember::Layer(layer) => {
                            for method in &layer.methods {
                                items.insert(
                                    method.name.clone(),
                                    method_item(
                                        &method.name,
                                        &format!("{}.{}", def.name, layer.name),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            Item::Interface(def) => {
                items.insert(
                    def.name.clone(),
                    CompletionItem {
                        label: def.name.clone(),
                        kind: Some(CompletionItemKind::INTERFACE),
                        detail: Some("interface".to_string()),
                        ..CompletionItem::default()
                    },
                );
                for method in &def.methods {
                    items.insert(method.name.clone(), method_item(&method.name, &def.name));
                }
            }
            Item::Enum(def) => {
                items.insert(
                    def.name.clone(),
                    CompletionItem {
                        label: def.name.clone(),
                        kind: Some(CompletionItemKind::ENUM),
                        detail: Some("enum".to_string()),
                        ..CompletionItem::default()
                    },
                );
            }
            Item::Error(def) => {
                items.insert(
                    def.name.clone(),
                    CompletionItem {
                        label: def.name.clone(),
                        kind: Some(CompletionItemKind::STRUCT),
                        detail: Some("error".to_string()),
                        ..CompletionItem::default()
                    },
                );
            }
            Item::Const(def) => {
                items.insert(
                    def.name.clone(),
                    CompletionItem {
                        label: def.name.clone(),
                        kind: Some(CompletionItemKind::CONSTANT),
                        detail: Some("const".to_string()),
                        ..CompletionItem::default()
                    },
                );
            }
            Item::Import(def) => {
                for import_item in &def.items {
                    let visible = import_item.alias.as_deref().unwrap_or(&import_item.name);
                    items.insert(
                        visible.to_string(),
                        CompletionItem {
                            label: visible.to_string(),
                            kind: Some(CompletionItemKind::MODULE),
                            detail: Some(format!("import from {}", def.module.join("."))),
                            ..CompletionItem::default()
                        },
                    );
                }
            }
            Item::Extern(_) | Item::TypeBlock(_) => {}
        }
    }
}

fn collect_local_items(
    program: &Program,
    offset: usize,
    items: &mut BTreeMap<String, CompletionItem>,
) {
    for item in &program.items {
        match item {
            Item::Fn(def) | Item::PanicHandler(def) | Item::OomHandler(def) => {
                collect_fn_locals(def, offset, items);
            }
            Item::Class(def) => {
                for member in &def.members {
                    match member {
                        ClassMember::Method(method) => collect_fn_locals(method, offset, items),
                        ClassMember::Layer(layer) => {
                            for method in &layer.methods {
                                collect_fn_locals(method, offset, items);
                            }
                        }
                        ClassMember::Field(_) => {}
                    }
                }
            }
            Item::Interface(def) => {
                for method in &def.methods {
                    collect_fn_locals(method, offset, items);
                }
            }
            _ => {}
        }
    }
}

fn collect_fn_locals(def: &FnDef, offset: usize, items: &mut BTreeMap<String, CompletionItem>) {
    let Some(body) = &def.body else {
        return;
    };
    if offset < body.span.start || offset > body.span.end {
        return;
    }
    for param in &def.params {
        items.insert(
            param.name.clone(),
            CompletionItem {
                label: param.name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(format!("parameter in {}", def.name)),
                ..CompletionItem::default()
            },
        );
    }
    collect_block_locals(body, offset, items);
}

fn collect_block_locals(
    block: &Block,
    offset: usize,
    items: &mut BTreeMap<String, CompletionItem>,
) {
    if offset < block.span.start || offset > block.span.end {
        return;
    }
    for stmt in &block.stmts {
        if stmt_span_end(stmt) > offset {
            break;
        }
        match stmt {
            Stmt::Let(inner) => {
                items.insert(
                    inner.name.clone(),
                    CompletionItem {
                        label: inner.name.clone(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("local binding".to_string()),
                        ..CompletionItem::default()
                    },
                );
            }
            Stmt::For(inner) => {
                items.insert(
                    inner.name.clone(),
                    CompletionItem {
                        label: inner.name.clone(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("loop binding".to_string()),
                        ..CompletionItem::default()
                    },
                );
                collect_block_locals(&inner.body, offset, items);
            }
            Stmt::If(inner) => {
                collect_block_locals(&inner.then_branch, offset, items);
                if let Some(draton_ast::ElseBranch::Block(block)) = &inner.else_branch {
                    collect_block_locals(block, offset, items);
                }
            }
            Stmt::While(inner) => collect_block_locals(&inner.body, offset, items),
            Stmt::Block(inner)
            | Stmt::UnsafeBlock(inner)
            | Stmt::PointerBlock(inner)
            | Stmt::ComptimeBlock(inner) => collect_block_locals(inner, offset, items),
            Stmt::Expr(Expr::Lambda(_, _, _))
            | Stmt::LetDestructure(_)
            | Stmt::Assign(_)
            | Stmt::Return(_)
            | Stmt::Expr(_)
            | Stmt::Spawn(_)
            | Stmt::AsmBlock(_, _)
            | Stmt::IfCompile(_)
            | Stmt::GcConfig(_)
            | Stmt::TypeBlock(_) => {}
        }
    }
}

fn stmt_span_end(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::Let(inner) => inner.span.end,
        Stmt::LetDestructure(inner) => inner.span.end,
        Stmt::Assign(inner) => inner.span.end,
        Stmt::Return(inner) => inner.span.end,
        Stmt::Expr(expr) => expr.span().end,
        Stmt::If(inner) => inner.span.end,
        Stmt::For(inner) => inner.span.end,
        Stmt::While(inner) => inner.span.end,
        Stmt::Spawn(inner) => inner.span.end,
        Stmt::Block(inner)
        | Stmt::UnsafeBlock(inner)
        | Stmt::PointerBlock(inner)
        | Stmt::ComptimeBlock(inner) => inner.span.end,
        Stmt::AsmBlock(_, span) => span.end,
        Stmt::IfCompile(inner) => inner.span.end,
        Stmt::GcConfig(inner) => inner.span.end,
        Stmt::TypeBlock(inner) => inner.span.end,
    }
}

fn function_item(name: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_string(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some("function".to_string()),
        ..CompletionItem::default()
    }
}

fn method_item(name: &str, owner: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_string(),
        kind: Some(CompletionItemKind::METHOD),
        detail: Some(owner.to_string()),
        ..CompletionItem::default()
    }
}

fn position_to_offset(text: &str, line: usize, col: usize) -> usize {
    let mut current_line = 0usize;
    let mut current_col = 0usize;
    for (index, ch) in text.char_indices() {
        if current_line == line && current_col == col {
            return index;
        }
        if ch == '\n' {
            current_line += 1;
            current_col = 0;
        } else {
            current_col += 1;
        }
    }
    text.len()
}
