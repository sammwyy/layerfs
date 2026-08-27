# dracut integration

Install this directory as `99layerfs` below dracut's `modules.d` directory and
enable it explicitly with `add_dracutmodules+=" layerfs "`.

The module installs `layerfs-init` as `/sbin/layerfs-init`, includes OverlayFS,
and emits `rdinit=/sbin/layerfs-init` for host-only command lines. Set
`LAYERFS_INIT` while building the image to select the binary to embed.

Pass `layerfs.store=<device-spec>` (a path, or `UUID=`/`LABEL=`) to mount a
Btrfs store at `/run/layerfs-store`, `layerfs.subvol=<name>` to select a
non-default subvolume, and `layerfs.luks=<device-spec>` (with an optional
`layerfs.luks_key=<path>`, else `cryptsetup` prompts interactively) to unlock
an encrypted store device first. LUKS support needs `cryptsetup` and
`dm-crypt` in the image: also `--add crypt` when building it.
