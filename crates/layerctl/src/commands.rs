use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use layerfs_transaction::{Transaction, TransactionLock};

use crate::cli::{Bootloader, Command, Invocation};
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
        Command::Checkpoint {
            name,
            bootloader,
            esp,
            grub_cfg,
            grubenv,
        } => checkpoint_cmd(&name, bootloader, &esp, &grub_cfg, &grubenv),
        Command::Install {
            source,
            integrations,
            adapter_bins,
            grub_entries,
        } => install_cmd(
            &store_root,
            &source,
            &integrations,
            &adapter_bins,
            grub_entries.as_deref(),
        ),
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
    register_new_kernel(store_root)?;
    Ok(())
}

/// If this transaction wrote a new kernel/initramfs into `/boot`, registers
/// it as the new `head` boot generation — the transactional-boot-artifacts
/// counterpart to the root `UPDATE_HEAD` this same transaction just
/// committed. A no-op for any transaction that didn't touch `/boot`.
fn register_new_kernel(store_root: &Path) -> Result<(), String> {
    let Some(update_head) = store::discover_layers(store_root)?.update_head else {
        return Ok(());
    };
    let Some((kernel, initramfs)) = layerfs_storage::boot::find_new_kernel(&update_head) else {
        return Ok(());
    };

    let dest =
        layerfs_storage::boot::register(&boot_store(store_root), "head", &kernel, &initramfs)
            .map_err(|e| e.to_string())?;
    println!("registered boot generation: {}", dest.display());
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

/// Sets the default next-boot entry to `name` (one of the four canonical
/// checkpoints) without touching the currently mounted root — see section
/// 25. Only systemd-boot is supported so far: it's just an ESP file write,
/// with no external tool to shell out to. GRUB's equivalent
/// (`grubenv`/`grub2-set-default`) isn't implemented yet.
fn checkpoint_cmd(
    name: &str,
    bootloader: Bootloader,
    esp: &Path,
    grub_cfg: &Path,
    grubenv: &Path,
) -> Result<(), String> {
    let checkpoint: layerfs_core::Checkpoint = name.parse().map_err(|e| format!("{e}"))?;

    match bootloader {
        Bootloader::SystemdBoot => {
            let entry_id = format!("layerfs-{}.conf", checkpoint.name());
            let entry_path = esp.join("loader/entries").join(&entry_id);
            if !entry_path.is_file() {
                return Err(format!(
                    "{} not found: generate systemd-boot entries for this ESP first",
                    entry_path.display()
                ));
            }
            set_systemd_boot_default(esp, &entry_id)?;
            println!("next boot: {} ({entry_id})", checkpoint.name());
        }
        Bootloader::Grub => {
            let entry_id = format!("layerfs-{}", checkpoint.name());
            if !grub_entry_exists(grub_cfg, &entry_id)? {
                return Err(format!(
                    "no menuentry --id '{entry_id}' in {}: generate GRUB entries first",
                    grub_cfg.display()
                ));
            }
            set_grub_default(grubenv, &entry_id)?;
            println!(
                "next boot: {} ({entry_id}) — requires GRUB_DEFAULT=saved in this system's GRUB config",
                checkpoint.name()
            );
        }
    }

    Ok(())
}

/// Rewrites (or creates) `<esp>/loader/loader.conf`'s `default` line,
/// leaving every other line untouched, via the same write-temp-then-rename
/// pattern used for metadata commits elsewhere in this codebase.
fn set_systemd_boot_default(esp: &Path, entry_id: &str) -> Result<(), String> {
    let loader_dir = esp.join("loader");
    std::fs::create_dir_all(&loader_dir).map_err(|e| e.to_string())?;
    let loader_conf = loader_dir.join("loader.conf");

    let mut lines: Vec<String> = match std::fs::read_to_string(&loader_conf) {
        Ok(contents) => contents.lines().map(String::from).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.to_string()),
    };

    let default_line = format!("default {entry_id}");
    match lines
        .iter_mut()
        .find(|line| line.trim_start().starts_with("default "))
    {
        Some(existing) => *existing = default_line,
        None => lines.insert(0, default_line),
    }

    let temporary = loader_dir.join(".loader.conf.new");
    std::fs::write(&temporary, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    std::fs::rename(temporary, loader_conf).map_err(|e| e.to_string())
}

/// Whether `grub_cfg` contains a `menuentry --id '<entry_id>'`. The
/// generated entries live inside one rendered `grub.cfg`, not as separate
/// files the way systemd-boot's BLS entries do, so this is the only way to
/// check one exists before pointing `grubenv` at it.
fn grub_entry_exists(grub_cfg: &Path, entry_id: &str) -> Result<bool, String> {
    match std::fs::read_to_string(grub_cfg) {
        Ok(contents) => Ok(contents.contains(&format!("--id '{entry_id}'"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

const GRUB_ENVBLK_SIZE: usize = 1024;
const GRUB_ENVBLK_SIGNATURE: &str = "# GRUB Environment Block\n";

/// Sets `saved_entry=<entry_id>` in `grubenv`, preserving any other
/// variables already stored there — GRUB reads `saved_entry` as the
/// default boot entry when the system's GRUB config sets
/// `GRUB_DEFAULT=saved` (a `/etc/default/grub` setting outside LayerFS's
/// control, so this only takes effect if that's already set up).
fn set_grub_default(grubenv: &Path, entry_id: &str) -> Result<(), String> {
    let mut vars = read_grubenv(grubenv)?;
    match vars.iter_mut().find(|(k, _)| k == "saved_entry") {
        Some((_, v)) => *v = entry_id.to_string(),
        None => vars.push(("saved_entry".to_string(), entry_id.to_string())),
    }
    write_grubenv(grubenv, &vars)
}

/// Parses the fixed-size `grub2-editenv` block format: a fixed signature
/// line, `key=value\n` entries, then `#`-padding out to
/// `GRUB_ENVBLK_SIZE` bytes total. A missing file reads as no variables —
/// `grub2-mkconfig` creates a fresh one the same way.
fn read_grubenv(path: &Path) -> Result<Vec<(String, String)>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let text = String::from_utf8_lossy(&bytes);
    let body = text.strip_prefix(GRUB_ENVBLK_SIGNATURE).unwrap_or(&text);

    Ok(body
        .split('\n')
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect())
}

fn write_grubenv(path: &Path, vars: &[(String, String)]) -> Result<(), String> {
    let mut body = GRUB_ENVBLK_SIGNATURE.to_string();
    for (key, value) in vars {
        body.push_str(key);
        body.push('=');
        body.push_str(value);
        body.push('\n');
    }
    if body.len() > GRUB_ENVBLK_SIZE {
        return Err(format!(
            "grubenv contents ({} bytes) exceed the {GRUB_ENVBLK_SIZE}-byte block",
            body.len()
        ));
    }
    body.push_str(&"#".repeat(GRUB_ENVBLK_SIZE - body.len()));

    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(".grubenv.new");
    std::fs::write(&temporary, body.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}

/// Real binary names each named adapter stands in for.
const KNOWN_INTEGRATIONS: &[(&str, &[&str])] = &[
    ("dnf", &["dnf"]),
    ("apt", &["apt-get", "apt"]),
    ("pacman", &["pacman"]),
];

/// Installs each adapter binary as `layerfs-<name>`, preserves the real
/// binary as `<real_name>.layerfs-real` (adapters fall back to that name
/// when unwrapped by an env var), and symlinks the real name to the
/// adapter. Errs on an unrecognized name rather than booting with it
/// silently inactive, and on a missing adapter binary rather than leaving
/// a symlink that points nowhere.
fn activate_integrations(
    base: &Path,
    integrations: &[String],
    adapter_bins: &[(String, PathBuf)],
) -> Result<Vec<String>, String> {
    let mut activated = Vec::new();

    for name in integrations {
        let candidates = KNOWN_INTEGRATIONS
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| *c)
            .ok_or_else(|| format!("unknown integration: {name}"))?;

        let adapter_bin = adapter_bins
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p)
            .ok_or_else(|| format!("--adapter-bin required for integration: {name}"))?;

        let mut installed_wrapper = false;

        for real_name in candidates {
            let target = base.join("usr/bin").join(real_name);
            if !target.exists() {
                continue;
            }

            if !installed_wrapper {
                let wrapper = base.join("usr/bin").join(format!("layerfs-{name}"));
                std::fs::copy(adapter_bin, &wrapper).map_err(|e| e.to_string())?;
                installed_wrapper = true;
            }

            let real_backup = base
                .join("usr/bin")
                .join(format!("{real_name}.layerfs-real"));
            std::fs::rename(&target, &real_backup).map_err(|e| e.to_string())?;
            std::os::unix::fs::symlink(format!("layerfs-{name}"), &target)
                .map_err(|e| e.to_string())?;
            activated.push(format!(
                "{real_name} -> layerfs-{name} (real preserved as {real_name}.layerfs-real)"
            ));
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
    adapter_bins: &[(String, PathBuf)],
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

    for name in layerfs_core::VIRTUAL_MOUNTS {
        std::fs::create_dir_all(base.join(name)).map_err(|e| e.to_string())?;
    }

    for link in activate_integrations(&base, integrations, adapter_bins)? {
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

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "layerfs-activate-integrations-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("base/usr/bin")).unwrap();
        root
    }

    fn fake_binary(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn wraps_real_binary_and_preserves_it() {
        let root = scratch("wraps");
        let base = root.join("base");
        fake_binary(&base.join("usr/bin/dnf"), "real dnf");
        let adapter_bin = root.join("layerfs-dnf-built");
        fake_binary(&adapter_bin, "adapter dnf");

        let activated = activate_integrations(
            &base,
            &["dnf".to_string()],
            &[("dnf".to_string(), adapter_bin)],
        )
        .unwrap();

        assert_eq!(
            activated,
            vec!["dnf -> layerfs-dnf (real preserved as dnf.layerfs-real)"]
        );
        assert_eq!(
            fs::read_to_string(base.join("usr/bin/dnf.layerfs-real")).unwrap(),
            "real dnf"
        );
        assert_eq!(
            fs::read_to_string(base.join("usr/bin/layerfs-dnf")).unwrap(),
            "adapter dnf"
        );
        assert_eq!(
            fs::read_link(base.join("usr/bin/dnf")).unwrap(),
            Path::new("layerfs-dnf")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_adapter_binary() {
        let root = scratch("missing-adapter-bin");
        let base = root.join("base");
        fake_binary(&base.join("usr/bin/dnf"), "real dnf");

        let err = activate_integrations(&base, &["dnf".to_string()], &[]).unwrap_err();

        assert!(err.contains("--adapter-bin required for integration: dnf"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unknown_integration() {
        let root = scratch("unknown");
        let base = root.join("base");

        let err = activate_integrations(&base, &["bogus".to_string()], &[]).unwrap_err();

        assert!(err.contains("unknown integration: bogus"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn skips_absent_real_binary_without_error() {
        let root = scratch("absent");
        let base = root.join("base");
        let adapter_bin = root.join("layerfs-dnf-built");
        fake_binary(&adapter_bin, "adapter dnf");

        let activated = activate_integrations(
            &base,
            &["dnf".to_string()],
            &[("dnf".to_string(), adapter_bin)],
        )
        .unwrap();

        assert!(activated.is_empty());
        assert!(!base.join("usr/bin/layerfs-dnf").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn set_systemd_boot_default_creates_loader_conf() {
        let root = scratch("checkpoint-create");
        set_systemd_boot_default(&root, "layerfs-safe.conf").unwrap();

        assert_eq!(
            fs::read_to_string(root.join("loader/loader.conf")).unwrap(),
            "default layerfs-safe.conf\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn set_systemd_boot_default_replaces_existing_default_only() {
        let root = scratch("checkpoint-replace");
        fs::create_dir_all(root.join("loader")).unwrap();
        fs::write(
            root.join("loader/loader.conf"),
            "timeout 3\ndefault layerfs-normal.conf\nconsole-mode auto\n",
        )
        .unwrap();

        set_systemd_boot_default(&root, "layerfs-base.conf").unwrap();

        assert_eq!(
            fs::read_to_string(root.join("loader/loader.conf")).unwrap(),
            "timeout 3\ndefault layerfs-base.conf\nconsole-mode auto\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn systemd_boot_checkpoint(name: &str, esp: &Path) -> Result<(), String> {
        checkpoint_cmd(
            name,
            Bootloader::SystemdBoot,
            esp,
            Path::new("/nonexistent-grub-cfg"),
            Path::new("/nonexistent-grubenv"),
        )
    }

    fn grub_checkpoint(name: &str, grub_cfg: &Path, grubenv: &Path) -> Result<(), String> {
        checkpoint_cmd(
            name,
            Bootloader::Grub,
            Path::new("/nonexistent-esp"),
            grub_cfg,
            grubenv,
        )
    }

    #[test]
    fn checkpoint_cmd_rejects_unknown_checkpoint_name() {
        let root = scratch("checkpoint-bad-name");
        assert!(systemd_boot_checkpoint("bogus", &root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checkpoint_cmd_rejects_missing_entry() {
        let root = scratch("checkpoint-missing-entry");
        let err = systemd_boot_checkpoint("safe", &root).unwrap_err();
        assert!(err.contains("layerfs-safe.conf"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checkpoint_cmd_sets_default_when_entry_present() {
        let root = scratch("checkpoint-ok");
        fs::create_dir_all(root.join("loader/entries")).unwrap();
        fs::write(root.join("loader/entries/layerfs-safe.conf"), "title x\n").unwrap();

        systemd_boot_checkpoint("safe", &root).unwrap();

        assert_eq!(
            fs::read_to_string(root.join("loader/loader.conf")).unwrap(),
            "default layerfs-safe.conf\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn grubenv_round_trips_through_the_fixed_size_block() {
        let root = scratch("grubenv-roundtrip");
        let grubenv = root.join("grubenv");

        write_grubenv(
            &grubenv,
            &[("saved_entry".to_string(), "layerfs-safe".to_string())],
        )
        .unwrap();

        let bytes = fs::read(&grubenv).unwrap();
        assert_eq!(bytes.len(), GRUB_ENVBLK_SIZE);
        assert!(bytes.starts_with(GRUB_ENVBLK_SIGNATURE.as_bytes()));
        assert_eq!(
            read_grubenv(&grubenv).unwrap(),
            vec![("saved_entry".to_string(), "layerfs-safe".to_string())]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn set_grub_default_preserves_other_vars() {
        let root = scratch("grub-default-preserve");
        let grubenv = root.join("grubenv");
        write_grubenv(
            &grubenv,
            &[
                ("saved_entry".to_string(), "layerfs-normal".to_string()),
                ("other_var".to_string(), "kept".to_string()),
            ],
        )
        .unwrap();

        set_grub_default(&grubenv, "layerfs-base").unwrap();

        let mut vars = read_grubenv(&grubenv).unwrap();
        vars.sort();
        assert_eq!(
            vars,
            vec![
                ("other_var".to_string(), "kept".to_string()),
                ("saved_entry".to_string(), "layerfs-base".to_string()),
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn grub_checkpoint_rejects_missing_entry() {
        let root = scratch("grub-checkpoint-missing");
        let grub_cfg = root.join("grub.cfg");
        fs::write(&grub_cfg, "menuentry 'x' --id 'layerfs-normal' {}\n").unwrap();

        let err = grub_checkpoint("safe", &grub_cfg, &root.join("grubenv")).unwrap_err();

        assert!(err.contains("layerfs-safe"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn grub_checkpoint_sets_default_when_entry_present() {
        let root = scratch("grub-checkpoint-ok");
        let grub_cfg = root.join("grub.cfg");
        fs::write(&grub_cfg, "menuentry 'Safe' --id 'layerfs-safe' {}\n").unwrap();
        let grubenv = root.join("grubenv");

        grub_checkpoint("safe", &grub_cfg, &grubenv).unwrap();

        assert_eq!(
            read_grubenv(&grubenv).unwrap(),
            vec![("saved_entry".to_string(), "layerfs-safe".to_string())]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
