use std::path::PathBuf;

#[derive(Debug)]
pub enum Command {
    Status,
    Inspect { layer: String },
    Diff { layer: String },
    Reset { path: PathBuf },
    Verify,
    Rollback { target: String },
    Rebuild { target: String },
    Checkpoint { name: String },
    Install,
    Doctor,
}

#[derive(Debug)]
pub struct Invocation {
    /// Store root, from `--store <path>`. Falls back to a fixed default
    /// when absent; see `store::resolve`.
    pub store: Option<PathBuf>,
    pub command: Command,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("missing argument: {0}")]
    MissingArgument(&'static str),
}

/// Parses `layerctl [--store <path>] <command> [args...]` using a minimal
/// argument parser, per the dependency philosophy of avoiding heavy CLI
/// frameworks. `--store` may appear anywhere before the command's own
/// arguments.
pub fn parse(args: impl Iterator<Item = String>) -> Result<Invocation, CliError> {
    let mut store = None;
    let mut rest = Vec::new();
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        if arg == "--store" {
            store = Some(PathBuf::from(
                args.next()
                    .ok_or(CliError::MissingArgument("--store value"))?,
            ));
        } else {
            rest.push(arg);
        }
    }

    let mut rest = rest.into_iter();
    let command_name = rest.next().ok_or(CliError::MissingArgument("command"))?;

    let command = match command_name.as_str() {
        "status" => Command::Status,
        "inspect" => Command::Inspect {
            layer: rest.next().ok_or(CliError::MissingArgument("layer"))?,
        },
        "diff" => Command::Diff {
            layer: rest.next().ok_or(CliError::MissingArgument("layer"))?,
        },
        "reset" => Command::Reset {
            path: rest.next().ok_or(CliError::MissingArgument("path"))?.into(),
        },
        "verify" => Command::Verify,
        "rollback" => Command::Rollback {
            target: rest.next().ok_or(CliError::MissingArgument("target"))?,
        },
        "rebuild" => Command::Rebuild {
            target: rest.next().ok_or(CliError::MissingArgument("target"))?,
        },
        "checkpoint" => Command::Checkpoint {
            name: rest.next().ok_or(CliError::MissingArgument("name"))?,
        },
        "install" => Command::Install,
        "doctor" => Command::Doctor,
        other => return Err(CliError::UnknownCommand(other.to_string())),
    };

    Ok(Invocation { store, command })
}
