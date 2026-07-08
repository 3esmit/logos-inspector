mod gui;

use anyhow::Result;
use clap::Parser as _;
use logos_inspector::cli::{Args, Mode};

fn main() -> Result<()> {
    let args = Args::parse();
    match args.mode.unwrap_or(Mode::Gui) {
        Mode::Gui => gui::run(),
        Mode::Cli(args) => logos_inspector::cli::run(*args),
    }
}
