#!/usr/bin/env bash
# Boots the real kernel + a throwaway initramfs running
# `layerfs-init`'s qemu_smoke_init example under QEMU, to verify OverlayFS
# root assembly and DATA mounting against an actual kernel boot rather than
# an unprivileged user namespace. Never touches the host filesystem beyond
# a temp directory; the guest kernel image is read-only copied from /boot.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KERNEL="${LAYERFS_QEMU_KERNEL:-/boot/vmlinuz-$(uname -r)}"
if [[ ! -r "$KERNEL" ]]; then
    echo "qemu-smoke: cannot read kernel at $KERNEL (set LAYERFS_QEMU_KERNEL)" >&2
    exit 1
fi

OVERLAY_KO_XZ="${LAYERFS_QEMU_OVERLAY_KO:-/lib/modules/$(uname -r)/kernel/fs/overlayfs/overlay.ko.xz}"
if [[ ! -r "$OVERLAY_KO_XZ" ]]; then
    echo "qemu-smoke: cannot read overlay.ko.xz at $OVERLAY_KO_XZ (set LAYERFS_QEMU_OVERLAY_KO)" >&2
    exit 1
fi

echo "qemu-smoke: building qemu_smoke_init (musl, release)"
cargo build -p layerfs-init --example qemu_smoke_init \
    --target x86_64-unknown-linux-musl --release

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

INITRAMFS_ROOT="$WORK/initramfs"
mkdir -p "$INITRAMFS_ROOT"/{proc,sys,dev,run}
mkdir -p "$INITRAMFS_ROOT"/store/{base,override,data/home,work}

cp target/x86_64-unknown-linux-musl/release/examples/qemu_smoke_init "$INITRAMFS_ROOT/init"
chmod +x "$INITRAMFS_ROOT/init"

# This throwaway initramfs has no module dependency resolution (that's a
# dracut job in a real integration), so ship the decompressed module the
# init binary loads by hand before mounting an overlay.
xz -dc "$OVERLAY_KO_XZ" > "$INITRAMFS_ROOT/overlay.ko"

# BASE: two files, one of which OVERRIDE will shadow and one it will delete.
echo -n "base-a" > "$INITRAMFS_ROOT/store/base/a.txt"
echo -n "base-b" > "$INITRAMFS_ROOT/store/base/b.txt"

# OVERRIDE: shadow a.txt, delete b.txt via an OverlayFS whiteout.
echo -n "modified" > "$INITRAMFS_ROOT/store/override/a.txt"
mknod "$INITRAMFS_ROOT/store/override/b.txt" c 0 0

# DATA: persistent content the assembled root should expose under /home.
echo -n "persisted" > "$INITRAMFS_ROOT/store/data/home/user.txt"

echo "qemu-smoke: packing initramfs"
(cd "$INITRAMFS_ROOT" && find . | cpio -o -H newc 2>/dev/null | gzip -9) > "$WORK/initramfs.cpio.gz"

QEMU_ARGS=(
    -kernel "$KERNEL"
    -initrd "$WORK/initramfs.cpio.gz"
    -append "console=ttyS0 rdinit=/init layerfs.checkpoint=normal layerfs.store=/store"
    -nographic
    -serial mon:stdio
    -no-reboot
    -m 256M
)

if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    QEMU_ARGS+=(-enable-kvm -cpu host)
fi

echo "qemu-smoke: booting"
OUTPUT="$(timeout 60s qemu-system-x86_64 "${QEMU_ARGS[@]}" 2>&1 || true)"
echo "$OUTPUT"

if echo "$OUTPUT" | grep -q "QEMU-SMOKE: PASS"; then
    echo "qemu-smoke: PASS"
    exit 0
else
    echo "qemu-smoke: FAIL (no PASS marker found)" >&2
    exit 1
fi
