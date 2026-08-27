#!/usr/bin/env bash
# Boots a real Arch system through the actual mkinitcpio hook
# (not a hand-built initramfs) all the way to a login prompt.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo build -p layerfs-init --bin layerfs-init --target x86_64-unknown-linux-musl --release

WORK="$(mktemp -d)"
cleanup() {
    docker run --rm --privileged -v "$WORK:/output" rust:latest rm -rf /output >/dev/null 2>&1 || true
    rm -rf "$WORK" || true
}
trap cleanup EXIT

docker run --rm --privileged -v "$WORK:/output" -v "$PWD:/workspace:ro" archlinux:latest bash -lc '
set -eu
export TERM=dumb
pacman -Sy --noconfirm --needed mkinitcpio arch-install-scripts btrfs-progs >/dev/null
pacman -Sw --noconfirm linux >/dev/null
kernel_package=$(find /var/cache/pacman/pkg -name "linux-[0-9]*.pkg.tar.*" | head -n1)
mkdir /kernel
bsdtar -xf "$kernel_package" -C /kernel
version=$(basename /kernel/usr/lib/modules/*)
mkdir -p /usr/lib/modules
ln -s "/kernel/usr/lib/modules/$version" "/usr/lib/modules/$version"
depmod "$version"
cp "/kernel/usr/lib/modules/$version/vmlinuz" /output/vmlinuz

install -Dm755 /workspace/integrations/mkinitcpio/install/layerfs /etc/initcpio/install/layerfs
printf "%s\n" "HOOKS=(base layerfs)" "COMPRESSION=\"zstd\"" > /tmp/mkinitcpio.conf
LAYERFS_INIT=/workspace/target/x86_64-unknown-linux-musl/release/layerfs-init \
    mkinitcpio -c /tmp/mkinitcpio.conf -k "$version" -g /output/initramfs.img

pacman-key --init
pacman-key --populate archlinux
INSTALLROOT=/root/installroot
mkdir -p "$INSTALLROOT"
pacstrap -c -M "$INSTALLROOT" base systemd >/dev/null

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

ARGS=(
    -kernel "$WORK/vmlinuz"
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
    echo "qemu-mkinitcpio-smoke: PASS"
else
    echo "$OUTPUT" >&2
    echo "qemu-mkinitcpio-smoke: FAIL" >&2
    exit 1
fi
