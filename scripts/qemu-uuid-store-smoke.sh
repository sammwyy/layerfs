#!/usr/bin/env bash
# Proves layerfs.store=UUID=... actually works under the real boot
# architecture: layerfs-init is invoked via rdinit=, which replaces the
# initramfs's own /init at the kernel level (a genuine kernel parameter,
# not a dracut convention) — so dracut's udev never runs and
# /dev/disk/by-uuid never gets created. UUID resolution has to work from
# layerfs-init's own direct Btrfs superblock scan (device_scan.rs), not
# the udev symlinks. This boots a real kernel against a Btrfs image with a
# fixed UUID and layerfs.store=UUID=<that uuid> — no /dev/disk symlinks
# exist in this environment at all, so a pass here can only mean the scan
# fallback found it.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KERNEL="${LAYERFS_QEMU_KERNEL:-/boot/vmlinuz-$(uname -r)}"
OVERLAY_KO_XZ="${LAYERFS_QEMU_OVERLAY_KO:-/lib/modules/$(uname -r)/kernel/fs/overlayfs/overlay.ko.xz}"
if [[ ! -r "$KERNEL" || ! -r "$OVERLAY_KO_XZ" ]]; then
    echo "qemu-uuid-store-smoke: missing kernel or overlay module" >&2
    exit 1
fi

STORE_UUID="11111111-2222-3333-4444-555555555555"

cargo build -p layerfs-init --bin layerfs-init \
    --example qemu_switch_root_preinit --example switched_root_init \
    --target x86_64-unknown-linux-musl --release

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
ROOT="$WORK/initramfs"
mkdir -p "$ROOT"/{proc,sys,dev,run,sysroot}

cp target/x86_64-unknown-linux-musl/release/examples/qemu_switch_root_preinit "$ROOT/init"
cp target/x86_64-unknown-linux-musl/release/layerfs-init "$ROOT/layerfs-init"
xz -dc "$OVERLAY_KO_XZ" > "$ROOT/overlay.ko"
truncate -s 128M "$WORK/store.img"

docker run --rm --privileged -v "$WORK:/output" -v "$PWD:/workspace:ro" rust:latest bash -lc "
set -eu
apt-get update -qq
apt-get install -y -qq btrfs-progs >/dev/null
loopdev=\$(losetup --find --show /output/store.img)
cleanup() {
    mountpoint -q /mnt/layerfs-store && umount /mnt/layerfs-store || true
    losetup -d \"\$loopdev\" || true
}
trap cleanup EXIT
mkfs.btrfs -q -U $STORE_UUID \"\$loopdev\"
mkdir -p /mnt/layerfs-store
mount \"\$loopdev\" /mnt/layerfs-store
mkdir -p /mnt/layerfs-store/{base/{sbin,dev,proc,run,sys},override,data,work}
cp /workspace/target/x86_64-unknown-linux-musl/release/examples/switched_root_init /mnt/layerfs-store/base/sbin/init
printf 'base\n' > /mnt/layerfs-store/base/handoff-marker
printf 'override\n' > /mnt/layerfs-store/override/handoff-marker
"

(cd "$ROOT" && find . | cpio -o -H newc 2>/dev/null | gzip -9) > "$WORK/initramfs.cpio.gz"
ARGS=(
    -kernel "$KERNEL"
    -initrd "$WORK/initramfs.cpio.gz"
    -drive "file=$WORK/store.img,format=raw,if=virtio"
    -append "console=ttyS0 rdinit=/init layerfs.checkpoint=normal layerfs.store=UUID=$STORE_UUID"
    -nographic
    -serial mon:stdio
    -no-reboot
    -m 256M
)
if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    ARGS+=(-enable-kvm -cpu host)
fi

OUTPUT="$(timeout 60s qemu-system-x86_64 "${ARGS[@]}" 2>&1 || true)"
grep -E 'layerfs:|QEMU-SWITCH-ROOT|Kernel panic|qemu-system' <<<"$OUTPUT" || true
if grep -q "QEMU-SWITCH-ROOT: PASS" <<<"$OUTPUT"; then
    echo "qemu-uuid-store-smoke: PASS"
else
    echo "qemu-uuid-store-smoke: FAIL" >&2
    exit 1
fi
