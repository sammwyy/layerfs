# APT adapter (layerfs-apt)

Stands in for `apt`/`apt-get` so `sudo apt install foo` transparently
becomes a LayerFS system transaction — same rationale as `layerfs-dnf`
(see its README): interception has to happen at process start. Wraps
`apt-get` by default rather than `apt`, since `apt`'s CLI is explicitly
documented as unstable for scripting.

Every argument is passed straight through to the real binary; control
comes from environment variables only:

```text
LAYERFS_APT_BIN      path to the real apt-get binary (default: "apt-get")
LAYERFS_STORE        store root (default: /run/layerfs-store)
LAYERFS_LAYERCTL_BIN path to layerctl, used to run the transaction (default: "layerctl")
LAYERFS_LIVE_ROOT    root to hot-apply safe updates to (default: "/")
```

After a successful commit, a safe update (confined to `usr`/`opt`, no
shared library/kernel/systemd changes) is applied to the running system
immediately, no reboot — see `layerctl apply-now` in the top-level README.
Anything else is committed but left for the next reboot, and says so.

`layerfs_apt::classify::is_mutating` mirrors `layerfs-dnf`'s: an allowlist
of read-only verbs (`list`, `search`, `show`, `update`, ...) runs directly;
everything else, including unrecognized verbs, is treated as mutating.
`--dry-run`/`-s`/`--just-print`/`--download-only`/`-d` override this back
to read-only regardless of verb, since none of them touch the installed set.

That classification is all this crate contributes — the passthrough exec,
transaction staging, chrooted execution, validation, and commit all live in
`layerfs-adapter`, shared with every other package-manager adapter.

Not built by `cargo build` (see the workspace's `default-members`) — build
with `cargo build -p layerfs-apt` or `cargo build --workspace`.

Verified for real end to end (`unshare --map-root-user --mount --user`, a
statically-linked musl stand-in `apt-get` chrooted into a real transaction
root): `install curl` mutates the store and hot-applies live, `curl`'s
content changing from `v1` to `v2` in a stand-in live root with no reboot.
Not yet installed over a real `/usr/bin/apt-get` (Milestone 9, retrofit
installation) or exercised against a real Debian/Ubuntu root/initramfs-tools.
