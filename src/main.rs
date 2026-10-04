use clap::Parser;

use ttype_core::cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(error) = ttype_core::app::run(cli) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
