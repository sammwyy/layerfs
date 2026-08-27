# Pacman adapter (layerfs-pacman)

Stands in for `pacman` itself so `sudo pacman -S foo` transparently becomes
a LayerFS system transaction — same rationale as `layerfs-dnf` (see its
README): interception has to happen at process start, not via a hook that
fires after pacman is already writing to the live root.

Every argument is passed straight through to the real `pacman`; control
comes from environment variables only:

```text
LAYERFS_PACMAN_BIN   path to the real pacman binary (default: "pacman")
LAYERFS_STORE        store root (default: /run/layerfs-store)
LAYERFS_LAYERCTL_BIN path to layerctl, used to run the transaction (default: "layerctl")
LAYERFS_LIVE_ROOT    root to hot-apply safe updates to (default: "/")
```

After a successful commit, a safe update (confined to `usr`/`opt`, no
shared library/kernel/systemd changes) is applied to the running system
immediately, no reboot — see `layerctl apply-now` in the top-level README.
Anything else is committed but left for the next reboot, and says so.

`layerfs_pacman::classify::is_mutating` reads pacman's operation flag
(`-S`/`-R`/`-U`/`-Q`/`-F`/`-D`/`-T`/`-V`/`-h`, short or long, since pacman
takes an operation rather than a verb): `-Q`/`-F`/`-T`/`-V`/`-h` are always
read-only; `-R`/`-U`/`-D` are always mutating; `-S` is mutating unless
combined with an informational modifier (`-Ss`, `-Si`, `-Sl`, `-Sp`,
`-Sw`/`--downloadonly`). Unrecognized non-empty invocations default to
mutating for the same reason as `layerfs-dnf`'s unknown-verb default: a
wasted empty transaction is harmless, skipping a real one is not.

That classification is all this crate contributes — the passthrough exec,
transaction staging, chrooted execution, validation, and commit all live in
`layerfs-adapter`, shared with every other package-manager adapter.

Not built by `cargo build` (see the workspace's `default-members`) — build
with `cargo build -p layerfs-pacman` or `cargo build --workspace`.

Verified for real end to end (`unshare --map-root-user --mount --user`,
a statically-linked musl stand-in `pacman` chrooted into a real
transaction root): `-S vim` mutates the store and hot-applies live,
`vim`'s content changing from `v1` to `v2` in a stand-in live root with no
reboot. Not yet installed over a real `/usr/bin/pacman` (Milestone 9,
retrofit installation) or exercised against a real Arch root/mkinitcpio.
