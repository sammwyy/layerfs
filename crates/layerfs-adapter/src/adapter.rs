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
    pub fn run(
        &self,
        is_mutating: impl Fn(&[String]) -> bool,
        export_manifest: impl Fn() -> Result<String, String>,
    ) -> ExitCode {
        let args: Vec<String> = env::args().skip(1).collect();
        let bin =
            env::var(bin_env_var(self.name)).unwrap_or_else(|_| self.default_binary.to_string());

        if !is_mutating(&args) {
            return self.passthrough(&bin, &args);
        }

        let store_root =
            PathBuf::from(env::var("LAYERFS_STORE").unwrap_or_else(|_| DEFAULT_STORE.to_string()));
        self.transacted(&store_root, &bin, &args, export_manifest)
    }

    fn passthrough(&self, bin: &str, args: &[String]) -> ExitCode {
        let err = Command::new(bin).args(args).exec();
        self.fail(&format!("failed to exec {bin}: {err}"))
    }

    /// Runs in a separate `layerctl` process: its mount-namespace unshare
    /// dies with it, leaving this process able to hot-apply afterward.
    fn transacted(
        &self,
        store_root: &Path,
        bin: &str,
        args: &[String],
        export_manifest: impl Fn() -> Result<String, String>,
    ) -> ExitCode {
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

        self.save_manifest(store_root, export_manifest);
        self.try_hot_apply(store_root);
        ExitCode::SUCCESS
    }

    /// Best-effort: a failed save shouldn't undo an already-committed transaction.
    fn save_manifest(
        &self,
        store_root: &Path,
        export_manifest: impl Fn() -> Result<String, String>,
    ) {
        let content = match export_manifest() {
            Ok(content) => content,
            Err(e) => return eprintln!("{}: manifest export failed: {e}", self.name),
        };

        let dir = store_root.join("manifest");
        let path = dir.join(format!("{}.json", self.name));
        let temporary = dir.join(format!(".{}.json.new", self.name));
        let result = std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&temporary, content))
            .and_then(|()| std::fs::rename(&temporary, &path));
        if let Err(e) = result {
            eprintln!("{}: failed to save manifest: {e}", self.name);
        }
    }

    /// Applies the just-committed update live if safe, else reports a reboot is needed.
    fn try_hot_apply(&self, store_root: &Path) {
        use layerfs_storage::live_update::Outcome;

        let live_root = PathBuf::from(
            env::var("LAYERFS_LIVE_ROOT").unwrap_or_else(|_| DEFAULT_LIVE_ROOT.to_string()),
        );

        match layerfs_storage::live_update::apply(store_root, &live_root) {
            Ok(Outcome::Applied(scopes)) => {
                eprintln!(
                    "{}: update applied live ({}), no reboot needed",
                    self.name,
                    scopes.join(", ")
                )
            }
            Ok(Outcome::NothingToApply) | Ok(Outcome::RequiresReboot) => {
                self.report_reboot_required()
            }
            Err(e) => eprintln!(
                "{}: update committed but live apply failed ({e}); reboot required to apply",
                self.name
            ),
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
