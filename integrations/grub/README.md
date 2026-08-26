# GRUB integration

`layerfs-grub-entries` prints the checkpoint menu entries (Normal, Safe
Mode, System Only, Previous Update, Base Recovery) described in
ROADMAP.md section 8, given `--boot-store` and `--store` paths.

`--boot-store <path>` points at a `layerfs-storage::boot` store
(`layerctl boot-register` populates one). Each entry resolves its own
kernel/initramfs from that store — Normal/Safe/System use the newest
(HEAD) generation, Previous Update uses the one before it (UPDATE),
Base Recovery uses the original (BASE) — falling back to the next lower
tier if its own generation is missing, and skipping an entry entirely if
none resolve, rather than pointing GRUB at a kernel that doesn't exist.

`--integrations dnf,apt` bakes `layerfs.integrations=dnf,apt` into every
entry's kernel command line, re-chosen at each boot like the checkpoint
itself rather than pulled from a separate config file. Reading that
parameter and activating the corresponding adapter binaries at system
startup is Milestone 9's job — not implemented yet.

Installable directly as an executable `/etc/grub.d/` script (e.g.
`41_layerfs`): `grub2-mkconfig` runs every script in that directory and
concatenates its stdout into `grub.cfg`. It doesn't matter that this one is
a compiled binary rather than shell, only that its output is valid GRUB
configuration syntax — which `grub2-script-check`,
`scripts/qemu-grub-smoke.sh`, and `scripts/qemu-boot-artifacts-smoke.sh`
all verify, the last of those specifically proving different entries boot
different registered kernel/initramfs pairs.

Not yet wired into an actual `/etc/grub.d/` install, and boot generations
aren't yet registered automatically by a kernel-package transaction
(Milestone 9).
