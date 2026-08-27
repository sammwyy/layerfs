#!/usr/bin/env bash
# Boots a real dnf --installroot Fedora system through LayerFS's own root
# assembly and switch_root, all the way to a login prompt.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KERNEL="${LAYERFS_QEMU_KERNEL:-/boot/vmlinuz-$(uname -r)}"
OVERLAY_KO_XZ="${LAYERFS_QEMU_OVERLAY_KO:-/lib/modules/$(uname -r)/kernel/fs/overlayfs/overlay.ko.xz}"
if [[ ! -r "$KERNEL" || ! -r "$OVERLAY_KO_XZ" ]]; then
    echo "qemu-fedora-installroot-smoke: missing kernel or overlay module" >&2
    exit 1
fi

cargo build -p layerfs-init --bin layerfs-init \
    --example qemu_switch_root_preinit \
    --target x86_64-unknown-linux-musl --release

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
ROOT="$WORK/initramfs"
mkdir -p "$ROOT"/{proc,sys,dev,run,sysroot}
cp target/x86_64-unknown-linux-musl/release/examples/qemu_switch_root_preinit "$ROOT/init"
cp target/x86_64-unknown-linux-musl/release/layerfs-init "$ROOT/layerfs-init"
xz -dc "$OVERLAY_KO_XZ" > "$ROOT/overlay.ko"
(cd "$ROOT" && find . | cpio -o -H newc 2>/dev/null | gzip -9) > "$WORK/initramfs.cpio.gz"
truncate -s 3G "$WORK/store.img"

docker run --rm --privileged -v "$WORK:/output" fedora:42 bash -lc '
set -eu
dnf -qy install dnf btrfs-progs >/dev/null
INSTALLROOT=$(mktemp -d)
dnf -qy --installroot="$INSTALLROOT" --releasever=42 --use-host-config \
    --setopt=install_weak_deps=False install systemd systemd-udev passwd >/dev/null

loopdev=$(losetup --find --show /output/store.img)
cleanup() { mountpoint -q /mnt/layerfs-store && umount /mnt/layerfs-store || true; losetup -d "$loopdev" || true; }
trap cleanup EXIT
mkfs.btrfs -q "$loopdev"
mkdir -p /mnt/layerfs-store
mount "$loopdev" /mnt/layerfs-store
mkdir -p /mnt/layerfs-store/{override,data,work}
cp -a "$INSTALLROOT" /mnt/layerfs-store/base
'

ARGS=(
    -kernel "$KERNEL"
    -initrd "$WORK/initramfs.cpio.gz"
    -drive "file=$WORK/store.img,format=raw,if=virtio"
    -append "console=ttyS0 rdinit=/init layerfs.checkpoint=normal layerfs.store=/dev/vda"
    -nographic -serial mon:stdio -no-reboot -m 1024M
)
if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    ARGS+=(-enable-kvm -cpu host)
fi

OUTPUT="$(timeout 45s qemu-system-x86_64 "${ARGS[@]}" 2>&1 || true)"
grep -E 'layerfs:|Kernel panic|Reached target|login:' <<<"$OUTPUT" || true
if grep -q 'login:' <<<"$OUTPUT"; then
    echo "qemu-fedora-installroot-smoke: PASS"
else
    echo "$OUTPUT" >&2
    echo "qemu-fedora-installroot-smoke: FAIL" >&2
    exit 1
fi
