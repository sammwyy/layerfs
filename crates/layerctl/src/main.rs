mod cli;
mod commands;
mod doctor;
mod store;
mod walk;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = std::env::args().skip(1);

    let invocation = match cli::parse(args) {
        Ok(invocation) => invocation,
        Err(e) => {
            eprintln!("layerctl: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = commands::run(invocation) {
        eprintln!("layerctl: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
