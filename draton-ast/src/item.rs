use crate::stmt::Block;
use crate::types::TypeExpr;
use crate::Span;

/// A parsed Draton program.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A function definition.
    Fn(FnDef),
    /// A class definition.
    Class(ClassDef),
    /// An interface definition.
    Interface(InterfaceDef),
    /// An enum definition.
    Enum(EnumDef),
    /// An error definition.
    Error(ErrorDef),
    /// A const definition.
    Const(ConstDef),
    /// An import declaration.
    Import(ImportDef),
    /// An extern block.
    Extern(ExternBlock),
    /// A `@type` block.
    TypeBlock(TypeBlock),
    /// A `@panic_handler` function.
    PanicHandler(FnDef),
    /// A `@oom_handler` function.
    OomHandler(FnDef),
}

/// A function definition or declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDef {
    pub is_pub: bool,
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub ret_type: Option<TypeExpr>,
    pub body: Option<Block>,
    pub span: Span,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_hint: Option<TypeExpr>,
    pub span: Span,
}

/// A class definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: String,
    pub type_params: Vec<String>,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub members: Vec<ClassMember>,
    pub type_blocks: Vec<TypeBlock>,
    pub span: Span,
}

/// A class member.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    /// A field definition.
    Field(FieldDef),
    /// A method definition.
    Method(FnDef),
    /// A named group of class methods.
    Layer(LayerDef),
}

/// A named group of methods within a class.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerDef {
    pub name: String,
    pub methods: Vec<FnDef>,
    pub type_blocks: Vec<TypeBlock>,
    pub span: Span,
}

/// A class field definition.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub is_mut: bool,
    pub name: String,
    pub type_hint: Option<TypeExpr>,
    pub span: Span,
}

/// An interface definition.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDef {
    pub name: String,
    pub methods: Vec<FnDef>,
    pub span: Span,
}

/// An enum definition.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<String>,
    pub span: Span,
}

/// An error definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorDef {
    pub name: String,
    pub fields: Vec<Param>,
    pub span: Span,
}

/// A const definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDef {
    pub name: String,
    pub value: crate::expr::Expr,
    pub span: Span,
}

/// An import declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDef {
    pub module: Vec<String>,
    pub items: Vec<ImportItem>,
    pub span: Span,
}

/// A single import target.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportItem {
    pub name: String,
    pub alias: Option<String>,
    pub span: Span,
}

/// An extern block.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternBlock {
    pub abi: String,
    pub functions: Vec<FnDef>,
    pub span: Span,
}

/// A `@type` block.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeBlock {
    pub members: Vec<TypeMember>,
    pub span: Span,
}

/// A single member in a `@type` block.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeMember {
    /// A binding type annotation.
    Binding {
        name: String,
        type_expr: TypeExpr,
        span: Span,
    },
    /// A function signature annotation.
    Function(FnDef),
}
