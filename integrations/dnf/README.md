# DNF adapter (layerfs-dnf)

Stands in for `dnf` itself so `sudo dnf install foo` transparently becomes
a LayerFS system transaction. Not a wrapper script or a DNF plugin — DNF's
Python plugin hooks fire from inside an already-running `dnf` process,
after it's already writing to the live root, too late to chroot it into an
isolated view. Interception has to happen at process start.

Every argument is passed straight through to the real `dnf`; control comes
from environment variables only, so nothing collides with dnf's own flags:

```text
LAYERFS_DNF_BIN      path to the real dnf binary (default: "dnf")
LAYERFS_STORE        store root (default: /run/layerfs-store)
LAYERFS_LAYERCTL_BIN path to layerctl, used to run the transaction (default: "layerctl")
LAYERFS_LIVE_ROOT    root to hot-apply safe updates to (default: "/")
```

After a successful commit, a safe update (confined to `usr`/`opt`, no
shared library/kernel/systemd changes) is applied to the running system
immediately, no reboot — see `layerctl apply-now` in the top-level README.
Anything else (touches `etc`, a shared library, `/boot`, systemd) is
committed but left for the next reboot, and says so.

`layerfs_dnf::classify::is_mutating` decides whether an invocation needs a
transaction: an explicit allowlist of read-only verbs (`search`, `info`,
`list`, ...) runs directly against the live system; everything else,
including unrecognized verbs, is treated as mutating — a wasted empty
transaction is harmless, skipping a real one is not. `--downloadonly`
overrides this back to read-only regardless of verb, since it only
populates the cache.

That classification is all this crate contributes — the passthrough exec,
transaction staging, chrooted execution, validation, and commit all live in
`layerfs-adapter`, shared with every other package-manager adapter. Future
adapters (`layerfs-apt`, `layerfs-pacman`) supply their own `is_mutating`
and reuse the same runner, each in its own crate under `integrations/`.

Not built by `cargo build` (see the workspace's `default-members`) — build
with `cargo build -p layerfs-dnf` or `cargo build --workspace`.

Not yet installed over a real `/usr/bin/dnf` (Milestone 9, retrofit
installation) or tested against a real package transaction — see
ROADMAP.md's Milestone 7 notes for what's verified so far.
