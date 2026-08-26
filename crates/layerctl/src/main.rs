mod cli;
mod commands;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = std::env::args().skip(1);

    let command = match cli::parse(args) {
        Ok(command) => command,
        Err(e) => {
            eprintln!("layerctl: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = commands::run(command) {
        eprintln!("layerctl: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
