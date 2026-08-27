#!/usr/bin/env bash
# Boots a real Debian system through the actual initramfs-tools hook
# (not a hand-built initramfs) all the way to a login prompt.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KERNEL_ARG="${LAYERFS_QEMU_KERNEL:-}"

cargo build -p layerfs-init --bin layerfs-init --target x86_64-unknown-linux-musl --release

WORK="$(mktemp -d)"
cleanup() {
    docker run --rm --privileged -v "$WORK:/output" rust:latest rm -rf /output >/dev/null 2>&1 || true
    rm -rf "$WORK" || true
}
trap cleanup EXIT

docker run --rm --privileged -v "$WORK:/output" -v "$PWD:/workspace:ro" debian:bookworm bash -lc '
set -eu
apt-get update -qq
apt-get install -y -qq debootstrap initramfs-tools-core kmod btrfs-progs >/dev/null

kernel=$(apt-cache depends --important linux-image-amd64 | awk "/Depends:/ { print \$2; exit }")
apt-get download "$kernel" >/dev/null
mkdir /kernel
dpkg-deb -x "$kernel"_*.deb /kernel
version=$(basename /kernel/lib/modules/*)
mkdir -p /lib/modules
ln -s "/kernel/lib/modules/$version" "/lib/modules/$version"
ln -s "/kernel/boot/config-$version" "/boot/config-$version"
cp "/kernel/boot/vmlinuz-$version" /output/vmlinuz

install -Dm755 /workspace/integrations/initramfs-tools/hooks/layerfs /etc/initramfs-tools/hooks/layerfs
LAYERFS_INIT=/workspace/target/x86_64-unknown-linux-musl/release/layerfs-init \
    mkinitramfs -o /output/initramfs.img "$version"

INSTALLROOT=/root/installroot
debootstrap --variant=minbase bookworm "$INSTALLROOT" >/dev/null
chroot "$INSTALLROOT" apt-get install -y -qq systemd systemd-sysv udev passwd >/dev/null 2>&1 || \
    DEBIAN_FRONTEND=noninteractive chroot "$INSTALLROOT" apt-get install -y -qq systemd systemd-sysv udev passwd >/dev/null

truncate -s 3G /output/store.img
loopdev=$(losetup --find --show /output/store.img)
cleanup() { mountpoint -q /mnt/layerfs-store && umount /mnt/layerfs-store || true; losetup -d "$loopdev" || true; }
trap cleanup EXIT
mkfs.btrfs -q "$loopdev"
mkdir -p /mnt/layerfs-store
mount "$loopdev" /mnt/layerfs-store
mkdir -p /mnt/layerfs-store/{override,data,work}
cp -a "$INSTALLROOT" /mnt/layerfs-store/base
chmod 666 /output/vmlinuz /output/initramfs.img /output/store.img
'

KERNEL="${KERNEL_ARG:-$WORK/vmlinuz}"

ARGS=(
    -kernel "$KERNEL"
    -initrd "$WORK/initramfs.img"
    -drive "file=$WORK/store.img,format=raw,if=virtio"
    -append "console=ttyS0 rdinit=/sbin/layerfs-init layerfs.checkpoint=normal layerfs.store=/dev/vda"
    -nographic -serial mon:stdio -no-reboot -m 1024M
)
if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    ARGS+=(-enable-kvm -cpu host)
fi

OUTPUT="$(timeout 60s qemu-system-x86_64 "${ARGS[@]}" 2>&1 || true)"
grep -E 'layerfs:|Kernel panic|Reached target|login:' <<<"$OUTPUT" || true
if grep -q 'login:' <<<"$OUTPUT"; then
    echo "qemu-initramfs-tools-smoke: PASS"
else
    echo "$OUTPUT" >&2
    echo "qemu-initramfs-tools-smoke: FAIL" >&2
    exit 1
fi
