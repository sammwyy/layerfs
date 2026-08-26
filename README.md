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
  layerfs-init          early-userspace root assembly binary (initramfs)
  layerctl               administrative CLI

integrations/
  dracut, grub, dnf, apt, pacman     distro/init-system glue, not yet implemented

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
