use std::fs;
use std::io;
use std::path::Path;

use crate::{discover, overlay, risk};

#[derive(Debug)]
pub enum Outcome {
    NothingToApply,
    RequiresReboot,
    Applied(Vec<String>),
}

/// Applies the store's current UPDATE_HEAD/UPDATE to `live_root` without a
/// reboot, scoped to whichever top-level directories (`usr`, `opt`) it's
/// safe to swap in place — never the whole root, since that can orphan
/// mounts nested under paths like `/proc` or `/home`. Refuses (rather than
/// guessing) if the update touches a shared library, the kernel, or
/// anything outside that safe scope.
pub fn apply(store_root: &Path, live_root: &Path) -> io::Result<Outcome> {
    let discovered = discover(store_root).map_err(io::Error::other)?;

    let mut lowers = Vec::new();
    if let Some(head) = &discovered.update_head {
        lowers.push(head.as_path());
    }
    if let Some(update) = &discovered.update {
        lowers.push(update.as_path());
    }
    if lowers.is_empty() {
        return Ok(Outcome::NothingToApply);
    }

    if let Some(head) = &discovered.update_head
        && risk::layer_is_risky(head)?
    {
        return Ok(Outcome::RequiresReboot);
    }

    let Some(scopes) = risk::hot_applicable_scopes(&lowers)? else {
        return Ok(Outcome::RequiresReboot);
    };

    let override_root = discovered
        .r#override
        .unwrap_or_else(|| store_root.join("override"));
    let hot = store_root.join("hot");

    let mut applied = Vec::new();
    for scope in scopes {
        let scope_lowers: Vec<_> = lowers
            .iter()
            .map(|l| l.join(&scope))
            .filter(|p| p.is_dir())
            .collect();
        if scope_lowers.is_empty() {
            continue;
        }
        let scope_lowers: Vec<&Path> = scope_lowers.iter().map(|p| p.as_path()).collect();

        let override_dir = override_root.join(&scope);
        fs::create_dir_all(&override_dir)?;

        overlay::hot_apply(
            &live_root.join(&scope),
            &scope_lowers,
            &override_dir,
            &hot.join(&scope).join("work"),
            &hot.join(&scope).join("snapshot"),
            &hot.join(&scope).join("staging"),
        )?;
        applied.push(scope);
    }

    if applied.is_empty() {
        Ok(Outcome::NothingToApply)
    } else {
        Ok(Outcome::Applied(applied))
    }
}
