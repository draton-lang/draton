use draton_typeck::Type;

/// Renders a type into a stable mangled suffix for monomorphized symbols.
pub fn mangle_type(ty: &Type) -> String {
    match ty {
        Type::Int => "Int".to_string(),
        Type::Int8 => "Int8".to_string(),
        Type::Int16 => "Int16".to_string(),
        Type::Int32 => "Int32".to_string(),
        Type::Int64 => "Int64".to_string(),
        Type::UInt8 => "UInt8".to_string(),
        Type::UInt16 => "UInt16".to_string(),
        Type::UInt32 => "UInt32".to_string(),
        Type::UInt64 => "UInt64".to_string(),
        Type::Float => "Float".to_string(),
        Type::Float32 => "Float32".to_string(),
        Type::Float64 => "Float64".to_string(),
        Type::Bool => "Bool".to_string(),
        Type::String => "String".to_string(),
        Type::Char => "Char".to_string(),
        Type::Unit => "Unit".to_string(),
        Type::Never => "Never".to_string(),
        Type::Array(inner) => format!("Array_{}_", mangle_type(inner)),
        Type::Map(key, value) => format!("Map_{}_{}_", mangle_type(key), mangle_type(value)),
        Type::Set(inner) => format!("Set_{}_", mangle_type(inner)),
        Type::Tuple(items) => {
            let rendered = items.iter().map(mangle_type).collect::<Vec<_>>().join("_");
            format!("Tuple_{rendered}_")
        }
        Type::Option(inner) => format!("Option_{}_", mangle_type(inner)),
        Type::Result(ok, err) => format!("Result_{}_{}_", mangle_type(ok), mangle_type(err)),
        Type::Chan(inner) => format!("Chan_{}_", mangle_type(inner)),
        Type::Fn(params, ret) => {
            let rendered = params.iter().map(mangle_type).collect::<Vec<_>>().join("_");
            format!("Fn_{}_to_{}", rendered, mangle_type(ret))
        }
        Type::Named(name, args) if args.is_empty() => sanitize(name),
        Type::Named(name, args) => {
            let rendered = args.iter().map(mangle_type).collect::<Vec<_>>().join("_");
            format!("{}__{rendered}", sanitize(name))
        }
        Type::Pointer(inner) => format!("Ptr_{}_", mangle_type(inner)),
        Type::Var(id) => format!("T{id}"),
        Type::Row { .. } => "Row".to_string(),
    }
}

/// Returns the mangled LLVM name for a class instantiation.
pub fn mangle_class(class_name: &str, type_args: &[Type]) -> String {
    if type_args.is_empty() {
        return sanitize(class_name);
    }
    let rendered = type_args
        .iter()
        .map(mangle_type)
        .collect::<Vec<_>>()
        .join("_");
    format!("{}__{rendered}", sanitize(class_name))
}

/// Returns the mangled LLVM name for a function instantiation.
pub fn mangle_fn(fn_name: &str, class_name: Option<&str>, type_args: &[Type]) -> String {
    let base = class_name
        .map(|class_name| format!("{}.{}", sanitize(class_name), sanitize(fn_name)))
        .unwrap_or_else(|| sanitize(fn_name));
    if type_args.is_empty() {
        return base;
    }
    let rendered = type_args
        .iter()
        .map(mangle_type)
        .collect::<Vec<_>>()
        .join("_");
    format!("{base}__{rendered}")
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '[' | ']' | ',' | ' ' | ':' | '(' | ')' | '{' | '}' | '-' | '>' | '.' => '_',
            other => other,
        })
        .collect()
}
