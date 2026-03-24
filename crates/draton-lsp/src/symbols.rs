#![allow(deprecated)]

use crate::analysis::offset_to_position;
use crate::document::DocumentStore;
use draton_ast::{ClassMember, Item, Program, Span};
use lsp_types::{
    DocumentSymbol, Location, Position, Range, SymbolInformation, SymbolKind, SymbolTag, Url,
};
use serde_json::{json, to_value, Value};

pub fn document_symbols(docs: &DocumentStore, uri: &str) -> Option<Value> {
    let doc = docs.get(uri)?;
    let analysis = doc.analysis.as_ref()?;
    let program = analysis.ast_program.as_ref()?;
    let symbols = build_document_symbols(&doc.text, program);
    to_value(symbols).ok()
}

pub fn workspace_symbols(docs: &DocumentStore, query: &str) -> Value {
    let mut out = Vec::<SymbolInformation>::new();
    let needle = query.to_ascii_lowercase();
    for (uri, doc) in docs.iter() {
        let Some(analysis) = doc.analysis.as_ref() else {
            continue;
        };
        let Some(program) = analysis.ast_program.as_ref() else {
            continue;
        };
        let Ok(url) = Url::parse(uri) else {
            continue;
        };
        collect_workspace_symbols(&doc.text, program, &url, &needle, &mut out);
    }
    to_value(out).unwrap_or_else(|_| json!([]))
}

fn build_document_symbols(text: &str, program: &Program) -> Vec<DocumentSymbol> {
    program
        .items
        .iter()
        .filter_map(|item| item_to_document_symbol(text, item))
        .collect()
}

fn item_to_document_symbol(text: &str, item: &Item) -> Option<DocumentSymbol> {
    match item {
        Item::Fn(def) | Item::PanicHandler(def) | Item::OomHandler(def) => Some(DocumentSymbol {
            name: def.name.clone(),
            detail: None,
            kind: SymbolKind::FUNCTION,
            tags: None::<Vec<SymbolTag>>,
            deprecated: None,
            range: range_for_span(text, def.span),
            selection_range: range_for_span(text, def.span),
            children: None,
        }),
        Item::Class(def) => {
            let children = def
                .members
                .iter()
                .map(|member| match member {
                    ClassMember::Field(field) => DocumentSymbol {
                        name: field.name.clone(),
                        detail: None,
                        kind: SymbolKind::FIELD,
                        tags: None::<Vec<SymbolTag>>,
                        deprecated: None,
                        range: range_for_span(text, field.span),
                        selection_range: range_for_span(text, field.span),
                        children: None,
                    },
                    ClassMember::Method(method) => DocumentSymbol {
                        name: method.name.clone(),
                        detail: None,
                        kind: SymbolKind::METHOD,
                        tags: None::<Vec<SymbolTag>>,
                        deprecated: None,
                        range: range_for_span(text, method.span),
                        selection_range: range_for_span(text, method.span),
                        children: None,
                    },
                    ClassMember::Layer(layer) => DocumentSymbol {
                        name: layer.name.clone(),
                        detail: Some("layer".to_string()),
                        kind: SymbolKind::NAMESPACE,
                        tags: None::<Vec<SymbolTag>>,
                        deprecated: None,
                        range: range_for_span(text, layer.span),
                        selection_range: range_for_span(text, layer.span),
                        children: Some(
                            layer
                                .methods
                                .iter()
                                .map(|method| DocumentSymbol {
                                    name: method.name.clone(),
                                    detail: None,
                                    kind: SymbolKind::METHOD,
                                    tags: None::<Vec<SymbolTag>>,
                                    deprecated: None,
                                    range: range_for_span(text, method.span),
                                    selection_range: range_for_span(text, method.span),
                                    children: None,
                                })
                                .collect(),
                        ),
                    },
                })
                .collect();
            Some(DocumentSymbol {
                name: def.name.clone(),
                detail: None,
                kind: SymbolKind::CLASS,
                tags: None::<Vec<SymbolTag>>,
                deprecated: None,
                range: range_for_span(text, def.span),
                selection_range: range_for_span(text, def.span),
                children: Some(children),
            })
        }
        Item::Interface(def) => Some(DocumentSymbol {
            name: def.name.clone(),
            detail: None,
            kind: SymbolKind::INTERFACE,
            tags: None::<Vec<SymbolTag>>,
            deprecated: None,
            range: range_for_span(text, def.span),
            selection_range: range_for_span(text, def.span),
            children: Some(
                def.methods
                    .iter()
                    .map(|method| DocumentSymbol {
                        name: method.name.clone(),
                        detail: None,
                        kind: SymbolKind::METHOD,
                        tags: None::<Vec<SymbolTag>>,
                        deprecated: None,
                        range: range_for_span(text, method.span),
                        selection_range: range_for_span(text, method.span),
                        children: None,
                    })
                    .collect(),
            ),
        }),
        Item::Enum(def) => Some(DocumentSymbol {
            name: def.name.clone(),
            detail: None,
            kind: SymbolKind::ENUM,
            tags: None::<Vec<SymbolTag>>,
            deprecated: None,
            range: range_for_span(text, def.span),
            selection_range: range_for_span(text, def.span),
            children: None,
        }),
        Item::Error(def) => Some(DocumentSymbol {
            name: def.name.clone(),
            detail: None,
            kind: SymbolKind::STRUCT,
            tags: None::<Vec<SymbolTag>>,
            deprecated: None,
            range: range_for_span(text, def.span),
            selection_range: range_for_span(text, def.span),
            children: None,
        }),
        Item::Const(def) => Some(DocumentSymbol {
            name: def.name.clone(),
            detail: None,
            kind: SymbolKind::CONSTANT,
            tags: None::<Vec<SymbolTag>>,
            deprecated: None,
            range: range_for_span(text, def.span),
            selection_range: range_for_span(text, def.span),
            children: None,
        }),
        Item::Import(_) | Item::Extern(_) | Item::TypeBlock(_) => None,
    }
}

fn collect_workspace_symbols(
    text: &str,
    program: &Program,
    uri: &Url,
    needle: &str,
    out: &mut Vec<SymbolInformation>,
) {
    for symbol in build_document_symbols(text, program) {
        flatten_symbol(uri, None, needle, &symbol, out);
    }
}

fn flatten_symbol(
    uri: &Url,
    container_name: Option<String>,
    needle: &str,
    symbol: &DocumentSymbol,
    out: &mut Vec<SymbolInformation>,
) {
    let matches = needle.is_empty() || symbol.name.to_ascii_lowercase().contains(needle);
    if matches {
        out.push(SymbolInformation {
            name: symbol.name.clone(),
            kind: symbol.kind,
            tags: symbol.tags.clone(),
            deprecated: symbol.deprecated,
            location: Location {
                uri: uri.clone(),
                range: symbol.selection_range,
            },
            container_name: container_name.clone(),
        });
    }
    if let Some(children) = &symbol.children {
        for child in children {
            flatten_symbol(uri, Some(symbol.name.clone()), needle, child, out);
        }
    }
}

fn range_for_span(text: &str, span: Span) -> Range {
    let (start_line, start_col) = offset_to_position(text, span.start);
    let end = span.end.max(span.start + 1).min(text.len());
    let (end_line, end_col) = offset_to_position(text, end);
    Range {
        start: Position {
            line: start_line as u32,
            character: start_col as u32,
        },
        end: Position {
            line: end_line as u32,
            character: end_col as u32,
        },
    }
}
