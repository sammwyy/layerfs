use std::path::PathBuf;

use crate::cli::{Command, Invocation};
use crate::walk::{self, EntryKind};
use crate::{store, verify};

/// Executes a parsed invocation. `status`, `inspect`, `diff`, `reset`, and
/// `verify` are implemented against a `DirectoryBackend`-style store;
/// everything requiring the transaction engine or a package-manager
/// adapter (`rollback`, `rebuild`, `install`) is still a stub.
pub fn run(invocation: Invocation) -> Result<(), String> {
    let Invocation { store, command } = invocation;
    let store_root = crate::store::resolve(&store);

    match command {
        Command::Status => status(&store_root),
        Command::Inspect { layer } => inspect(&store_root, &layer),
        Command::Diff { layer } => diff(&store_root, &layer),
        Command::Reset { path } => reset(&store_root, &path),
        Command::Verify => verify_cmd(&store_root),
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

fn status(store_root: &std::path::Path) -> Result<(), String> {
    let discovered = store::discover_layers(store_root)?;

    println!("store: {}", store_root.display());
    println!("  {:<14}{}", "base:", discovered.base.display());
    print_optional("update:", &discovered.update);
    print_optional("update-head:", &discovered.update_head);
    print_optional("override:", &discovered.r#override);
    print_optional("data:", &discovered.data);

    Ok(())
}

fn print_optional(label: &str, path: &Option<PathBuf>) {
    match path {
        Some(p) => println!("  {label:<14}{}", p.display()),
        None => println!("  {label:<14}(absent)"),
    }
}

fn inspect(store_root: &std::path::Path, layer: &str) -> Result<(), String> {
    let discovered = store::discover_layers(store_root)?;
    let path = store::layer_path(layer, &discovered)?;
    let entries = walk::walk(&path).map_err(|e| e.to_string())?;

    if entries.is_empty() {
        println!("{layer}: empty");
        return Ok(());
    }

    for entry in entries {
        let marker = match entry.kind {
            EntryKind::Whiteout => "whiteout",
            EntryKind::Present => "present ",
        };
        println!("{marker}  /{}", entry.path.display());
    }

    Ok(())
}

fn diff(store_root: &std::path::Path, layer: &str) -> Result<(), String> {
    let discovered = store::discover_layers(store_root)?;
    let path = store::layer_path(layer, &discovered)?;
    let below = store::layer_below(layer, &discovered);
    let entries = walk::walk(&path).map_err(|e| e.to_string())?;

    if entries.is_empty() {
        println!("{layer}: no changes");
        return Ok(());
    }

    for entry in entries {
        let status = match entry.kind {
            EntryKind::Whiteout => "removed ",
            EntryKind::Present => match &below {
                Some(below) if below.join(&entry.path).exists() => "modified",
                _ => "added   ",
            },
        };
        println!("{status}  /{}", entry.path.display());
    }

    Ok(())
}

fn reset(store_root: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    let discovered = store::discover_layers(store_root)?;
    let override_dir = discovered.r#override.ok_or("no override layer present")?;

    let relative = path.strip_prefix("/").unwrap_or(path);
    let target = override_dir.join(relative);

    if !target.exists() {
        return Err(format!("{} is not overridden", path.display()));
    }

    if target.is_dir() {
        std::fs::remove_dir_all(&target)
    } else {
        std::fs::remove_file(&target)
    }
    .map_err(|e| e.to_string())?;

    println!("reset /{}", relative.display());
    Ok(())
}

fn verify_cmd(store_root: &std::path::Path) -> Result<(), String> {
    let discovered = store::discover_layers(store_root)?;
    let report = verify::verify_base(&discovered.base);

    for check in &report.checks {
        println!(
            "[{}] {}",
            if check.ok { "ok" } else { "FAIL" },
            check.description
        );
    }

    if report.passed() {
        Ok(())
    } else {
        Err("verification failed".to_string())
    }
}
