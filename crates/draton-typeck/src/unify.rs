use crate::typed_ast::Type;

/// Returns true if the type contains the given variable.
pub(crate) fn occurs(var: u32, ty: &Type) -> bool {
    match ty {
        Type::Var(id) => *id == var,
        Type::Array(inner)
        | Type::Set(inner)
        | Type::Option(inner)
        | Type::Chan(inner)
        | Type::Pointer(inner) => occurs(var, inner),
        Type::Map(key, value) | Type::Result(key, value) => occurs(var, key) || occurs(var, value),
        Type::Tuple(items) => items.iter().any(|item| occurs(var, item)),
        Type::Fn(params, ret) => params.iter().any(|param| occurs(var, param)) || occurs(var, ret),
        Type::Row { fields, rest } => {
            fields.values().any(|field_ty| occurs(var, field_ty))
                || rest.as_ref().is_some_and(|rest| occurs(var, rest))
        }
        Type::Named(_, args) => args.iter().any(|arg| occurs(var, arg)),
        _ => false,
    }
}
