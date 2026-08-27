#!/usr/bin/env bash
# Boots the migration initramfs against disposable loop-mounted images
# (never a host device) and verifies the resulting store for real.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KERNEL="${LAYERFS_QEMU_KERNEL:-/boot/vmlinuz-$(uname -r)}"
OVERLAY_KO_XZ="${LAYERFS_QEMU_OVERLAY_KO:-/lib/modules/$(uname -r)/kernel/fs/overlayfs/overlay.ko.xz}"
if [[ ! -r "$KERNEL" || ! -r "$OVERLAY_KO_XZ" ]]; then
    echo "qemu-migration-smoke: missing kernel or overlay module" >&2
    exit 1
fi

cargo build --release -p layerfs-init --bin layerfs-init \
    --example qemu_switch_root_preinit --target x86_64-unknown-linux-musl
cargo build --release -p layerctl --target x86_64-unknown-linux-musl

WORK="$(mktemp -d)"
cleanup() {
    docker run --rm --privileged -v "$WORK:/output" rust:latest rm -rf /output >/dev/null 2>&1 || true
    rm -rf "$WORK" || true
}
trap cleanup EXIT
ROOT="$WORK/initramfs"
mkdir -p "$ROOT"/{proc,sys,dev,run,sysroot,usr/bin,mnt}
cp target/x86_64-unknown-linux-musl/release/examples/qemu_switch_root_preinit "$ROOT/init"
cp target/x86_64-unknown-linux-musl/release/layerfs-init "$ROOT/layerfs-init"
cp target/x86_64-unknown-linux-musl/release/layerctl "$ROOT/usr/bin/layerctl"
xz -dc "$OVERLAY_KO_XZ" > "$ROOT/overlay.ko"

docker run --rm --privileged -v "$WORK:/output" rust:latest bash -lc '
set -eu
apt-get update -qq
apt-get install -y -qq mount util-linux e2fsprogs btrfs-progs >/dev/null

ROOT=/output/initramfs
for bin in mount btrfs; do
    cp "/usr/bin/$bin" "$ROOT/usr/bin/$bin"
    for lib in $(ldd "/usr/bin/$bin" | grep -oP "/[^ ]+"); do
        mkdir -p "$ROOT$(dirname "$lib")"
        cp -n "$lib" "$ROOT$lib" 2>/dev/null || true
    done
done

truncate -s 512M /output/old-system.img
mkfs.ext4 -q /output/old-system.img
mkdir -p /mnt/old
mount -o loop /output/old-system.img /mnt/old
mkdir -p /mnt/old/usr/bin /mnt/old/etc /mnt/old/home/testuser
echo "real-old-system-marker" > /mnt/old/etc/old-system-marker
echo "user-data" > /mnt/old/home/testuser/important-file.txt
umount /mnt/old

truncate -s 512M /output/store.img
mkfs.btrfs -q /output/store.img
chmod 666 /output/old-system.img /output/store.img
'

(cd "$ROOT" && find . | cpio -o -H newc 2>/dev/null | gzip -9) > "$WORK/initramfs.cpio.gz"

ARGS=(
    -kernel "$KERNEL"
    -initrd "$WORK/initramfs.cpio.gz"
    -drive "file=$WORK/old-system.img,format=raw,if=virtio"
    -drive "file=$WORK/store.img,format=raw,if=virtio"
    -append "console=ttyS0 rdinit=/init layerfs.migrate=1 layerfs.migrate_source=/dev/vda layerfs.store=/dev/vdb"
    -nographic -serial mon:stdio -no-reboot -m 512M
)
if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    ARGS+=(-enable-kvm -cpu host)
fi

OUTPUT="$(timeout 60s qemu-system-x86_64 "${ARGS[@]}" 2>&1 || true)"
grep -E 'layerfs:|installed store|Kernel panic' <<<"$OUTPUT" || true
grep -q "migration complete; store ready" <<<"$OUTPUT" || {
    echo "$OUTPUT" >&2
    echo "qemu-migration-smoke: FAIL" >&2
    exit 1
}

docker run --rm --privileged -v "$WORK:/output" rust:latest bash -lc '
set -eu
apt-get update -qq && apt-get install -y -qq btrfs-progs >/dev/null
mkdir -p /mnt/verify
mount -o loop /output/store.img /mnt/verify
test "$(cat /mnt/verify/base/etc/old-system-marker)" = "real-old-system-marker"
test ! -e /mnt/verify/base/home
test "$(cat /mnt/verify/data/home/testuser/important-file.txt)" = "user-data"
test -d /mnt/verify/override
umount /mnt/verify
'

echo "qemu-migration-smoke: PASS"
