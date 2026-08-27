# dracut integration

Install this directory as `99layerfs` below dracut's `modules.d` directory and
enable it explicitly with `add_dracutmodules+=" layerfs "`.

The module installs `layerfs-init` as `/sbin/layerfs-init`, includes OverlayFS,
and emits `rdinit=/sbin/layerfs-init` for host-only command lines. Set
`LAYERFS_INIT` while building the image to select the binary to embed.

Pass `layerfs.store=/dev/...` to mount an unencrypted Btrfs store at
`/run/layerfs-store`. The surrounding dracut setup must still make the device
available; LUKS activation and Btrfs subvolume selection remain separate work.
