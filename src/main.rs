mod cli;
mod config;
mod console_ui;
mod git;
mod interactive;
mod metadata;
mod platforms;
mod skills;
mod templates;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    // Windows: 启用 ANSI 颜色支持
    #[cfg(windows)]
    let _ = console::Term::stdout();

    cli::Cli::parse().run()
}
