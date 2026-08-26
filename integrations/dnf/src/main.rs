//! Stands in for `dnf` itself so `sudo dnf install foo` transparently
//! becomes a system transaction. Control comes from env vars, never argv,
//! so nothing collides with dnf's own flags.

mod classify;

use std::env;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use layerfs_storage::DirectoryBackend;
use layerfs_transaction::Transaction;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let dnf_bin = env::var("LAYERFS_DNF_BIN").unwrap_or_else(|_| "dnf".to_string());

    if !classify::is_mutating(&args) {
        return run_passthrough(&dnf_bin, &args);
    }

    let store_root = PathBuf::from(
        env::var("LAYERFS_STORE").unwrap_or_else(|_| "/run/layerfs-store".to_string()),
    );
    run_transacted(&store_root, &dnf_bin, &args)
}

/// `exec` replaces this process, so stdio/exit code match calling `dnf` directly.
fn run_passthrough(dnf_bin: &str, args: &[String]) -> ExitCode {
    let err = Command::new(dnf_bin).args(args).exec();
    eprintln!("layerfs-dnf: failed to exec {dnf_bin}: {err}");
    ExitCode::FAILURE
}

fn run_transacted(store_root: &PathBuf, dnf_bin: &str, args: &[String]) -> ExitCode {
    let backend = DirectoryBackend::new(store_root);
    let mut txn = match Transaction::begin(store_root.clone(), &backend, transaction_id(), "dnf") {
        Ok(t) => t,
        Err(e) => return fail(&format!("begin: {e}")),
    };

    if let Err(e) = txn.stage(store_root.join("transaction-root")) {
        return fail(&format!("stage: {e}"));
    }

    let status = match txn.execute(dnf_bin, args) {
        Ok(s) => s,
        Err(e) => return fail(&format!("execute: {e}")),
    };

    if !status.success() {
        eprintln!("layerfs-dnf: {dnf_bin} exited with {status}; transaction discarded");
        return ExitCode::from(status.code().unwrap_or(1).clamp(1, 255) as u8);
    }

    if let Err(e) = txn.validate() {
        return fail(&format!("validate: {e}"));
    }
    if let Err(e) = txn.commit() {
        return fail(&format!("commit: {e}"));
    }

    ExitCode::SUCCESS
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("layerfs-dnf: {msg}");
    ExitCode::FAILURE
}

fn transaction_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("dnf-{nanos}")
}
