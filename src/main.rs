mod gui;

use anyhow::Result;
use clap::Parser as _;
use logos_inspector::cli::{Args, Mode};
use std::ffi::OsString;

fn main() -> Result<()> {
    let args = Args::parse_from(
        std::iter::once(OsString::from("logos-inspector")).chain(std::env::args_os().skip(1)),
    );
    match args.mode.unwrap_or(Mode::Gui) {
        Mode::Gui => gui::run(),
        Mode::Cli(args) => logos_inspector::cli::run(*args),
    }
}
