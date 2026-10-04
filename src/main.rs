use clap::{Parser, error::ErrorKind};

use ttype_core::cli::Cli;

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = match error.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
                _ => 1,
            };
            let _ = error.print();
            std::process::exit(code);
        }
    };
    if let Err(error) = ttype_core::app::run(cli) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
