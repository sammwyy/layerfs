use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use layerfs_transaction::{Transaction, TransactionLock};

use crate::cli::{Command, Invocation};
use crate::store;
use crate::walk::{self, EntryKind};

/// Executes a parsed invocation; `rebuild` and `checkpoint` are stubs.
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
        Command::Rollback { target } => rollback_cmd(&store_root, &target),
        Command::Rebuild { target } => todo(&format!("rebuild {target}")),
        Command::Checkpoint { name } => todo(&format!("checkpoint {name}")),
        Command::Install {
            source,
            integrations,
            grub_entries,
        } => install_cmd(&store_root, &source, &integrations, grub_entries.as_deref()),
        Command::ApplyNow { live_root } => apply_now_cmd(&store_root, &live_root),
        Command::Doctor => crate::doctor::run(&store_root),
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

/// Development-only system transaction: stages, chroots `program` into the
/// assembled view, validates, and commits on success.
fn transaction_cmd(store_root: &Path, program: &str, args: &[String]) -> Result<(), String> {
    let backend = layerfs_storage::detect_backend(store_root);
    let mut txn = Transaction::begin(store_root, backend.as_ref(), transaction_id(), "layerctl")
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

/// Rolls back to `target` (only `"update"` is valid) by discarding the
/// active UPDATE_HEAD — the one-step rollback the design allows.
fn rollback_cmd(store_root: &Path, target: &str) -> Result<(), String> {
    if target != "update" {
        return Err(format!(
            "unknown rollback target: {target} (only \"update\" is valid)"
        ));
    }

    let _lock = TransactionLock::acquire(&store_root.join("transaction.lock"))
        .map_err(|e| e.to_string())?;

    let discovered = store::discover_layers(store_root)?;
    let head = discovered
        .update_head
        .ok_or("nothing to roll back: no active update-head")?;
    let resolved_head = std::fs::canonicalize(&head).map_err(|e| e.to_string())?;

    std::fs::remove_file(store_root.join("update-head")).map_err(|e| e.to_string())?;

    let backend = layerfs_storage::detect_backend(store_root);
    backend
        .delete_layer(&resolved_head)
        .map_err(|e| e.to_string())?;

    println!("rolled back: update-head discarded, now booting base+update only");
    Ok(())
}

/// Registers a kernel/initramfs pair as the `name` boot generation, atomically activating it.
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

/// Real binary names each named adapter stands in for.
const KNOWN_INTEGRATIONS: &[(&str, &[&str])] = &[
    ("dnf", &["dnf"]),
    ("apt", &["apt-get", "apt"]),
    ("pacman", &["pacman"]),
];

/// Symlinks each present real binary to its adapter (`dnf` -> `layerfs-dnf`).
/// Errs on an unrecognized name rather than booting with it silently inactive.
fn activate_integrations(base: &Path, integrations: &[String]) -> Result<Vec<String>, String> {
    let mut activated = Vec::new();

    for name in integrations {
        let candidates = KNOWN_INTEGRATIONS
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| *c)
            .ok_or_else(|| format!("unknown integration: {name}"))?;

        for real_name in candidates {
            let target = base.join("usr/bin").join(real_name);
            if !target.exists() {
                continue;
            }
            std::fs::remove_file(&target).map_err(|e| e.to_string())?;
            std::os::unix::fs::symlink(format!("layerfs-{name}"), &target)
                .map_err(|e| e.to_string())?;
            activated.push(format!("{real_name} -> layerfs-{name}"));
        }
    }

    Ok(activated)
}

/// Copies a built `layerfs-grub-entries` binary into `etc/grub.d/41_layerfs`,
/// executable, so `grub2-mkconfig` picks it up on the installed system.
fn install_grub_entries(base: &Path, bin: &Path) -> Result<(), String> {
    let grub_d = base.join("etc/grub.d");
    if !grub_d.is_dir() {
        return Err(format!("{} not found: not a GRUB system", grub_d.display()));
    }

    let dest = grub_d.join("41_layerfs");
    std::fs::copy(bin, &dest).map_err(|e| e.to_string())?;

    let mut perms = std::fs::metadata(&dest)
        .map_err(|e| e.to_string())?
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(&dest, perms).map_err(|e| e.to_string())?;

    Ok(())
}

/// Converts a static (not live) source directory into `base`/`override`/
/// `data` under `store_root`. Not the full reboot-into-migration flow yet.
fn install_cmd(
    store_root: &Path,
    source: &Path,
    integrations: &[String],
    grub_entries: Option<&Path>,
) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!("{} is not a directory", source.display()));
    }

    let base = store_root.join("base");
    if base.exists() {
        return Err(format!("{} already has a base layer", base.display()));
    }
    std::fs::create_dir_all(store_root).map_err(|e| e.to_string())?;

    let backend = layerfs_storage::detect_backend(store_root);
    backend
        .prepare_layer(&base, Some(source))
        .map_err(|e| e.to_string())?;

    let data_root = store_root.join("data");
    std::fs::create_dir_all(&data_root).map_err(|e| e.to_string())?;
    for name in layerfs_core::DATA_MOUNTS {
        let from = base.join(name);
        if from.is_dir() {
            move_dir(&from, &data_root.join(name)).map_err(|e| e.to_string())?;
        }
    }

    std::fs::create_dir_all(store_root.join("override")).map_err(|e| e.to_string())?;

    for link in activate_integrations(&base, integrations)? {
        println!("activated {link}");
    }

    if let Some(bin) = grub_entries {
        install_grub_entries(&base, bin)?;
        println!("installed etc/grub.d/41_layerfs");
    }

    let report = layerfs_storage::validate::verify_root(&base);
    for check in &report.checks {
        println!(
            "[{}] {}",
            if check.ok { "ok" } else { "FAIL" },
            check.description
        );
    }
    if !report.passed() {
        return Err("installed base failed validation".to_string());
    }

    println!("installed store at {}", store_root.display());
    Ok(())
}

/// Renames `from` to `to`, falling back to copy+delete on `EXDEV` (e.g. moving
/// out of a Btrfs subvolume, which rename can't cross even on the same fs).
fn move_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(18) /* EXDEV */ => {
            layerfs_storage::copy_tree::copy_tree(from, to)?;
            std::fs::remove_dir_all(from)
        }
        Err(e) => Err(e),
    }
}

/// Applies the current UPDATE_HEAD/UPDATE to `live_root` live, scoped to
/// whatever subtree is safe (see `layerfs_storage::live_update`).
fn apply_now_cmd(store_root: &Path, live_root: &Path) -> Result<(), String> {
    use layerfs_storage::live_update::Outcome;

    match layerfs_storage::live_update::apply(store_root, live_root).map_err(|e| e.to_string())? {
        Outcome::NothingToApply => {
            Err("nothing to apply: no update or update-head present".to_string())
        }
        Outcome::RequiresReboot => Err(
            "cannot apply live: touches a path outside usr/opt or a shared library/kernel"
                .to_string(),
        ),
        Outcome::Applied(scopes) => {
            println!(
                "applied live: {} ({})",
                live_root.display(),
                scopes.join(", ")
            );
            Ok(())
        }
    }
}

fn transaction_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("txn-{nanos}")
}
