# initramfs-tools integration

Install `hooks/layerfs` under `/etc/initramfs-tools/hooks/`, then regenerate
the initramfs with `LAYERFS_INIT=/path/to/layerfs-init update-initramfs -u`.
The hook embeds `layerfs-init`, OverlayFS, and Btrfs support.

Configure the boot loader to pass `rdinit=/sbin/layerfs-init` and an explicit
`layerfs.store=/dev/...` device. The device still has to be available before
the initramfs starts; LUKS activation and automatic store discovery remain
separate work.
