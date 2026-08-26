# GRUB integration

`layerfs-grub-entries` prints the five hardcoded checkpoint menu entries
(Normal, Safe Mode, System Only, Previous Update, Base Recovery) described
in ROADMAP.md section 8, given `--linux`, `--initrd`, and `--store` paths.

Installable directly as an executable `/etc/grub.d/` script (e.g.
`41_layerfs`): `grub2-mkconfig` runs every script in that directory and
concatenates its stdout into `grub.cfg`. It doesn't matter that this one is
a compiled binary rather than shell, only that its output is valid GRUB
configuration syntax — which `grub2-script-check` and
`scripts/qemu-grub-smoke.sh` both verify.

Not yet wired into an actual `/etc/grub.d/` install (Milestone 9, retrofit
installation) or into boot artifact tracking (Milestone 8) — `--linux` and
`--initrd` are passed through verbatim rather than discovered.
