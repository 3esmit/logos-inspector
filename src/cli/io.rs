use std::io::{self, Write as _};

use anyhow::Result;

pub(super) fn print_json(value: &impl serde::Serialize) -> Result<()> {
    print_line(serde_json::to_string_pretty(value)?)
}

fn print_line(value: impl std::fmt::Display) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{value}")?;
    Ok(())
}
