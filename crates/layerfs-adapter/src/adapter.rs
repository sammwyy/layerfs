use std::env;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::env::bin_env_var;

const DEFAULT_STORE: &str = "/run/layerfs-store";
const DEFAULT_LIVE_ROOT: &str = "/";
const DEFAULT_LAYERCTL: &str = "layerctl";

/// A package-manager adapter: exec passthrough for read-only invocations,
/// a real system transaction for mutating ones. `name` drives both the
/// `LAYERFS_<NAME>_BIN` override and the transaction's `adapter` field.
pub struct Adapter {
    pub name: &'static str,
    pub default_binary: &'static str,
}

impl Adapter {
    /// Reads argv/env, classifies with `is_mutating`, and either execs the
    /// real binary directly or drives it through a staged transaction.
    pub fn run(&self, is_mutating: impl Fn(&[String]) -> bool) -> ExitCode {
        let args: Vec<String> = env::args().skip(1).collect();
        let bin =
            env::var(bin_env_var(self.name)).unwrap_or_else(|_| self.default_binary.to_string());

        if !is_mutating(&args) {
            return self.passthrough(&bin, &args);
        }

        let store_root =
            PathBuf::from(env::var("LAYERFS_STORE").unwrap_or_else(|_| DEFAULT_STORE.to_string()));
        self.transacted(&store_root, &bin, &args)
    }

    fn passthrough(&self, bin: &str, args: &[String]) -> ExitCode {
        let err = Command::new(bin).args(args).exec();
        self.fail(&format!("failed to exec {bin}: {err}"))
    }

    /// Runs the transaction in a separate `layerctl` process rather than
    /// in-process: `Transaction::stage` unshares into a private mount
    /// namespace, and that namespace dies with whatever process called it
    /// — running it here would leave this process unable to hot-apply to
    /// the real one afterward.
    fn transacted(&self, store_root: &Path, bin: &str, args: &[String]) -> ExitCode {
        let layerctl =
            env::var("LAYERFS_LAYERCTL_BIN").unwrap_or_else(|_| DEFAULT_LAYERCTL.to_string());

        let status = Command::new(&layerctl)
            .arg("--store")
            .arg(store_root)
            .arg("transaction")
            .arg("--")
            .arg(bin)
            .args(args)
            .status();

        let status = match status {
            Ok(s) => s,
            Err(e) => return self.fail(&format!("failed to run {layerctl}: {e}")),
        };

        if !status.success() {
            return ExitCode::from(status.code().unwrap_or(1).clamp(1, 255) as u8);
        }

        self.try_hot_apply(store_root);
        ExitCode::SUCCESS
    }

    /// Applies the just-committed update to the running system if it's
    /// judged safe (no shared libraries/kernel touched); otherwise reports
    /// that a reboot is needed, never guessing wrong in the risky direction.
    fn try_hot_apply(&self, store_root: &Path) {
        let discovered = match layerfs_storage::discover(store_root) {
            Ok(d) => d,
            Err(_) => return self.report_reboot_required(),
        };
        let Some(head) = &discovered.update_head else {
            return self.report_reboot_required();
        };

        match layerfs_storage::risk::layer_is_risky(head) {
            Ok(true) | Err(_) => self.report_reboot_required(),
            Ok(false) => {
                let live_root = PathBuf::from(
                    env::var("LAYERFS_LIVE_ROOT").unwrap_or_else(|_| DEFAULT_LIVE_ROOT.to_string()),
                );
                let override_dir = discovered
                    .r#override
                    .unwrap_or_else(|| store_root.join("override"));
                let _ = std::fs::create_dir_all(&override_dir);

                let mut lowers = vec![head.as_path()];
                if let Some(update) = &discovered.update {
                    lowers.push(update.as_path());
                }

                let hot = store_root.join("hot");
                let result = layerfs_storage::overlay::hot_apply(
                    &live_root,
                    &lowers,
                    &override_dir,
                    &hot.join("work"),
                    &hot.join("snapshot"),
                    &hot.join("staging"),
                );

                match result {
                    Ok(()) => eprintln!("{}: update applied live, no reboot needed", self.name),
                    Err(e) => eprintln!(
                        "{}: update committed but live apply failed ({e}); reboot required to apply",
                        self.name
                    ),
                }
            }
        }
    }

    fn report_reboot_required(&self) {
        eprintln!("{}: update committed; reboot required to apply", self.name);
    }

    fn fail(&self, msg: &str) -> ExitCode {
        eprintln!("{}: {msg}", self.name);
        ExitCode::FAILURE
    }
}
