# mkinitcpio integration

Install `install/layerfs` under `/etc/initcpio/install/`, then add `layerfs`
to `HOOKS` in `mkinitcpio.conf`. Regenerate the image with
`LAYERFS_INIT=/path/to/layerfs-init mkinitcpio -P`.

Configure the boot loader to pass `rdinit=/sbin/layerfs-init` and a
`layerfs.store=<device-spec>` (a path, or `UUID=`/`LABEL=`), plus
`layerfs.subvol=<name>` or `layerfs.luks=<device-spec>` as needed — the same
options the dracut integration documents. LUKS support needs `cryptsetup`
embedded in the image too.
