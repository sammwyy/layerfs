# mkinitcpio integration

Install `install/layerfs` under `/etc/initcpio/install/`, then add `layerfs`
to `HOOKS` in `mkinitcpio.conf`. Regenerate the image with
`LAYERFS_INIT=/path/to/layerfs-init mkinitcpio -P`.

Configure the boot loader to pass `rdinit=/sbin/layerfs-init` and an explicit
`layerfs.store=/dev/...` device. Device availability, encryption, and automatic
store discovery remain separate work.
