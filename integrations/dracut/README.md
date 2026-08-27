# dracut integration

Install this directory as `99layerfs` below dracut's `modules.d` directory and
enable it explicitly with `add_dracutmodules+=" layerfs "`.

The module installs `layerfs-init` as `/sbin/layerfs-init`, includes OverlayFS,
and emits `rdinit=/sbin/layerfs-init` for host-only command lines. Set
`LAYERFS_INIT` while building the image to select the binary to embed.

The backing store must already be mounted by the surrounding dracut setup.
Mounting a device, LUKS volume, or Btrfs subvolume remains separate work.
