use clap::Parser;

/// Carries a development ticket from description to merge.
#[derive(Parser)]
#[command(name = "stapel", version)]
struct Cli {}

fn main() {
    Cli::parse();
}
