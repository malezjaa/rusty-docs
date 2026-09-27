mod app;
mod cli;
mod config;
mod formatting;
mod metadata;
mod rustdoc;
mod site;
mod terminal;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(error) = app::run(cli) {
        terminal::print_error(&format!("{error:#}"));
        std::process::exit(1);
    }
}
