use std::path::PathBuf;

#[derive(Debug)]
pub enum Command {
    Status,
    Inspect { layer: String },
    Diff { layer: String },
    Reset { path: PathBuf },
    Verify,
    Transaction { program: String, args: Vec<String> },
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
/// frameworks. `--store` is only recognized before the command name —
/// everything after it (notably `transaction -- <program> [args...]`) is
/// passed through verbatim rather than re-scanned for flags.
pub fn parse(args: impl Iterator<Item = String>) -> Result<Invocation, CliError> {
    let mut args = args.peekable();
    let mut store = None;

    while let Some(arg) = args.peek() {
        if arg != "--store" {
            break;
        }
        args.next();
        store = Some(PathBuf::from(
            args.next()
                .ok_or(CliError::MissingArgument("--store value"))?,
        ));
    }

    let command_name = args.next().ok_or(CliError::MissingArgument("command"))?;

    let command = match command_name.as_str() {
        "status" => Command::Status,
        "inspect" => Command::Inspect {
            layer: args.next().ok_or(CliError::MissingArgument("layer"))?,
        },
        "diff" => Command::Diff {
            layer: args.next().ok_or(CliError::MissingArgument("layer"))?,
        },
        "reset" => Command::Reset {
            path: args.next().ok_or(CliError::MissingArgument("path"))?.into(),
        },
        "verify" => Command::Verify,
        "transaction" => {
            let mut rest: Vec<String> = args.collect();
            if rest.first().map(String::as_str) == Some("--") {
                rest.remove(0);
            }
            if rest.is_empty() {
                return Err(CliError::MissingArgument("command to run"));
            }
            let program = rest.remove(0);
            Command::Transaction {
                program,
                args: rest,
            }
        }
        "rollback" => Command::Rollback {
            target: args.next().ok_or(CliError::MissingArgument("target"))?,
        },
        "rebuild" => Command::Rebuild {
            target: args.next().ok_or(CliError::MissingArgument("target"))?,
        },
        "checkpoint" => Command::Checkpoint {
            name: args.next().ok_or(CliError::MissingArgument("name"))?,
        },
        "install" => Command::Install,
        "doctor" => Command::Doctor,
        other => return Err(CliError::UnknownCommand(other.to_string())),
    };

    Ok(Invocation { store, command })
}
