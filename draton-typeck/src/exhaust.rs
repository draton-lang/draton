use std::collections::{HashMap, HashSet};

use crate::typed_ast::{Type, TypedExpr, TypedExprKind};

/// A normalized match pattern used by the exhaustiveness checker.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_` or a variable binding.
    Wildcard,
    /// A literal pattern.
    Literal(LiteralPat),
    /// An enum variant written as `Enum.Variant`.
    EnumVariant(String, String),
    /// A tuple pattern.
    Tuple(Vec<Pattern>),
    /// A named constructor such as `Ok(_)`, `Err(_)`, or `Some(_)`.
    Named(String),
    /// Reserved for future `p1 | p2` patterns.
    Or(Vec<Pattern>),
}

/// A literal pattern payload.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralPat {
    /// Integer literal.
    Int(i64),
    /// Float literal.
    Float(f64),
    /// Boolean literal.
    Bool(bool),
    /// String literal.
    Str(String),
    /// `None`.
    None,
}

/// The shape of the match subject after type resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum SubjectKind {
    /// `Bool`.
    Bool,
    /// An enum and its known variants.
    Enum(String, Vec<String>),
    /// Integral types.
    Int,
    /// Floating-point types.
    Float,
    /// Strings.
    String,
    /// Tuple subjects.
    Tuple(Vec<SubjectKind>),
    /// `Option[T]`.
    Option(Box<SubjectKind>),
    /// `Result[T, E]`.
    Result(Box<SubjectKind>, Box<SubjectKind>),
    /// Any other subject kind.
    Other,
}

/// Simplified exhaustiveness and redundancy checker for Draton match expressions.
#[derive(Debug, Clone, Default)]
pub struct ExhaustivenessChecker {
    /// Known enum definitions keyed by name.
    pub enum_defs: HashMap<String, Vec<String>>,
}

impl ExhaustivenessChecker {
    /// Checks whether `patterns` exhaustively cover `subject`.
    pub fn check(&self, patterns: &[Pattern], subject: &SubjectKind) -> Vec<String> {
        match subject {
            SubjectKind::Bool => self.check_bool(patterns),
            SubjectKind::Enum(name, variants) => self.check_enum(patterns, name, variants),
            SubjectKind::Option(inner) => self.check_option(patterns, inner),
            SubjectKind::Result(ok, err) => self.check_result(patterns, ok, err),
            SubjectKind::Int | SubjectKind::Float | SubjectKind::String | SubjectKind::Other => {
                if patterns.iter().any(Self::covers_all) {
                    Vec::new()
                } else {
                    vec!["_ (wildcard required for non-enumerable type)".to_string()]
                }
            }
            SubjectKind::Tuple(slots) => self.check_tuple(patterns, slots),
        }
    }

    /// Returns user-facing labels for redundant patterns.
    pub fn check_redundancy(
        &self,
        patterns: &[Pattern],
        subject: &SubjectKind,
    ) -> Vec<(usize, String)> {
        match subject {
            SubjectKind::Bool => self.redundant_finite(patterns, |pattern| match pattern {
                Pattern::Literal(LiteralPat::Bool(true)) => Some("true".to_string()),
                Pattern::Literal(LiteralPat::Bool(false)) => Some("false".to_string()),
                Pattern::Wildcard => Some("_".to_string()),
                _ => None,
            }),
            SubjectKind::Enum(enum_name, _) => {
                self.redundant_finite(patterns, |pattern| match pattern {
                    Pattern::EnumVariant(found_enum, variant) if found_enum == enum_name => {
                        Some(format!("{enum_name}.{variant}"))
                    }
                    Pattern::Wildcard => Some("_".to_string()),
                    _ => None,
                })
            }
            SubjectKind::Option(_) => self.redundant_finite(patterns, |pattern| match pattern {
                Pattern::Literal(LiteralPat::None) => Some("None".to_string()),
                Pattern::Named(name) if name == "Some" => Some("Some(_)".to_string()),
                Pattern::Wildcard => Some("_".to_string()),
                _ => None,
            }),
            SubjectKind::Result(_, _) => self.redundant_finite(patterns, |pattern| match pattern {
                Pattern::Named(name) if name == "Ok" => Some("Ok(_)".to_string()),
                Pattern::Named(name) if name == "Err" => Some("Err(_)".to_string()),
                Pattern::Wildcard => Some("_".to_string()),
                _ => None,
            }),
            SubjectKind::Int | SubjectKind::Float | SubjectKind::String | SubjectKind::Other => {
                self.redundant_finite(patterns, Self::literal_or_wildcard_label)
            }
            SubjectKind::Tuple(_) => {
                let mut seen_wildcard = false;
                let mut redundant = Vec::new();
                for (index, pattern) in patterns.iter().enumerate() {
                    if seen_wildcard {
                        redundant.push((index, display_pattern(pattern)));
                        continue;
                    }
                    if matches!(pattern, Pattern::Wildcard) {
                        seen_wildcard = true;
                    }
                }
                redundant
            }
        }
    }

    fn covers_all(pattern: &Pattern) -> bool {
        matches!(pattern, Pattern::Wildcard)
    }

    fn check_bool(&self, patterns: &[Pattern]) -> Vec<String> {
        let has_true = patterns.iter().any(|pattern| {
            matches!(pattern, Pattern::Literal(LiteralPat::Bool(true))) || Self::covers_all(pattern)
        });
        let has_false = patterns.iter().any(|pattern| {
            matches!(pattern, Pattern::Literal(LiteralPat::Bool(false)))
                || Self::covers_all(pattern)
        });
        let mut missing = Vec::new();
        if !has_true {
            missing.push("true".to_string());
        }
        if !has_false {
            missing.push("false".to_string());
        }
        missing
    }

    fn check_enum(
        &self,
        patterns: &[Pattern],
        enum_name: &str,
        variants: &[String],
    ) -> Vec<String> {
        if patterns.iter().any(Self::covers_all) {
            return Vec::new();
        }
        let covered = patterns
            .iter()
            .filter_map(|pattern| match pattern {
                Pattern::EnumVariant(found_enum, variant) if found_enum == enum_name => {
                    Some(variant.as_str())
                }
                _ => None,
            })
            .collect::<HashSet<_>>();
        variants
            .iter()
            .filter(|variant| !covered.contains(variant.as_str()))
            .map(|variant| format!("{enum_name}.{variant}"))
            .collect()
    }

    fn check_option(&self, patterns: &[Pattern], _inner: &SubjectKind) -> Vec<String> {
        if patterns.iter().any(Self::covers_all) {
            return Vec::new();
        }
        let has_some = patterns
            .iter()
            .any(|pattern| matches!(pattern, Pattern::Named(name) if name == "Some"));
        let has_none = patterns
            .iter()
            .any(|pattern| matches!(pattern, Pattern::Literal(LiteralPat::None)));
        let mut missing = Vec::new();
        if !has_some {
            missing.push("Some(_)".to_string());
        }
        if !has_none {
            missing.push("None".to_string());
        }
        missing
    }

    fn check_result(
        &self,
        patterns: &[Pattern],
        _ok: &SubjectKind,
        _err: &SubjectKind,
    ) -> Vec<String> {
        if patterns.iter().any(Self::covers_all) {
            return Vec::new();
        }
        let has_ok = patterns
            .iter()
            .any(|pattern| matches!(pattern, Pattern::Named(name) if name == "Ok"));
        let has_err = patterns
            .iter()
            .any(|pattern| matches!(pattern, Pattern::Named(name) if name == "Err"));
        let mut missing = Vec::new();
        if !has_ok {
            missing.push("Ok(_)".to_string());
        }
        if !has_err {
            missing.push("Err(_)".to_string());
        }
        missing
    }

    fn check_tuple(&self, patterns: &[Pattern], slots: &[SubjectKind]) -> Vec<String> {
        if patterns.iter().any(Self::covers_all) {
            return Vec::new();
        }
        let has_full_tuple = patterns.iter().any(|pattern| match pattern {
            Pattern::Tuple(subpatterns) => subpatterns.len() == slots.len(),
            _ => false,
        });
        if has_full_tuple {
            Vec::new()
        } else {
            vec![format!(
                "({})",
                slots.iter().map(|_| "_").collect::<Vec<_>>().join(", ")
            )]
        }
    }

    fn redundant_finite<F>(&self, patterns: &[Pattern], mut label_of: F) -> Vec<(usize, String)>
    where
        F: FnMut(&Pattern) -> Option<String>,
    {
        let mut seen = HashSet::new();
        let mut saw_wildcard = false;
        let mut redundant = Vec::new();
        for (index, pattern) in patterns.iter().enumerate() {
            if saw_wildcard {
                redundant.push((index, display_pattern(pattern)));
                continue;
            }
            let Some(label) = label_of(pattern) else {
                continue;
            };
            if label == "_" {
                saw_wildcard = true;
                continue;
            }
            if !seen.insert(label.clone()) {
                redundant.push((index, label));
            }
        }
        redundant
    }

    fn literal_or_wildcard_label(pattern: &Pattern) -> Option<String> {
        match pattern {
            Pattern::Literal(LiteralPat::Int(value)) => Some(value.to_string()),
            Pattern::Literal(LiteralPat::Float(value)) => Some(value.to_string()),
            Pattern::Literal(LiteralPat::Bool(value)) => Some(value.to_string()),
            Pattern::Literal(LiteralPat::Str(value)) => Some(format!("{value:?}")),
            Pattern::Literal(LiteralPat::None) => Some("None".to_string()),
            Pattern::Wildcard => Some("_".to_string()),
            _ => None,
        }
    }
}

/// Classifies a resolved type into a subject kind that the exhaustiveness checker understands.
pub fn classify_subject(ty: &Type, enum_defs: &HashMap<String, Vec<String>>) -> SubjectKind {
    match ty {
        Type::Bool => SubjectKind::Bool,
        Type::Int
        | Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64 => SubjectKind::Int,
        Type::Float | Type::Float32 | Type::Float64 => SubjectKind::Float,
        Type::String => SubjectKind::String,
        Type::Option(inner) => SubjectKind::Option(Box::new(classify_subject(inner, enum_defs))),
        Type::Result(ok, err) => SubjectKind::Result(
            Box::new(classify_subject(ok, enum_defs)),
            Box::new(classify_subject(err, enum_defs)),
        ),
        Type::Tuple(items) => SubjectKind::Tuple(
            items
                .iter()
                .map(|item| classify_subject(item, enum_defs))
                .collect(),
        ),
        Type::Named(name, args) if args.is_empty() => enum_defs
            .get(name)
            .cloned()
            .map(|variants| SubjectKind::Enum(name.clone(), variants))
            .unwrap_or(SubjectKind::Other),
        _ => SubjectKind::Other,
    }
}

/// Extracts a normalized pattern from a typed match-arm expression.
pub fn extract_pattern(expr: &TypedExpr) -> Pattern {
    match &expr.kind {
        TypedExprKind::Ident(name) if name == "_" => Pattern::Wildcard,
        TypedExprKind::Ident(_) => Pattern::Wildcard,
        TypedExprKind::IntLit(value) => Pattern::Literal(LiteralPat::Int(*value)),
        TypedExprKind::FloatLit(value) => Pattern::Literal(LiteralPat::Float(*value)),
        TypedExprKind::BoolLit(value) => Pattern::Literal(LiteralPat::Bool(*value)),
        TypedExprKind::StrLit(value) => Pattern::Literal(LiteralPat::Str(value.clone())),
        TypedExprKind::NoneLit => Pattern::Literal(LiteralPat::None),
        TypedExprKind::Field(target, variant) => {
            if let TypedExprKind::Ident(enum_name) = &target.kind {
                Pattern::EnumVariant(enum_name.clone(), variant.clone())
            } else {
                Pattern::Wildcard
            }
        }
        TypedExprKind::Tuple(items) => {
            Pattern::Tuple(items.iter().map(extract_pattern).collect::<Vec<_>>())
        }
        TypedExprKind::Ok(_) => Pattern::Named("Ok".to_string()),
        TypedExprKind::Err(_) => Pattern::Named("Err".to_string()),
        TypedExprKind::Call(callee, _) => {
            if let TypedExprKind::Ident(name) = &callee.kind {
                Pattern::Named(name.clone())
            } else {
                Pattern::Wildcard
            }
        }
        _ => Pattern::Wildcard,
    }
}

/// Formats a pattern for diagnostics.
pub fn display_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_string(),
        Pattern::Literal(LiteralPat::Int(value)) => value.to_string(),
        Pattern::Literal(LiteralPat::Float(value)) => value.to_string(),
        Pattern::Literal(LiteralPat::Bool(value)) => value.to_string(),
        Pattern::Literal(LiteralPat::Str(value)) => format!("{value:?}"),
        Pattern::Literal(LiteralPat::None) => "None".to_string(),
        Pattern::EnumVariant(enum_name, variant) => format!("{enum_name}.{variant}"),
        Pattern::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(display_pattern)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Pattern::Named(name) => format!("{name}(...)"),
        Pattern::Or(items) => items
            .iter()
            .map(display_pattern)
            .collect::<Vec<_>>()
            .join(" | "),
    }
}
