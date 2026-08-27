#!/usr/bin/env bash
# Confirms layerfs.subvol= selects a non-default Btrfs subvolume: the same
# device without --subvol has no base there and correctly fails.
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
