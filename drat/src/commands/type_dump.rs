use std::path::Path;

use anyhow::Result;

pub(crate) fn run(path: &Path) -> Result<()> {
    match draton_runtime::host_type_dump_path(path) {
        Ok(dump) => {
            println!("{dump}");
            Ok(())
        }
        Err(message) => {
            if !message.is_empty() {
                println!("{message}");
            }
            Ok(())
        }
    }
}
