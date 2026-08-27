#!/usr/bin/env bash
# Creates a real loop-mounted Btrfs image with a non-default 'layerfs'
# subvolume (the layout a migrated system uses per section 33, keeping
# LayerFS alongside other subvolumes rather than owning the whole
# filesystem's default one) and confirms layerfs.subvol= actually selects
# it: the same device without --subvol has no base at its default subvolume
# and correctly fails, while --subvol layerfs finds it.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

docker run --rm --privileged -v "$PWD:/workspace" -w /workspace rust:latest bash -lc '
set -eu
apt-get update -qq && apt-get install -y -qq btrfs-progs >/dev/null

truncate -s 256M /tmp/store.img
loopdev=$(losetup --find --show /tmp/store.img)
mkfs.btrfs -q "$loopdev"

mkdir -p /mnt/top
mount "$loopdev" /mnt/top
btrfs subvolume create /mnt/top/layerfs >/dev/null
mkdir -p /mnt/top/layerfs/base
umount /mnt/top

echo "== without --subvol: default subvolume has no base, must fail =="
if LAYERFS_BTRFS_STORE_DEVICE="$loopdev" /usr/local/cargo/bin/cargo test -p layerfs-init --lib -- --ignored mounts_a_btrfs_device_store; then
    echo "expected failure did not occur" >&2
    exit 1
else
    echo "correctly failed"
fi

echo "== with --subvol layerfs: must find base =="
LAYERFS_BTRFS_STORE_DEVICE="$loopdev" /usr/local/cargo/bin/cargo test -p layerfs-init --lib -- --ignored mounts_a_specific_btrfs_subvolume

losetup -d "$loopdev" || true
echo "btrfs-subvol-smoke: PASS"
'
