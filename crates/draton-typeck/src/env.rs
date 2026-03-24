use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use im::HashMap as ImHashMap;

use crate::typed_ast::Type;

/// A polymorphic type scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    pub quantified: Vec<u32>,
    pub ty: Type,
}

/// A lexical type environment.
#[derive(Debug, Clone)]
pub struct TypeEnv {
    scopes: Vec<ImHashMap<String, Scheme>>,
    fresh_counter: Rc<Cell<u32>>,
}

impl TypeEnv {
    /// Creates a fresh empty type environment.
    pub fn new() -> Self {
        Self::with_counter(Rc::new(Cell::new(0)))
    }

    /// Creates a type environment using a shared fresh-variable counter.
    pub fn with_counter(counter: Rc<Cell<u32>>) -> Self {
        Self {
            scopes: vec![ImHashMap::new()],
            fresh_counter: counter,
        }
    }

    /// Pushes a new lexical scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(ImHashMap::new());
    }

    /// Pops the innermost lexical scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            let _ = self.scopes.pop();
        }
    }

    /// Defines a name in the innermost scope.
    pub fn define(&mut self, name: &str, scheme: Scheme) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), scheme);
        }
    }

    /// Looks up a name by walking scopes from inner to outer.
    pub fn lookup(&self, name: &str) -> Option<&Scheme> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    /// Removes a name from the nearest scope that contains it.
    pub fn remove(&mut self, name: &str) -> Option<Scheme> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                return scope.remove(name);
            }
        }
        None
    }

    /// Instantiates a scheme with fresh type variables.
    pub fn instantiate(&self, scheme: &Scheme) -> Type {
        let mut replacements = HashMap::new();
        for var in &scheme.quantified {
            let fresh = self.fresh_counter.get();
            self.fresh_counter.set(fresh + 1);
            replacements.insert(*var, Type::Var(fresh));
        }
        substitute_type(&scheme.ty, &replacements)
    }

    /// Returns all scopes for free-variable analysis.
    pub(crate) fn scopes(&self) -> &[ImHashMap<String, Scheme>] {
        &self.scopes
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

fn substitute_type(ty: &Type, replacements: &HashMap<u32, Type>) -> Type {
    match ty {
        Type::Var(id) => replacements.get(id).cloned().unwrap_or(Type::Var(*id)),
        Type::Array(inner) => Type::Array(Box::new(substitute_type(inner, replacements))),
        Type::Map(key, value) => Type::Map(
            Box::new(substitute_type(key, replacements)),
            Box::new(substitute_type(value, replacements)),
        ),
        Type::Set(inner) => Type::Set(Box::new(substitute_type(inner, replacements))),
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| substitute_type(item, replacements))
                .collect(),
        ),
        Type::Option(inner) => Type::Option(Box::new(substitute_type(inner, replacements))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute_type(ok, replacements)),
            Box::new(substitute_type(err, replacements)),
        ),
        Type::Chan(inner) => Type::Chan(Box::new(substitute_type(inner, replacements))),
        Type::Fn(params, ret) => Type::Fn(
            params
                .iter()
                .map(|param| substitute_type(param, replacements))
                .collect(),
            Box::new(substitute_type(ret, replacements)),
        ),
        Type::Named(name, args) => Type::Named(
            name.clone(),
            args.iter()
                .map(|arg| substitute_type(arg, replacements))
                .collect(),
        ),
        Type::Row { fields, rest } => Type::Row {
            fields: fields
                .iter()
                .map(|(name, ty)| (name.clone(), substitute_type(ty, replacements)))
                .collect(),
            rest: rest
                .as_ref()
                .map(|rest| Box::new(substitute_type(rest, replacements))),
        },
        Type::Pointer(inner) => Type::Pointer(Box::new(substitute_type(inner, replacements))),
        other => other.clone(),
    }
}
