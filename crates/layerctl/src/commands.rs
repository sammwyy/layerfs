use crate::cli::Command;

/// Executes a parsed command. All handlers are stubs pending the storage
/// discovery and transaction engine milestones; each prints its intended
/// action rather than silently succeeding.
pub fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Status => todo("status"),
        Command::Inspect { layer } => todo(&format!("inspect {layer:?}")),
        Command::Diff { layer } => todo(&format!("diff {layer:?}")),
        Command::Reset { path } => todo(&format!("reset {}", path.display())),
        Command::Verify => todo("verify"),
        Command::Rollback { target } => todo(&format!("rollback {target}")),
        Command::Rebuild { target } => todo(&format!("rebuild {target}")),
        Command::Checkpoint { name } => todo(&format!("checkpoint {name}")),
        Command::Install => todo("install"),
        Command::Doctor => todo("doctor"),
    }
}

fn todo(action: &str) -> Result<(), String> {
    Err(format!("layerctl {action}: not implemented yet"))
}
