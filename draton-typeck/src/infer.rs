use std::collections::BTreeSet;

use crate::env::{Scheme, TypeEnv};
use crate::typed_ast::Type;

/// Returns the free type variables inside a type.
pub(crate) fn free_type_vars(ty: &Type) -> BTreeSet<u32> {
    match ty {
        Type::Var(id) => BTreeSet::from([*id]),
        Type::Array(inner)
        | Type::Set(inner)
        | Type::Option(inner)
        | Type::Chan(inner)
        | Type::Pointer(inner) => free_type_vars(inner),
        Type::Map(key, value) | Type::Result(key, value) => {
            let mut vars = free_type_vars(key);
            vars.extend(free_type_vars(value));
            vars
        }
        Type::Tuple(items) => items.iter().fold(BTreeSet::new(), |mut acc, item| {
            acc.extend(free_type_vars(item));
            acc
        }),
        Type::Fn(params, ret) => {
            let mut vars = params.iter().fold(BTreeSet::new(), |mut acc, item| {
                acc.extend(free_type_vars(item));
                acc
            });
            vars.extend(free_type_vars(ret));
            vars
        }
        Type::Named(_, args) => args.iter().fold(BTreeSet::new(), |mut acc, item| {
            acc.extend(free_type_vars(item));
            acc
        }),
        _ => BTreeSet::new(),
    }
}

/// Returns the free type variables inside a scheme.
pub(crate) fn free_type_vars_in_scheme(scheme: &Scheme) -> BTreeSet<u32> {
    let mut vars = free_type_vars(&scheme.ty);
    for var in &scheme.quantified {
        vars.remove(var);
    }
    vars
}

/// Returns the free type variables inside the full environment.
pub(crate) fn free_type_vars_in_env(env: &TypeEnv) -> BTreeSet<u32> {
    env.scopes().iter().fold(BTreeSet::new(), |mut acc, scope| {
        for scheme in scope.values() {
            acc.extend(free_type_vars_in_scheme(scheme));
        }
        acc
    })
}
