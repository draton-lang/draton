use anyhow::Result;

pub(crate) fn run() -> Result<()> {
    draton_lsp::run_stdio()
}
