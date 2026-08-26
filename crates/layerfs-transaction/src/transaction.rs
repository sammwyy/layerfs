use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use rustix::mount::{UnmountFlags, unmount};
use rustix::process::chroot;
use rustix::thread::{UnshareFlags, unshare_unsafe};

use layerfs_core::{Layer, LayerKind, LayerStack};
use layerfs_storage::{DiscoveredStore, StorageBackend};

use crate::error::TransactionError;
use crate::lock::TransactionLock;
use crate::state::{TransactionRecord, TransactionState};

struct Staged {
    update_next: PathBuf,
    head_next: PathBuf,
    target: PathBuf,
    /// UPDATE this generation was cloned from, if any — deleted on commit
    /// once the new generation is active. `None` for a bootstrap
    /// transaction (no prior UPDATE existed).
    superseded_update: Option<PathBuf>,
}

/// Drives one system transaction: staging → execution → validation →
/// atomic commit.
///
/// Never mutates the active UPDATE/UPDATE_HEAD in place — only the
/// `.next` generations created by `stage()` are writable, and only
/// `commit()` may change which generation is active. If a `Transaction`
/// is dropped without having committed, any staged generations are
/// deleted so a crash or an early return never leaves the previous
/// system's bootability in question (section 28).
pub struct Transaction<'a> {
    _lock: TransactionLock,
    backend: &'a dyn StorageBackend,
    store_root: PathBuf,
    record: TransactionRecord,
    staged: Option<Staged>,
}

impl<'a> Transaction<'a> {
    pub fn begin(
        store_root: impl Into<PathBuf>,
        backend: &'a dyn StorageBackend,
        id: impl Into<String>,
        adapter: impl Into<String>,
    ) -> Result<Self, TransactionError> {
        let store_root = store_root.into();
        let lock = TransactionLock::acquire(&store_root.join("transaction.lock"))?;
        let record = TransactionRecord {
            id: id.into(),
            kind: "system".to_string(),
            started_at_unix: 0,
            adapter: adapter.into(),
            state: TransactionState::Preparing,
        };

        Ok(Self {
            _lock: lock,
            backend,
            store_root,
            record,
            staged: None,
        })
    }

    /// Clones the active UPDATE into a fresh generation, prepares an empty
    /// writable HEAD.next, and mounts `HEAD.next > UPDATE.next > BASE` at
    /// `target` inside a private mount namespace — OVERRIDE is never part
    /// of a system transaction's view (section 13).
    ///
    /// An active UPDATE_HEAD blocks staging: consolidating it into
    /// UPDATE.next requires `layerfs_core::squash`, which is not
    /// implemented yet (Milestone 6). The very first transaction against a
    /// fresh store — no UPDATE or UPDATE_HEAD yet — is unaffected.
    pub fn stage(&mut self, target: impl Into<PathBuf>) -> Result<(), TransactionError> {
        if self.staged.is_some() {
            return Err(TransactionError::AlreadyStaged);
        }

        let discovered = discover(&self.store_root)?;

        if discovered.update_head.is_some() {
            return Err(TransactionError::SquashRequired);
        }

        let update_next =
            layerfs_storage::generations::new_generation_path(&self.store_root, "update")?;
        match &discovered.update {
            Some(active_update) => self.backend.clone_layer(active_update, &update_next)?,
            None => self.backend.prepare_layer(&update_next, None)?,
        }

        let head_next =
            layerfs_storage::generations::new_generation_path(&self.store_root, "head")?;
        self.backend.prepare_layer(&head_next, None)?;

        // SAFETY: only NEWNS is requested; we don't use CLONE_FILES/CLONE_FS,
        // the flags unshare_unsafe's docs warn about.
        unsafe { unshare_unsafe(UnshareFlags::NEWNS) }
            .map_err(|e| TransactionError::Namespace(e.to_string()))?;

        let mut stack = LayerStack::new();
        stack.push(Layer::new(
            LayerKind::UpdateHead,
            "head-next",
            head_next.clone(),
            false,
        ));
        stack.push(Layer::new(
            LayerKind::Update,
            "update-next",
            update_next.clone(),
            true,
        ));
        stack.push(Layer::new(
            LayerKind::Base,
            "base",
            discovered.base.clone(),
            true,
        ));

        let target = target.into();
        layerfs_storage::overlay::assemble(&stack, &discovered.work, &target)?;

        self.record.state = TransactionState::Running;
        self.staged = Some(Staged {
            update_next,
            head_next,
            target,
            superseded_update: discovered.update,
        });

        Ok(())
    }

    /// Runs `program` with `args` inside the staged transaction root,
    /// chrooted so it cannot see anything outside the assembled
    /// `HEAD.next > UPDATE.next > BASE` view — for development, in place
    /// of a real package-manager adapter (section 12).
    pub fn execute(&self, program: &str, args: &[String]) -> Result<ExitStatus, TransactionError> {
        let staged = self.staged.as_ref().ok_or(TransactionError::NotStaged)?;
        let target = staged.target.clone();

        // SAFETY: chroot+chdir are async-signal-safe syscalls with no
        // allocation; this closure runs in the forked child before exec.
        unsafe {
            Command::new(program)
                .args(args)
                .pre_exec(move || {
                    chroot(&target)?;
                    std::env::set_current_dir("/")?;
                    Ok(())
                })
                .status()
                .map_err(TransactionError::from)
        }
    }

    /// Runs MVP structural checks against the staged root.
    pub fn validate(&mut self) -> Result<(), TransactionError> {
        let staged = self.staged.as_ref().ok_or(TransactionError::NotStaged)?;
        self.record.state = TransactionState::Validating;

        let report = layerfs_storage::validate::verify_root(&staged.target);
        if !report.passed() {
            let failed: Vec<_> = report
                .checks
                .iter()
                .filter(|c| !c.ok)
                .map(|c| c.description.clone())
                .collect();
            return Err(TransactionError::ValidationFailed(failed.join(", ")));
        }

        Ok(())
    }

    /// Unmounts the transaction root, freezes the staged generations, and
    /// atomically activates them as the new UPDATE/UPDATE_HEAD — the only
    /// step allowed to change what is active. Garbage-collects the
    /// superseded UPDATE generation, if any.
    pub fn commit(&mut self) -> Result<(), TransactionError> {
        let staged = self.staged.take().ok_or(TransactionError::NotStaged)?;
        self.record.state = TransactionState::Committing;

        let _ = unmount(&staged.target, UnmountFlags::DETACH);

        self.backend.freeze_layer(&staged.update_next)?;
        self.backend.freeze_layer(&staged.head_next)?;

        layerfs_storage::generations::activate(&self.store_root, "update", &staged.update_next)?;
        layerfs_storage::generations::activate(&self.store_root, "update-head", &staged.head_next)?;

        if let Some(old_update) = staged.superseded_update {
            let _ = self.backend.delete_layer(&old_update);
        }

        self.record.state = TransactionState::Committed;
        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if self.record.state == TransactionState::Committed {
            return;
        }

        if let Some(staged) = self.staged.take() {
            let _ = unmount(&staged.target, UnmountFlags::DETACH);
            let _ = self.backend.delete_layer(&staged.update_next);
            let _ = self.backend.delete_layer(&staged.head_next);
        }
    }
}

fn discover(store_root: &Path) -> Result<DiscoveredStore, TransactionError> {
    layerfs_storage::discover(store_root).map_err(TransactionError::from)
}
