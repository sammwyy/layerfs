# initramfs-tools integration

Install `hooks/layerfs` under `/etc/initramfs-tools/hooks/`, then regenerate
the initramfs with `LAYERFS_INIT=/path/to/layerfs-init update-initramfs -u`.
The hook embeds `layerfs-init`, OverlayFS, and Btrfs support.

Configure the boot loader to pass `rdinit=/sbin/layerfs-init` and a
`layerfs.store=<device-spec>` (a path, or `UUID=`/`LABEL=`), plus
`layerfs.subvol=<name>` or `layerfs.luks=<device-spec>` as needed — the same
options the dracut integration documents. LUKS support needs `cryptsetup`
embedded in the initramfs too.
