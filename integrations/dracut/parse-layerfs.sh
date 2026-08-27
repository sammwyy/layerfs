#!/usr/bin/sh
# Blocks the initqueue until the store device layerfs.store= names actually
# shows up (UUID=/LABEL=/PARTUUID= resolved via the same udev symlinks
# layerfs-init itself resolves at runtime), the same way rootfs-block's
# parse-block.sh waits for root=. Without this, layerfs-init can run before
# udev has settled and fail with a spurious "no such device".

store=$(getarg layerfs.store=)

case "$store" in
    UUID=* | LABEL=* | PARTUUID=*)
        wait_for_dev "$(label_uuid_to_dev "$store")"
        ;;
    /dev/*)
        wait_for_dev "$store"
        ;;
esac
