use std::env;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use layerfs_storage::DirectoryBackend;
use layerfs_transaction::Transaction;

use crate::env::bin_env_var;

const DEFAULT_STORE: &str = "/run/layerfs-store";

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

    fn transacted(&self, store_root: &PathBuf, bin: &str, args: &[String]) -> ExitCode {
        let backend = DirectoryBackend::new(store_root);
        let mut txn = match Transaction::begin(
            store_root.clone(),
            &backend,
            self.transaction_id(),
            self.name,
        ) {
            Ok(t) => t,
            Err(e) => return self.fail(&format!("begin: {e}")),
        };

        if let Err(e) = txn.stage(store_root.join("transaction-root")) {
            return self.fail(&format!("stage: {e}"));
        }

        let status = match txn.execute(bin, args) {
            Ok(s) => s,
            Err(e) => return self.fail(&format!("execute: {e}")),
        };

        if !status.success() {
            eprintln!(
                "{}: {bin} exited with {status}; transaction discarded",
                self.name
            );
            return ExitCode::from(status.code().unwrap_or(1).clamp(1, 255) as u8);
        }

        if let Err(e) = txn.validate() {
            return self.fail(&format!("validate: {e}"));
        }
        if let Err(e) = txn.commit() {
            return self.fail(&format!("commit: {e}"));
        }

        ExitCode::SUCCESS
    }

    fn fail(&self, msg: &str) -> ExitCode {
        eprintln!("{}: {msg}", self.name);
        ExitCode::FAILURE
    }

    fn transaction_id(&self) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!("{}-{nanos}", self.name)
    }
}
