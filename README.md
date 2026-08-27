# LayerFS

Transactional, layered, recoverable root filesystem architecture for Linux.

LayerFS lets root modify a running system normally — edit config, remove
binaries, overwrite libraries — while keeping every mutation redirected into
layered storage that can be inspected, reset, or rolled back. It is not an
immutable distribution: there is no sandbox, no confirmation prompt, no
special syntax for `sudo rm`. The layering is invisible until you need it —
recovering from a broken update, booting a known-good checkpoint, or
inspecting what changed.

Status: early skeleton. Core semantics and CLI scaffolding exist; root
assembly, the transaction engine, and package-manager adapters are not
implemented yet. See [ROADMAP.md](ROADMAP.md) for the full design document
and milestone-by-milestone progress — treat it as working notes, not a
frozen spec; where the code takes a different path than a section
describes, the code is authoritative and the divergence should be
documented in commit messages or comments.

## Architecture

```
OVERRIDE       RW, ordinary root mutations (upperdir)
UPDATE_HEAD    RO, most recent system transaction
UPDATE         RO, consolidated prior transactions
BASE           RO, original known-good system
```

These compose into an OverlayFS mount, highest layer wins. Four hardcoded
boot checkpoints select a prefix of this stack:

| checkpoint | layers                          | data | override |
|-----------:|----------------------------------|:----:|:--------:|
| `base`     | BASE                              |  no  |    no    |
| `system`   | UPDATE_HEAD > UPDATE > BASE       |  no  |    no    |
| `safe`     | UPDATE_HEAD > UPDATE > BASE       | yes  |    no    |
| `normal`   | OVERRIDE > UPDATE_HEAD > UPDATE > BASE | yes | yes |

`layerfs.head=off` drops UPDATE_HEAD from any checkpoint, giving exactly one
system-update rollback without unbounded snapshot history.

## Workspace layout

```
crates/
  layerfs-core         checkpoint/layer/state semantics, no I/O
  layerfs-storage       StorageBackend trait, Btrfs and directory backends
  layerfs-transaction    staging, locking, atomic commit
  layerfs-adapter        shared package-manager adapter runner (classify → passthrough or transaction)
  layerfs-init          early-userspace root assembly binary (initramfs)
  layerctl               administrative CLI

integrations/
  grub                   layerfs-grub-entries: generates the checkpoint GRUB menu entries
  dnf                    layerfs-dnf: dnf verb classification, built on layerfs-adapter
  dracut, apt, pacman     distro/init-system glue, not yet implemented

tests/
  integration    cross-crate filesystem tests
  qemu           boot-level integration tests

xtask/           developer task runner (musl builds, etc.)
```

## Building

```bash
cargo build
cargo test
```

Distro/package-manager integrations (`integrations/grub`, `integrations/dnf`,
...) are separate crates excluded from the workspace's `default-members`, so
the commands above stay distro-agnostic. Build one explicitly, or build
everything with `--workspace`:

```bash
cargo build -p layerfs-dnf
cargo build --workspace
```

`layerfs-init` is intended for static musl builds:

```bash
cargo run -p xtask -- init-musl
```

OverlayFS assembly needs `CAP_SYS_ADMIN` to mount anything, so it isn't
covered by `cargo test`. Verify it against a real kernel mount inside an
unprivileged user+mount namespace instead — never against the host root:

```bash
cargo build -p layerfs-init --example overlay_smoke
unshare --map-root-user --mount -- \
    ./target/debug/examples/overlay_smoke /tmp/some-scratch-dir
```

For a stronger check — a real kernel, not a namespace — `scripts/qemu-smoke.sh`
boots the host's own kernel under QEMU/KVM with `layerfs-init`'s mount logic
running as `rdinit=/init` in a throwaway initramfs, then powers off. Needs
`qemu-system-x86_64`, a readable `/boot/vmlinuz-*`, and `overlay.ko(.xz)`
under `/lib/modules/$(uname -r)`; nothing on the host is modified.

```bash
./scripts/qemu-smoke.sh
```

`scripts/qemu-grub-smoke.sh` goes one step further: it generates a real
`grub.cfg` with `layerfs-grub-entries`, checks it with `grub2-script-check`,
builds an actual bootable ISO (`grub2-mkrescue`), and boots it under
QEMU/KVM twice — once selecting the Normal entry, once selecting Base
Recovery — proving GRUB itself renders the menu and passes the right
`layerfs.checkpoint=` through to the kernel. Needs `grub2-mkrescue` and
`grub2-script-check` in addition to the tools above.

```bash
./scripts/qemu-grub-smoke.sh
```

`scripts/qemu-boot-artifacts-smoke.sh` proves boot artifact selection
specifically: registers the real host kernel under two distinguishable
initramfs images as the BASE and HEAD boot generations, generates entries
from that store, and boots both the Normal and Base Recovery entries under
QEMU/KVM, checking each loaded the initramfs its own tier actually
registered — not just that the generated paths look plausible.

```bash
./scripts/qemu-boot-artifacts-smoke.sh
```

`layerctl transaction -- <program> [args...]` drives the real transaction
engine (staging, a private mount namespace, chrooted execution, validation,
atomic commit) for development, in place of a package-manager adapter. Like
OverlayFS assembly, it needs `CAP_SYS_ADMIN`/`CAP_SYS_CHROOT`:

```bash
unshare --map-root-user --mount -- \
    ./target/debug/layerctl --store /path/to/a/store transaction -- /bin/some-static-binary
```

`layerctl rollback update` discards the active UPDATE_HEAD, the one-step
rollback the design allows (see "One-update rollback" above) — the prior
UPDATE_HEAD was already squashed into UPDATE by the transaction that
superseded it, so UPDATE alone is enough to boot from. Refuses if there's
no UPDATE_HEAD to discard:

```bash
./target/debug/layerctl --store /path/to/a/store rollback update
```

`layerctl install --source <dir>` converts a static (not live) source tree
into `base`/`override`/`data`, extracting `home`/`root`/`srv` out into
`data`. Not the full live-migration flow (that needs a reboot into a
dedicated dracut initramfs, not implemented yet). `--integrations dnf,apt`
symlinks each present real package-manager binary (e.g. `usr/bin/dnf`) to
its adapter (`layerfs-dnf`) inside the new base, matching what GRUB bakes
into `layerfs.integrations=` on the boot entry (see `integrations/grub`):

```bash
./target/debug/layerctl --store /path/to/a/store install --source /path/to/a/rootfs --integrations dnf
```

A committed transaction only becomes the *booted* root on the next reboot
— the running system's mounted `/` can't be reconfigured under it. But a
committed update can be applied to the running system without a reboot:
`layerctl apply-now [--live-root <path>]` snapshots the affected subtree,
layers the store's UPDATE_HEAD/UPDATE on top, and atomically swaps it in
via `mount --move`. This is scoped to `/usr` and `/opt` only — never `/`
itself — since a whole-root swap could orphan mounts nested under paths
like `/proc` or `/home` that a non-recursive snapshot bind mount wouldn't
capture. Already-open files on already-running processes keep their old
content (normal Unix replace-while-open behavior); new opens see the
change immediately. `dnf`/`apt` adapters call this automatically after a
commit, but only when the update stays within `usr`/`opt` and didn't touch
a shared library, `/boot`, or systemd itself — otherwise they report that
a reboot is required rather than risk leaving running processes on a stale
version of something they already loaded, or orphaning a mount:

```bash
unshare --map-root-user --mount -- \
    ./target/debug/layerctl --store /path/to/a/store apply-now
```

## Initial target

Fedora, Btrfs, GRUB, dracut, DNF, x86_64, UEFI. Other distributions and
backends come after this configuration is proven; see ROADMAP.md sections
32 and 41 for the staged milestone plan.

## Non-goals

Not a kernel filesystem, not a package manager, not a container runtime,
not an immutable distribution, not a security boundary against a hostile
root user. LayerFS protects against accidental or logical system damage,
not deliberate bypass by someone with root and raw block-device access.

## License

MIT. See [LICENSE](LICENSE).
