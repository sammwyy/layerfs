use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use rustix::mount::{UnmountFlags, mount_bind_recursive, unmount};
use rustix::process::chroot;
use rustix::thread::{UnshareFlags, unshare_unsafe};

use layerfs_core::{Layer, LayerKind, LayerStack, VIRTUAL_MOUNTS};
use layerfs_storage::{DiscoveredStore, StorageBackend};

use crate::error::TransactionError;
use crate::lock::TransactionLock;
use crate::state::{TransactionRecord, TransactionState};

struct Staged {
    update_next: PathBuf,
    head_next: PathBuf,
    target: PathBuf,
    /// Generations this transaction supersedes; GC'd on commit.
    superseded_update: Option<PathBuf>,
    superseded_update_head: Option<PathBuf>,
}

/// Drives one system transaction: stage → execute → validate → commit.
/// An uncommitted `Transaction` deletes its staged generations on drop.
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

    /// Builds UPDATE.next/HEAD.next and mounts them over BASE at `target`
    /// in a private mount namespace — OVERRIDE is never part of a transaction.
    pub fn stage(&mut self, target: impl Into<PathBuf>) -> Result<(), TransactionError> {
        if self.staged.is_some() {
            return Err(TransactionError::AlreadyStaged);
        }

        let discovered = discover(&self.store_root)?;

        let update_next =
            layerfs_storage::generations::new_generation_path(&self.store_root, "update")?;
        match (&discovered.update, &discovered.update_head) {
            (Some(active_update), Some(active_head)) => {
                layerfs_storage::squash::squash(active_update, active_head, &update_next)?;
            }
            (Some(active_update), None) => {
                self.backend.clone_layer(active_update, &update_next)?;
            }
            (None, Some(_)) => {
                return Err(TransactionError::InconsistentState(
                    "update-head is active without an active update".to_string(),
                ));
            }
            (None, None) => {
                self.backend.prepare_layer(&update_next, None)?;
            }
        }

        let head_next =
            layerfs_storage::generations::new_generation_path(&self.store_root, "head")?;
        self.backend.prepare_layer(&head_next, None)?;

        // SAFETY: only NEWNS is requested, not the FILES/FS flags this fn warns about.
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
        mount_virtual_filesystems(&target)?;

        // Resolve now: these are symlink paths, and activate() below
        // repoints them, so GC must target the old generation, not the symlink.
        let superseded_update = resolve_symlink(discovered.update.as_deref())?;
        let superseded_update_head = resolve_symlink(discovered.update_head.as_deref())?;

        self.record.state = TransactionState::Running;
        self.staged = Some(Staged {
            update_next,
            head_next,
            target,
            superseded_update,
            superseded_update_head,
        });

        Ok(())
    }

    /// Runs `program` chrooted into the staged transaction root — for
    /// development, in place of a real package-manager adapter.
    pub fn execute(&self, program: &str, args: &[String]) -> Result<ExitStatus, TransactionError> {
        let staged = self.staged.as_ref().ok_or(TransactionError::NotStaged)?;
        let target = staged.target.clone();

        // SAFETY: chroot+chdir run in the forked child before exec.
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

    /// Freezes and atomically activates the staged generations, then GCs
    /// whatever they superseded.
    pub fn commit(&mut self) -> Result<(), TransactionError> {
        let staged = self.staged.take().ok_or(TransactionError::NotStaged)?;
        self.record.state = TransactionState::Committing;

        unmount_virtual_filesystems(&staged.target);
        let _ = unmount(&staged.target, UnmountFlags::DETACH);

        self.backend.freeze_layer(&staged.update_next)?;
        self.backend.freeze_layer(&staged.head_next)?;

        layerfs_storage::generations::activate(&self.store_root, "update", &staged.update_next)?;
        layerfs_storage::generations::activate(&self.store_root, "update-head", &staged.head_next)?;

        if let Some(old_update) = staged.superseded_update {
            let _ = self.backend.delete_layer(&old_update);
        }
        if let Some(old_head) = staged.superseded_update_head {
            let _ = self.backend.delete_layer(&old_head);
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
            unmount_virtual_filesystems(&staged.target);
            let _ = unmount(&staged.target, UnmountFlags::DETACH);
            let _ = self.backend.delete_layer(&staged.update_next);
            let _ = self.backend.delete_layer(&staged.head_next);
        }
    }
}

fn mount_virtual_filesystems(target: &Path) -> Result<(), TransactionError> {
    for name in VIRTUAL_MOUNTS {
        let dest = target.join(name);
        fs::create_dir_all(&dest)?;
        mount_bind_recursive(Path::new("/").join(name), &dest).map_err(std::io::Error::from)?;
    }
    Ok(())
}

fn unmount_virtual_filesystems(target: &Path) {
    for name in VIRTUAL_MOUNTS {
        let _ = unmount(target.join(name), UnmountFlags::DETACH);
    }
}

fn discover(store_root: &Path) -> Result<DiscoveredStore, TransactionError> {
    layerfs_storage::discover(store_root).map_err(TransactionError::from)
}

fn resolve_symlink(path: Option<&Path>) -> Result<Option<PathBuf>, TransactionError> {
    path.map(fs::canonicalize)
        .transpose()
        .map_err(TransactionError::from)
}
