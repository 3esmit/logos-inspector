mod cli;
mod gui;

use anyhow::Result;
use clap::Parser as _;

fn main() -> Result<()> {
    let args = cli::Args::parse();
    match args.mode.unwrap_or(cli::Mode::Gui) {
        cli::Mode::Gui => gui::run(),
        cli::Mode::Cli(args) => cli::run(args),
    }
}
