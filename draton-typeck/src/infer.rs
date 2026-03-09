use std::collections::BTreeSet;

use im::HashMap as ImHashMap;

use crate::env::{Scheme, TypeEnv};
use crate::error::TypeError;
use crate::typed_ast::Type;
use crate::unify::occurs;

/// An immutable substitution used by Algorithm W.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Substitution {
    map: ImHashMap<u32, Type>,
}

impl Substitution {
    /// Creates an empty substitution.
    pub fn empty() -> Self {
        Self {
            map: ImHashMap::new(),
        }
    }

    /// Binds a type variable to a type, returning a new substitution.
    pub fn bind(mut self, var: u32, ty: Type) -> Result<Self, TypeError> {
        let ty = self.apply(ty);
        if ty == Type::Var(var) {
            return Ok(self);
        }
        if occurs(var, &ty) {
            return Err(TypeError::InfiniteType {
                var: format!("t{var}"),
                line: 0,
                col: 0,
            });
        }
        self.map = self
            .map
            .iter()
            .map(|(key, value)| (*key, single_binding(var, ty.clone()).apply(value.clone())))
            .collect();
        self.map.insert(var, ty);
        Ok(self)
    }

    /// Applies the substitution to a type.
    pub fn apply(&self, ty: Type) -> Type {
        match ty {
            Type::Var(id) => self
                .map
                .get(&id)
                .cloned()
                .map(|inner| self.apply(inner))
                .unwrap_or(Type::Var(id)),
            Type::Array(inner) => Type::Array(Box::new(self.apply(*inner))),
            Type::Map(key, value) => {
                Type::Map(Box::new(self.apply(*key)), Box::new(self.apply(*value)))
            }
            Type::Set(inner) => Type::Set(Box::new(self.apply(*inner))),
            Type::Tuple(items) => {
                Type::Tuple(items.into_iter().map(|item| self.apply(item)).collect())
            }
            Type::Option(inner) => Type::Option(Box::new(self.apply(*inner))),
            Type::Result(ok, err) => {
                Type::Result(Box::new(self.apply(*ok)), Box::new(self.apply(*err)))
            }
            Type::Chan(inner) => Type::Chan(Box::new(self.apply(*inner))),
            Type::Fn(params, ret) => Type::Fn(
                params.into_iter().map(|param| self.apply(param)).collect(),
                Box::new(self.apply(*ret)),
            ),
            Type::Named(name, args) => {
                Type::Named(name, args.into_iter().map(|arg| self.apply(arg)).collect())
            }
            Type::Row { fields, rest } => Type::Row {
                fields: fields
                    .into_iter()
                    .map(|(name, ty)| (name, self.apply(ty)))
                    .collect(),
                rest: rest.map(|rest| Box::new(self.apply(*rest))),
            },
            Type::Pointer(inner) => Type::Pointer(Box::new(self.apply(*inner))),
            other => other,
        }
    }

    /// Composes two substitutions: `self ∘ other`.
    pub fn compose(self, other: Substitution) -> Substitution {
        let mut map = other
            .map
            .iter()
            .map(|(key, value)| (*key, self.apply(value.clone())))
            .collect::<ImHashMap<_, _>>();
        for (key, value) in self.map {
            map.insert(key, value);
        }
        Substitution { map }
    }
}

fn single_binding(var: u32, ty: Type) -> Substitution {
    let mut map = ImHashMap::new();
    map.insert(var, ty);
    Substitution { map }
}

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
        Type::Row { fields, rest } => {
            let mut vars = fields.values().fold(BTreeSet::new(), |mut acc, item| {
                acc.extend(free_type_vars(item));
                acc
            });
            if let Some(rest) = rest {
                vars.extend(free_type_vars(rest));
            }
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
