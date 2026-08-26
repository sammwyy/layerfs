use std::path::PathBuf;

#[derive(Debug)]
pub enum Command {
    Status,
    Inspect { layer: Option<String> },
    Diff { layer: Option<String> },
    Reset { path: PathBuf },
    Verify,
    Rollback { target: String },
    Rebuild { target: String },
    Checkpoint { name: String },
    Install,
    Doctor,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("missing argument: {0}")]
    MissingArgument(&'static str),
}

/// Parses `layerctl <command> [args...]` using a minimal argument parser,
/// per the dependency philosophy of avoiding heavy CLI frameworks.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let command = args.next().ok_or(CliError::MissingArgument("command"))?;

    match command.as_str() {
        "status" => Ok(Command::Status),
        "inspect" => Ok(Command::Inspect { layer: args.next() }),
        "diff" => Ok(Command::Diff { layer: args.next() }),
        "reset" => Ok(Command::Reset {
            path: args.next().ok_or(CliError::MissingArgument("path"))?.into(),
        }),
        "verify" => Ok(Command::Verify),
        "rollback" => Ok(Command::Rollback {
            target: args.next().ok_or(CliError::MissingArgument("target"))?,
        }),
        "rebuild" => Ok(Command::Rebuild {
            target: args.next().ok_or(CliError::MissingArgument("target"))?,
        }),
        "checkpoint" => Ok(Command::Checkpoint {
            name: args.next().ok_or(CliError::MissingArgument("name"))?,
        }),
        "install" => Ok(Command::Install),
        "doctor" => Ok(Command::Doctor),
        other => Err(CliError::UnknownCommand(other.to_string())),
    }
}
