use std::io::{self, Write as _};

use anyhow::Result;
use serde_json::Value;

pub(super) fn print_json(value: &impl serde::Serialize) -> Result<()> {
    print_line(serde_json::to_string_pretty(value)?)
}

pub(super) fn print_text_value(value: &Value) -> Result<()> {
    if let Some(value) = value.as_str() {
        return print_line(value);
    }
    print_line(value)
}

fn print_line(value: impl std::fmt::Display) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{value}")?;
    Ok(())
}
