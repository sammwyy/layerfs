use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use layerfs_storage::DirectoryBackend;
use layerfs_transaction::Transaction;

use crate::cli::{Command, Invocation};
use crate::store;
use crate::walk::{self, EntryKind};

/// Executes a parsed invocation. `status`, `inspect`, `diff`, `reset`,
/// `verify`, `transaction`, and `boot-register` are implemented against a
/// `DirectoryBackend`-style store; everything requiring a real
/// package-manager adapter (`rollback`, `rebuild`, `install`) is still a
/// stub.
pub fn run(invocation: Invocation) -> Result<(), String> {
    let Invocation { store, command } = invocation;
    let store_root = crate::store::resolve(&store);

    match command {
        Command::Status => status(&store_root),
        Command::Inspect { layer } => inspect(&store_root, &layer),
        Command::Diff { layer } => diff(&store_root, &layer),
        Command::Reset { path } => reset(&store_root, &path),
        Command::Verify => verify_cmd(&store_root),
        Command::Transaction { program, args } => transaction_cmd(&store_root, &program, &args),
        Command::BootRegister {
            name,
            kernel,
            initramfs,
        } => boot_register_cmd(&store_root, &name, &kernel, &initramfs),
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

    let boot = layerfs_storage::boot::discover(&boot_store(store_root));
    println!("boot:");
    print_optional("  base:", &boot.base);
    print_optional("  update:", &boot.update);
    print_optional("  head:", &boot.head);

    Ok(())
}

fn boot_store(store_root: &Path) -> PathBuf {
    store_root.join("boot")
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
    let report = layerfs_storage::validate::verify_root(&discovered.base);

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

/// Development-only system transaction: stages UPDATE.next/HEAD.next,
/// chroots `program` into the assembled `HEAD.next > UPDATE.next > BASE`
/// view, validates, and commits on success. A real package-manager
/// adapter (dnf/apt/pacman) will drive this same engine; this command
/// exists to exercise it before those adapters are written (section 25).
fn transaction_cmd(store_root: &Path, program: &str, args: &[String]) -> Result<(), String> {
    let backend = DirectoryBackend::new(store_root);
    let mut txn = Transaction::begin(store_root, &backend, transaction_id(), "layerctl")
        .map_err(|e| e.to_string())?;

    let target = store_root.join("transaction-root");
    txn.stage(&target).map_err(|e| e.to_string())?;

    println!("transaction: running {program} in {}", target.display());
    let status = txn.execute(program, args).map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!(
            "{program} exited with {status}; transaction discarded"
        ));
    }

    txn.validate().map_err(|e| e.to_string())?;
    txn.commit().map_err(|e| e.to_string())?;

    println!("transaction committed");
    Ok(())
}

/// Registers a kernel/initramfs pair as the `name` boot generation
/// (`base`/`update`/`head`), atomically activating it. A real kernel
/// package upgrade inside a system transaction should call the same
/// primitive to keep GRUB pointed at a kernel that matches the rootfs.
fn boot_register_cmd(
    store_root: &Path,
    name: &str,
    kernel: &Path,
    initramfs: &Path,
) -> Result<(), String> {
    let dest = layerfs_storage::boot::register(&boot_store(store_root), name, kernel, initramfs)
        .map_err(|e| e.to_string())?;
    println!("registered {name} -> {}", dest.display());
    Ok(())
}

fn transaction_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("txn-{nanos}")
}
