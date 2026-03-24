use draton_ast::Program;

/// Stateful pretty printer for Draton AST nodes.
#[derive(Debug, Default)]
pub(crate) struct Printer {
    output: String,
    indent: usize,
}

impl Printer {
    /// Creates a new printer.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn fmt_program(&mut self, program: &Program) {
        let mut first = true;
        for item in &program.items {
            if !first {
                self.blank_line();
            }
            self.fmt_item(item);
            first = false;
        }
    }

    pub(crate) fn write(&mut self, text: &str) {
        self.output.push_str(text);
    }

    pub(crate) fn newline(&mut self) {
        self.output.push('\n');
    }

    pub(crate) fn blank_line(&mut self) {
        let trimmed = self.output.trim_end_matches('\n');
        self.output.truncate(trimmed.len());
        self.output.push('\n');
        self.output.push('\n');
    }

    pub(crate) fn write_indent(&mut self) {
        self.write(&self.indent_str());
    }

    pub(crate) fn push_indent(&mut self) {
        self.indent += 1;
    }

    pub(crate) fn pop_indent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    pub(crate) fn result(mut self) -> String {
        let trimmed = self.output.trim_end_matches('\n').to_string();
        self.output.clear();
        self.output.push_str(&trimmed);
        self.output.push('\n');
        self.output
    }

    fn indent_str(&self) -> String {
        " ".repeat(self.indent * 4)
    }
}
