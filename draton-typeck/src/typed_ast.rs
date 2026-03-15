use std::collections::BTreeMap;
use std::fmt;

use draton_ast::{AssignOp, BinOp, Span, UnOp};

/// A fully typed Draton program.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedProgram {
    pub items: Vec<TypedItem>,
}

/// A typed top-level item.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedItem {
    /// A typed function definition.
    Fn(TypedFnDef),
    /// A typed class definition.
    Class(TypedClassDef),
    /// A typed interface definition.
    Interface(TypedInterfaceDef),
    /// A typed enum definition.
    Enum(TypedEnumDef),
    /// A typed error type definition.
    Error(TypedErrorDef),
    /// A typed const definition.
    Const(TypedConstDef),
    /// A typed import declaration.
    Import(TypedImportDef),
    /// A typed extern block.
    Extern(TypedExternBlock),
    /// A typed `@type` block.
    TypeBlock(TypedTypeBlock),
    /// A typed `@panic_handler` function.
    PanicHandler(TypedFnDef),
    /// A typed `@oom_handler` function.
    OomHandler(TypedFnDef),
}

/// A typed function definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedFnDef {
    pub is_pub: bool,
    pub name: String,
    pub params: Vec<TypedParam>,
    pub ret_type: Type,
    pub body: Option<TypedBlock>,
    pub ty: Type,
    pub span: Span,
}

/// A typed function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedParam {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

/// A typed class definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedClassDef {
    pub name: String,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub fields: Vec<TypedFieldDef>,
    pub methods: Vec<TypedFnDef>,
    pub type_blocks: Vec<TypedTypeBlock>,
    pub span: Span,
}

/// A typed class field.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedFieldDef {
    pub is_mut: bool,
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

/// A typed interface definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedInterfaceDef {
    pub name: String,
    pub methods: Vec<TypedFnDef>,
    pub span: Span,
}

/// A typed enum definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedEnumDef {
    pub name: String,
    pub variants: Vec<String>,
    pub span: Span,
}

/// A typed error definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedErrorDef {
    pub name: String,
    pub fields: Vec<TypedParam>,
    pub span: Span,
}

/// A typed const definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedConstDef {
    pub name: String,
    pub value: TypedExpr,
    pub ty: Type,
    pub span: Span,
}

/// A typed import declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedImportDef {
    pub module: Vec<String>,
    pub items: Vec<TypedImportItem>,
    pub span: Span,
}

/// A typed import item.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedImportItem {
    pub name: String,
    pub alias: Option<String>,
    pub span: Span,
}

/// A typed extern block.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedExternBlock {
    pub abi: String,
    pub functions: Vec<TypedFnDef>,
    pub span: Span,
}

/// A typed `@type` block.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedTypeBlock {
    pub members: Vec<TypedTypeMember>,
    pub span: Span,
}

/// A typed member inside a `@type` block.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedTypeMember {
    /// A typed binding annotation.
    Binding { name: String, ty: Type, span: Span },
    /// A typed function annotation.
    Function(TypedFnDef),
}

/// A typed block of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedBlock {
    pub stmts: Vec<TypedStmt>,
    pub span: Span,
}

/// A typed statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedStmt {
    pub kind: TypedStmtKind,
    pub span: Span,
}

/// The kind of a typed statement.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedStmtKind {
    /// A typed let statement.
    Let(TypedLetStmt),
    /// A typed tuple destructuring let statement.
    LetDestructure(TypedLetDestructureStmt),
    /// A typed assignment statement.
    Assign(TypedAssignStmt),
    /// A typed return statement.
    Return(TypedReturnStmt),
    /// A typed expression statement.
    Expr(TypedExpr),
    /// A typed if statement.
    If(TypedIfStmt),
    /// A typed for statement.
    For(TypedForStmt),
    /// A typed while statement.
    While(TypedWhileStmt),
    /// A typed spawn statement.
    Spawn(TypedSpawnStmt),
    /// A typed nested block.
    Block(TypedBlock),
    /// A typed unsafe block.
    UnsafeBlock(TypedBlock),
    /// A typed pointer block.
    PointerBlock(TypedBlock),
    /// A typed asm block.
    AsmBlock(String),
    /// A typed comptime block.
    ComptimeBlock(TypedBlock),
    /// A typed compile-time if statement.
    IfCompile(TypedIfCompileStmt),
    /// A typed GC config block.
    GcConfig(TypedGcConfigStmt),
}

/// A typed let statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedLetStmt {
    pub is_mut: bool,
    pub name: String,
    pub value: Option<TypedExpr>,
    pub ty: Type,
    pub span: Span,
}

/// A typed tuple destructuring let statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedLetDestructureStmt {
    pub is_mut: bool,
    pub bindings: Vec<TypedDestructureBinding>,
    pub value: TypedExpr,
    pub tuple_ty: Type,
    pub span: Span,
}

/// A single typed tuple destructuring binding.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedDestructureBinding {
    /// Bind the slot to a local variable.
    Name { name: String, ty: Type },
    /// Discard the slot.
    Wildcard,
}

/// A typed assignment statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedAssignStmt {
    pub target: TypedExpr,
    pub op: AssignOp,
    pub value: Option<TypedExpr>,
    pub span: Span,
}

/// A typed return statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedReturnStmt {
    pub value: Option<TypedExpr>,
    pub ty: Type,
    pub span: Span,
}

/// A typed if statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedIfStmt {
    pub condition: TypedExpr,
    pub then_branch: TypedBlock,
    pub else_branch: Option<TypedElseBranch>,
    pub span: Span,
}

/// A typed else branch.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedElseBranch {
    /// A typed else-if branch.
    If(Box<TypedIfStmt>),
    /// A typed else block.
    Block(TypedBlock),
}

/// A typed for statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedForStmt {
    pub name: String,
    pub iter: TypedExpr,
    pub item_type: Type,
    pub body: TypedBlock,
    pub span: Span,
}

/// A typed while statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedWhileStmt {
    pub condition: TypedExpr,
    pub body: TypedBlock,
    pub span: Span,
}

/// A typed spawn statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedSpawnStmt {
    pub body: TypedSpawnBody,
    pub span: Span,
}

/// The body of a typed spawn statement.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedSpawnBody {
    /// A spawned typed expression.
    Expr(TypedExpr),
    /// A spawned typed block.
    Block(TypedBlock),
}

/// A typed compile-time if statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedIfCompileStmt {
    pub condition: TypedExpr,
    pub body: TypedBlock,
    pub span: Span,
}

/// A typed GC config block.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedGcConfigStmt {
    pub entries: Vec<TypedGcConfigEntry>,
    pub span: Span,
}

/// A typed GC config entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedGcConfigEntry {
    pub key: String,
    pub value: TypedExpr,
    pub span: Span,
}

/// A fully typed expression.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: Type,
    pub span: Span,
}

/// The kind of a typed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind {
    /// An integer literal.
    IntLit(i64),
    /// A floating-point literal.
    FloatLit(f64),
    /// A string literal.
    StrLit(String),
    /// An interpolated string literal.
    FStrLit(Vec<TypedFStrPart>),
    /// A boolean literal.
    BoolLit(bool),
    /// The `None` literal.
    NoneLit,
    /// An identifier.
    Ident(String),
    /// An array literal.
    Array(Vec<TypedExpr>),
    /// A map literal.
    Map(Vec<(TypedExpr, TypedExpr)>),
    /// A set literal.
    Set(Vec<TypedExpr>),
    /// A tuple literal.
    Tuple(Vec<TypedExpr>),
    /// A binary operation.
    BinOp(Box<TypedExpr>, BinOp, Box<TypedExpr>),
    /// A unary operation.
    UnOp(UnOp, Box<TypedExpr>),
    /// A call expression.
    Call(Box<TypedExpr>, Vec<TypedExpr>),
    /// A method call expression.
    MethodCall(Box<TypedExpr>, String, Vec<TypedExpr>),
    /// A field access.
    Field(Box<TypedExpr>, String),
    /// An index access.
    Index(Box<TypedExpr>, Box<TypedExpr>),
    /// A lambda expression.
    Lambda(Vec<TypedParam>, Box<TypedExpr>),
    /// A cast expression.
    Cast(Box<TypedExpr>, Type),
    /// A match expression.
    Match(Box<TypedExpr>, Vec<TypedMatchArm>),
    /// An `Ok` constructor expression.
    Ok(Box<TypedExpr>),
    /// An `Err` constructor expression.
    Err(Box<TypedExpr>),
    /// A nullish propagation expression.
    Nullish(Box<TypedExpr>, Box<TypedExpr>),
    /// A `chan[T]` constructor expression.
    Chan(Type),
}

/// A typed part inside an interpolated string.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedFStrPart {
    /// A plain literal fragment.
    Literal(String),
    /// An interpolated expression fragment.
    Interp(TypedExpr),
}

/// A typed match arm.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedMatchArm {
    pub pattern: TypedExpr,
    pub body: TypedMatchArmBody,
    pub span: Span,
}

/// The body of a typed match arm.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedMatchArmBody {
    /// An expression arm body.
    Expr(TypedExpr),
    /// A block arm body.
    Block(TypedBlock),
}

/// A resolved Draton type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// `Int`
    Int,
    /// `Int8`
    Int8,
    /// `Int16`
    Int16,
    /// `Int32`
    Int32,
    /// `Int64`
    Int64,
    /// `UInt8`
    UInt8,
    /// `UInt16`
    UInt16,
    /// `UInt32`
    UInt32,
    /// `UInt64`
    UInt64,
    /// `Float`
    Float,
    /// `Float32`
    Float32,
    /// `Float64`
    Float64,
    /// `Bool`
    Bool,
    /// `String`
    String,
    /// `Char`
    Char,
    /// `Unit`
    Unit,
    /// `Never`
    Never,
    /// `Array[T]`
    Array(Box<Type>),
    /// `Map[K, V]`
    Map(Box<Type>, Box<Type>),
    /// `Set[T]`
    Set(Box<Type>),
    /// `Tuple[...]`
    Tuple(Vec<Type>),
    /// `Option[T]`
    Option(Box<Type>),
    /// `Result[T, E]`
    Result(Box<Type>, Box<Type>),
    /// `Chan[T]`
    Chan(Box<Type>),
    /// `Fn(...) -> R`
    Fn(Vec<Type>, Box<Type>),
    /// User-defined named types.
    Named(String, Vec<Type>),
    /// An open or closed row type used for structural field inference.
    Row {
        fields: BTreeMap<String, Type>,
        rest: Option<Box<Type>>,
    },
    /// A pointer type.
    Pointer(Box<Type>),
    /// An internal unification variable.
    Var(u32),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => write!(f, "Int"),
            Self::Int8 => write!(f, "Int8"),
            Self::Int16 => write!(f, "Int16"),
            Self::Int32 => write!(f, "Int32"),
            Self::Int64 => write!(f, "Int64"),
            Self::UInt8 => write!(f, "UInt8"),
            Self::UInt16 => write!(f, "UInt16"),
            Self::UInt32 => write!(f, "UInt32"),
            Self::UInt64 => write!(f, "UInt64"),
            Self::Float => write!(f, "Float"),
            Self::Float32 => write!(f, "Float32"),
            Self::Float64 => write!(f, "Float64"),
            Self::Bool => write!(f, "Bool"),
            Self::String => write!(f, "String"),
            Self::Char => write!(f, "Char"),
            Self::Unit => write!(f, "Unit"),
            Self::Never => write!(f, "Never"),
            Self::Array(inner) => write!(f, "Array[{inner}]"),
            Self::Map(key, value) => write!(f, "Map[{key}, {value}]"),
            Self::Set(inner) => write!(f, "Set[{inner}]"),
            Self::Tuple(items) => {
                let rendered = items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "Tuple[{rendered}]")
            }
            Self::Option(inner) => write!(f, "Option[{inner}]"),
            Self::Result(ok, err) => write!(f, "Result[{ok}, {err}]"),
            Self::Chan(inner) => write!(f, "Chan[{inner}]"),
            Self::Fn(params, ret) => {
                let rendered = params
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "Fn({rendered}) -> {ret}")
            }
            Self::Named(name, args) => {
                if args.is_empty() {
                    write!(f, "{name}")
                } else {
                    let rendered = args
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    write!(f, "{name}[{rendered}]")
                }
            }
            Self::Row { fields, rest } => {
                let rendered = fields
                    .iter()
                    .map(|(name, ty)| format!("{name}: {ty}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                if let Some(rest) = rest {
                    if rendered.is_empty() {
                        write!(f, "{{ | {rest} }}")
                    } else {
                        write!(f, "{{ {rendered} | {rest} }}")
                    }
                } else {
                    write!(f, "{{ {rendered} }}")
                }
            }
            Self::Pointer(inner) => write!(f, "Pointer[{inner}]"),
            Self::Var(id) => write!(f, "t{id}"),
        }
    }
}
