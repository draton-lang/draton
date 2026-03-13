use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use colored::Colorize;
use draton_ast::Program;
use draton_codegen::{BuildMode, CodeGen};
use draton_lexer::{LexError, Lexer};
use draton_parser::{ParseError, Parser};
use draton_typeck::{Type, TypeChecker, TypeError, TypedItem, TypedProgram};
use inkwell::context::Context as LlvmContext;
use inkwell::AddressSpace;

use crate::config::DratonConfig;

const HOST_TARGET: &str = if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
    "x86_64-linux"
} else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
    "x86_64-macos"
} else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
    "arm64-macos"
} else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
    "x86_64-windows"
} else {
    "unknown"
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Profile {
    Debug,
    Release,
    Size,
    Fast,
}

#[derive(Debug, Clone)]
pub(crate) struct BuildRequest {
    pub profile: Profile,
    pub target: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BuildOutput {
    pub binary_path: PathBuf,
    pub object_path: PathBuf,
    pub ir_path: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledProject {
    typed_program: TypedProgram,
    main_return_type: Option<Type>,
}

pub(crate) fn run(project_root: &Path, request: &BuildRequest) -> Result<BuildOutput> {
    let config = DratonConfig::load(project_root)?;
    let resolved_target = request
        .target
        .clone()
        .or_else(|| config.default_target().map(ToOwned::to_owned));
    if let Some(target) = resolved_target.as_deref() {
        ensure_supported_target(target)?;
    }

    let entry_path = config.entry_path(project_root);
    let compiled = compile_project(&entry_path)?;
    let Some(main_return_type) = compiled.main_return_type.clone() else {
        bail!("khong tim thay fn main trong {}", entry_path.display());
    };
    let context = LlvmContext::create();
    let module = CodeGen::new(&context, request.profile.to_codegen_mode())
        .emit(&compiled.typed_program)
        .map_err(|error| anyhow!(error.to_string()))?;
    wrap_main_for_binary(&context, &module, &main_return_type)?;

    let build_dir = project_root
        .join("build")
        .join(request.profile.as_dir_name());
    fs::create_dir_all(&build_dir)
        .with_context(|| format!("khong the tao {}", build_dir.display()))?;
    let exe_name = if cfg!(windows) {
        format!("{}.exe", config.project.name)
    } else {
        config.project.name.clone()
    };
    let ir_path = build_dir.join(format!("{}.ll", config.project.name));
    let object_path = build_dir.join(format!("{}.o", config.project.name));
    let binary_path = build_dir.join(exe_name);
    CodeGen::write_ir(&module, &ir_path)?;
    CodeGen::write_object(&module, &object_path)?;
    link_binary(
        request.profile,
        &object_path,
        &binary_path,
        resolved_target.as_deref(),
    )?;

    Ok(BuildOutput {
        binary_path,
        object_path,
        ir_path,
    })
}

pub(crate) fn compile_project(entry_path: &Path) -> Result<CompiledProject> {
    let program = load_project_program(entry_path)?;
    let typed = TypeChecker::new().check(program);
    if !typed.errors.is_empty() {
        bail!(
            "{}",
            render_type_errors_without_source(entry_path, &typed.errors)
        );
    }
    if !typed.warnings.is_empty() {
        eprintln!(
            "{}",
            render_type_warnings_without_source(entry_path, &typed.warnings)
        );
    }
    let main_return_type = typed
        .typed_program
        .items
        .iter()
        .find_map(|item| match item {
            TypedItem::Fn(function) if function.name == "main" => Some(function.ret_type.clone()),
            _ => None,
        });
    Ok(CompiledProject {
        typed_program: typed.typed_program,
        main_return_type,
    })
}

fn render_type_errors_without_source(path: &Path, errors: &[TypeError]) -> String {
    errors
        .iter()
        .map(|error| format!("type error in {}:\n{}", path.display(), error))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_type_warnings_without_source(path: &Path, warnings: &[TypeError]) -> String {
    warnings
        .iter()
        .map(|warning| format!("type warning in {}:\n{}", path.display(), warning))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn load_project_program(entry_path: &Path) -> Result<Program> {
    let files = collect_project_sources(entry_path)?;
    let mut items = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .with_context(|| format!("khong the doc {}", path.display()))?;
        let lexed = Lexer::new(&source).tokenize();
        if !lexed.errors.is_empty() {
            bail!("{}", render_lex_errors(&path, &source, &lexed.errors));
        }
        let parsed = Parser::new(lexed.tokens).parse();
        if !parsed.errors.is_empty() {
            bail!("{}", render_parse_errors(&path, &source, &parsed.errors));
        }
        items.extend(parsed.program.items);
    }
    Ok(Program { items })
}

fn collect_project_sources(entry_path: &Path) -> Result<Vec<PathBuf>> {
    let src_root = entry_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut files = Vec::new();
    collect_dt_files_recursive(&src_root, &mut files)?;
    files.sort();
    if files.is_empty() {
        files.push(entry_path.to_path_buf());
    }
    Ok(files)
}

fn collect_dt_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("khong the doc {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_dt_files_recursive(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("dt") {
            out.push(path);
        }
    }
    Ok(())
}

pub(crate) fn compile_snippet(source: &str) -> Result<CompiledProject> {
    let synthetic = PathBuf::from("<repl>");
    let lexed = Lexer::new(source).tokenize();
    if !lexed.errors.is_empty() {
        bail!("{}", render_lex_errors(&synthetic, source, &lexed.errors));
    }
    let parsed = Parser::new(lexed.tokens).parse();
    if !parsed.errors.is_empty() {
        bail!(
            "{}",
            render_parse_errors(&synthetic, source, &parsed.errors)
        );
    }
    let typed = TypeChecker::new().check(parsed.program);
    if !typed.errors.is_empty() {
        bail!("{}", render_type_errors(&synthetic, source, &typed.errors));
    }
    if !typed.warnings.is_empty() {
        eprintln!(
            "{}",
            render_type_warnings(&synthetic, source, &typed.warnings)
        );
    }
    let main_return_type = typed
        .typed_program
        .items
        .iter()
        .find_map(|item| match item {
            TypedItem::Fn(function) if function.name == "main" => Some(function.ret_type.clone()),
            _ => None,
        });
    Ok(CompiledProject {
        typed_program: typed.typed_program,
        main_return_type,
    })
}

pub(crate) fn build_module<'ctx>(
    context: &'ctx LlvmContext,
    compiled: &CompiledProject,
    profile: Profile,
) -> Result<inkwell::module::Module<'ctx>> {
    CodeGen::new(context, profile.to_codegen_mode())
        .emit(&compiled.typed_program)
        .map_err(|error| anyhow!(error.to_string()))
}

pub(crate) fn main_return_type(compiled: &CompiledProject) -> Option<Type> {
    compiled.main_return_type.clone()
}

fn ensure_supported_target(target: &str) -> Result<()> {
    if target == HOST_TARGET {
        return Ok(());
    }
    bail!(
        "cross-compile hien chua duoc backend hien tai ho tro: target {}, host {}",
        target,
        HOST_TARGET
    )
}

fn wrap_main_for_binary<'ctx>(
    context: &'ctx LlvmContext,
    module: &inkwell::module::Module<'ctx>,
    main_return_type: &Type,
) -> Result<()> {
    let Some(user_main) = module.get_function("main") else {
        return Ok(());
    };
    user_main.as_global_value().set_name("draton_user_main");
    let i8_ptr = context.i8_type().ptr_type(AddressSpace::default());
    let argv_ty = i8_ptr.ptr_type(AddressSpace::default());
    let wrapper = module.add_function(
        "main",
        context
            .i64_type()
            .fn_type(&[context.i32_type().into(), argv_ty.into()], false),
        None,
    );
    let entry = context.append_basic_block(wrapper, "entry");
    let builder = context.create_builder();
    builder.position_at_end(entry);
    if let Some(set_cli_args) = module.get_function("draton_set_cli_args") {
        let argc = wrapper
            .get_nth_param(0)
            .ok_or_else(|| anyhow!("missing wrapper argc"))?;
        let argv = wrapper
            .get_nth_param(1)
            .ok_or_else(|| anyhow!("missing wrapper argv"))?;
        let _ = builder
            .build_call(
                set_cli_args,
                &[argc.into(), argv.into()],
                "drat.set_cli_args",
            )
            .map_err(|error| anyhow!(error.to_string()))?;
    }
    let call = builder
        .build_call(user_main, &[], "drat.main")
        .map_err(|error| anyhow!(error.to_string()))?;
    let exit_code = match main_return_type {
        Type::Int
        | Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64
        | Type::Bool => {
            let value = call
                .try_as_basic_value()
                .left()
                .map(|value| value.into_int_value())
                .unwrap_or_else(|| context.i64_type().const_zero());
            builder
                .build_int_z_extend_or_bit_cast(value, context.i64_type(), "drat.exit")
                .map_err(|error| anyhow!(error.to_string()))?
        }
        _ => context.i64_type().const_zero(),
    };
    builder
        .build_return(Some(&exit_code))
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(())
}

fn link_binary(
    profile: Profile,
    object_path: &Path,
    binary_path: &Path,
    target: Option<&str>,
) -> Result<()> {
    if let Some(target) = target {
        ensure_supported_target(target)?;
    }
    let mut command = Command::new("cc");
    command.arg(object_path);
    if env::var_os("DRATON_SKIP_RUNTIME_LINK").is_none() {
        let runtime_lib = ensure_runtime_staticlib(profile)?;
        command.arg(&runtime_lib);
    }
    command.arg("-o").arg(binary_path);
    if cfg!(target_os = "linux") && env::var_os("DRATON_ALLOW_MULTIPLE_RUNTIME_DEFS").is_some() {
        command.arg("-Wl,--allow-multiple-definition");
    }
    if cfg!(target_os = "linux") {
        command.args(["-no-pie", "-ldl", "-lpthread", "-lm", "-lrt", "-lutil"]);
    } else if cfg!(target_os = "macos") {
        command.args(["-ldl", "-lpthread", "-lm"]);
    }
    let output = command
        .output()
        .with_context(|| "khong the chay linker cc".to_string())?;
    if !output.status.success() {
        bail!(
            "link that bai:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn ensure_runtime_staticlib(profile: Profile) -> Result<PathBuf> {
    let workspace_root = workspace_root();
    let manifest = workspace_root.join("draton-runtime/Cargo.toml");
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("-p")
        .arg("draton-runtime")
        .arg("--manifest-path")
        .arg(manifest);
    if matches!(profile, Profile::Release | Profile::Size | Profile::Fast) {
        command.arg("--release");
    }
    if let Ok(target_dir) = env::var("CARGO_TARGET_DIR") {
        command.env("CARGO_TARGET_DIR", target_dir);
    }
    let output = command
        .output()
        .with_context(|| "khong the build draton-runtime".to_string())?;
    if !output.status.success() {
        bail!(
            "build draton-runtime that bai:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut path = target_dir();
    if matches!(profile, Profile::Release | Profile::Size | Profile::Fast) {
        path = path.join("release");
    } else {
        path = path.join("debug");
    }
    let filename = if cfg!(windows) {
        "draton_runtime.lib"
    } else {
        "libdraton_runtime.a"
    };
    let path = path.join(filename);
    if !path.exists() {
        bail!("khong tim thay runtime staticlib tai {}", path.display());
    }
    Ok(path)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn target_dir() -> PathBuf {
    env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().join("target"))
}

impl Profile {
    pub(crate) fn from_flags(release: bool, size: bool, fast: bool) -> Result<Self> {
        let selected = [release, size, fast]
            .into_iter()
            .filter(|flag| *flag)
            .count();
        if selected > 1 {
            bail!("chi duoc chon mot trong --release, --size, --fast");
        }
        Ok(if release {
            Self::Release
        } else if size {
            Self::Size
        } else if fast {
            Self::Fast
        } else {
            Self::Debug
        })
    }

    fn to_codegen_mode(self) -> BuildMode {
        match self {
            Self::Debug => BuildMode::Debug,
            Self::Release => BuildMode::Release,
            Self::Size => BuildMode::Size,
            Self::Fast => BuildMode::Fast,
        }
    }

    fn as_dir_name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
            Self::Size => "size",
            Self::Fast => "fast",
        }
    }
}

fn render_lex_errors(path: &Path, source: &str, errors: &[LexError]) -> String {
    errors
        .iter()
        .map(|error| match error {
            LexError::UnexpectedChar { found, line, col } => render_diagnostic(
                "E100",
                "lexer error",
                path,
                source,
                *line,
                *col,
                vec![format!("found:    {found}")],
                Some("xoa ky tu nay hoac thay bang token hop le".to_string()),
            ),
            LexError::UnterminatedString { line, col } => render_diagnostic(
                "E101",
                "unterminated string",
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("dong dau nhay kep truoc khi xuong dong".to_string()),
            ),
            LexError::UnterminatedBlockComment { line, col } => render_diagnostic(
                "E102",
                "unterminated block comment",
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("them */ de ket thuc block comment".to_string()),
            ),
            LexError::InvalidNumericLiteral { lexeme, line, col } => render_diagnostic(
                "E103",
                "invalid numeric literal",
                path,
                source,
                *line,
                *col,
                vec![format!("found:    {lexeme}")],
                Some("kiem tra prefix va ky tu cua literal so".to_string()),
            ),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_parse_errors(path: &Path, source: &str, errors: &[ParseError]) -> String {
    errors
        .iter()
        .map(|error| match error {
            ParseError::UnexpectedToken {
                found,
                expected,
                line,
                col,
            } => render_diagnostic(
                "E200",
                "unexpected token",
                path,
                source,
                *line,
                *col,
                vec![
                    format!("expected: {expected}"),
                    format!("found:    {found}"),
                ],
                None,
            ),
            ParseError::UnexpectedEof {
                expected,
                line,
                col,
            } => render_diagnostic(
                "E201",
                "unexpected eof",
                path,
                source,
                *line,
                *col,
                vec![format!("expected: {expected}")],
                Some("bo sung phan con thieu o cuoi file".to_string()),
            ),
            ParseError::InvalidExpr { line, col } => render_diagnostic(
                "E202",
                "invalid expression",
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("kiem tra lai cu phap bieu thuc".to_string()),
            ),
            ParseError::NestedLayerNotAllowed { line, col } => render_diagnostic(
                "E010",
                "nested layer not allowed",
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("layers khong duoc long ben trong layer khac".to_string()),
            ),
            ParseError::LayerOutsideClass { line, col } => render_diagnostic(
                "E011",
                "layer outside class",
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("layer chi duoc khai bao ben trong than class".to_string()),
            ),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_type_errors(path: &Path, source: &str, errors: &[TypeError]) -> String {
    errors
        .iter()
        .map(|error| match error {
            TypeError::Mismatch {
                expected,
                found,
                hint,
                line,
                col,
            } => render_diagnostic(
                "E001",
                "type mismatch",
                path,
                source,
                *line,
                *col,
                vec![
                    format!("expected: {expected}"),
                    format!("found:    {found}"),
                ],
                Some(format!("hint:     {hint}")),
            ),
            TypeError::UndefinedVar { name, line, col } => render_diagnostic(
                "E002",
                &format!("undefined variable '{name}'"),
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some(format!("hint:     khai bao bien bang 'let {name} = ...'")),
            ),
            TypeError::UndefinedFn { name, line, col } => render_diagnostic(
                "E003",
                &format!("undefined function '{name}'"),
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("hint:     kiem tra import hoac ten ham".to_string()),
            ),
            TypeError::NoField {
                field,
                ty,
                line,
                col,
            } => render_diagnostic(
                "E004",
                &format!("field '{field}' not found"),
                path,
                source,
                *line,
                *col,
                vec![format!("found:    {ty}")],
                Some("hint:     kiem tra field hoac type cua doi tuong".to_string()),
            ),
            TypeError::BadBinOp {
                op,
                lhs,
                rhs,
                line,
                col,
            } => render_diagnostic(
                "E005",
                &format!("cannot apply '{op}'"),
                path,
                source,
                *line,
                *col,
                vec![format!("found:    {lhs} va {rhs}")],
                Some("hint:     dong nhat kieu hai ve toan hang".to_string()),
            ),
            TypeError::ArgCount {
                expected,
                got,
                line,
                col,
            } => render_diagnostic(
                "E006",
                "wrong number of arguments",
                path,
                source,
                *line,
                *col,
                vec![format!("expected: {expected}"), format!("found:    {got}")],
                None,
            ),
            TypeError::DestructureArity {
                pattern_len,
                tuple_len,
                line,
                col,
            } => render_diagnostic(
                "E015",
                "tuple destructure arity mismatch",
                path,
                source,
                *line,
                *col,
                vec![
                    format!("pattern:  {pattern_len} binding(s)"),
                    format!("tuple:    {tuple_len} slot(s)"),
                ],
                Some("hint:     sua so luong binding cho khop tuple".to_string()),
            ),
            TypeError::CannotInfer { name, line, col } => render_diagnostic(
                "E007",
                &format!("cannot infer type for '{name}'"),
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("hint:     them ngữ cảnh sử dụng ro hon".to_string()),
            ),
            TypeError::InfiniteType { var, line, col } => render_diagnostic(
                "E008",
                "infinite type detected",
                path,
                source,
                *line,
                *col,
                vec![format!("found:    {var}")],
                Some("hint:     loai bo tu tham chieu de quy vao chinh no".to_string()),
            ),
            TypeError::BadCast {
                from,
                to,
                line,
                col,
            } => render_diagnostic(
                "E009",
                "invalid cast",
                path,
                source,
                *line,
                *col,
                vec![format!("expected: {to}"), format!("found:    {from}")],
                Some("hint:     chi cast giua cac kieu backend ho tro".to_string()),
            ),
            TypeError::IncompatibleErrors {
                lhs,
                rhs,
                line,
                col,
            } => render_diagnostic(
                "E010",
                "incompatible error propagation",
                path,
                source,
                *line,
                *col,
                vec![format!("left:     {lhs}"), format!("right:    {rhs}")],
                Some("hint:     wrap ca hai vao mot error type chung".to_string()),
            ),
            TypeError::MissingInterfaceMethod {
                class,
                interface,
                method,
                line,
                col,
            } => render_diagnostic(
                "E011",
                &format!("class '{class}' does not fully implement interface '{interface}'"),
                path,
                source,
                *line,
                *col,
                vec![format!("missing:  {method}")],
                Some(
                    "hint:     them method con thieu hoac sua lai danh sach implements".to_string(),
                ),
            ),
            TypeError::CircularInheritance { class, line, col } => render_diagnostic(
                "E013",
                &format!("circular inheritance involving '{class}'"),
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("hint:     pha vo vong extends giua cac class".to_string()),
            ),
            TypeError::UndefinedParent {
                class,
                parent,
                line,
                col,
            } => render_diagnostic(
                "E014",
                &format!("undefined parent class '{parent}' for '{class}'"),
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("hint:     khai bao class parent truoc khi extends".to_string()),
            ),
            TypeError::NonExhaustiveMatch { missing, line, col } => render_diagnostic(
                "E012",
                "non-exhaustive match",
                path,
                source,
                *line,
                *col,
                vec![format!("missing:  {missing}")],
                Some(
                    "hint:     them arm `_ => ...` hoac cover day du cac pattern con thieu"
                        .to_string(),
                ),
            ),
            TypeError::RedundantPattern { pattern, line, col } => render_diagnostic(
                "W001",
                &format!("redundant pattern '{pattern}'"),
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("hint:     pattern nay khong bao gio duoc match".to_string()),
            ),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_type_warnings(path: &Path, source: &str, warnings: &[TypeError]) -> String {
    warnings
        .iter()
        .filter_map(|warning| match warning {
            TypeError::RedundantPattern { pattern, line, col } => Some(render_warning_diagnostic(
                "W001",
                &format!("redundant pattern '{pattern}'"),
                path,
                source,
                *line,
                *col,
                Vec::new(),
                Some("hint:     pattern nay khong bao gio duoc match".to_string()),
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[allow(clippy::too_many_arguments)]
fn render_diagnostic(
    code: &str,
    title: &str,
    path: &Path,
    source: &str,
    line: usize,
    col: usize,
    notes: Vec<String>,
    hint: Option<String>,
) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let snippet = lines
        .get(line.saturating_sub(1))
        .copied()
        .unwrap_or_default();
    let marker_width = col.saturating_sub(1);
    let marker = format!("{}^", " ".repeat(marker_width));
    let mut out = String::new();
    out.push_str(&format!(
        "{}[{}] {} — {}:{}:{}\n",
        "error".red().bold(),
        code.red().bold(),
        title,
        path.display(),
        line,
        col
    ));
    out.push_str("  |\n");
    out.push_str(&format!("{line:>2}|   {snippet}\n"));
    out.push_str(&format!("  |   {}\n", marker.red().bold()));
    out.push_str("  |\n");
    for note in notes {
        out.push_str(&format!("  = {note}\n"));
    }
    if let Some(hint) = hint {
        out.push_str(&format!("  = {hint}\n"));
    }
    out.trim_end().to_string()
}

#[allow(clippy::too_many_arguments)]
fn render_warning_diagnostic(
    code: &str,
    title: &str,
    path: &Path,
    source: &str,
    line: usize,
    col: usize,
    notes: Vec<String>,
    hint: Option<String>,
) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let snippet = lines
        .get(line.saturating_sub(1))
        .copied()
        .unwrap_or_default();
    let marker_width = col.saturating_sub(1);
    let marker = format!("{}^", " ".repeat(marker_width));
    let mut out = String::new();
    out.push_str(&format!(
        "{}[{}] {} — {}:{}:{}\n",
        "warning".yellow().bold(),
        code.yellow().bold(),
        title,
        path.display(),
        line,
        col
    ));
    out.push_str("  |\n");
    out.push_str(&format!("{line:>2}|   {snippet}\n"));
    out.push_str(&format!("  |   {}\n", marker.yellow().bold()));
    out.push_str("  |\n");
    for note in notes {
        out.push_str(&format!("  = {note}\n"));
    }
    if let Some(hint) = hint {
        out.push_str(&format!("  = {hint}\n"));
    }
    out.trim_end().to_string()
}
