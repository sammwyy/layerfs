# GRUB integration

`layerfs-grub-entries` prints the five hardcoded checkpoint menu entries
(Normal, Safe Mode, System Only, Previous Update, Base Recovery) described
in ROADMAP.md section 8, given `--linux`, `--initrd`, and `--store` paths.

`--integrations dnf,apt` bakes `layerfs.integrations=dnf,apt` into every
entry's kernel command line, re-chosen at each boot like the checkpoint
itself rather than pulled from a separate config file. Reading that
parameter and activating the corresponding adapter binaries at system
startup is Milestone 9's job — not implemented yet.

Installable directly as an executable `/etc/grub.d/` script (e.g.
`41_layerfs`): `grub2-mkconfig` runs every script in that directory and
concatenates its stdout into `grub.cfg`. It doesn't matter that this one is
a compiled binary rather than shell, only that its output is valid GRUB
configuration syntax — which `grub2-script-check` and
`scripts/qemu-grub-smoke.sh` both verify.

Not yet wired into an actual `/etc/grub.d/` install (Milestone 9, retrofit
installation) or into boot artifact tracking (Milestone 8) — `--linux` and
`--initrd` are passed through verbatim rather than discovered.
