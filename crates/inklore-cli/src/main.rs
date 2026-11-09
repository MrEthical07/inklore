//! The `inklore` command: CLI, and later the `serve` daemon, MCP server and hook commands.

use clap::Parser;

/// Command-line arguments of `inklore`. No subcommands yet.
#[derive(Parser)]
#[command(
    name = "inklore",
    version,
    about = "Inklore: a knowledge engine for AI coding agents."
)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
