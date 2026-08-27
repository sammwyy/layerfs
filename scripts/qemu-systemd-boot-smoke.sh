#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KERNEL="${LAYERFS_QEMU_KERNEL:-/boot/vmlinuz-$(uname -r)}"
OVERLAY_KO_XZ="${LAYERFS_QEMU_OVERLAY_KO:-/lib/modules/$(uname -r)/kernel/fs/overlayfs/overlay.ko.xz}"
OVMF_CODE="${LAYERFS_OVMF_CODE:-/usr/share/OVMF/OVMF_CODE.fd}"
OVMF_VARS="${LAYERFS_OVMF_VARS:-/usr/share/OVMF/OVMF_VARS.fd}"
for file in "$KERNEL" "$OVERLAY_KO_XZ" "$OVMF_CODE" "$OVMF_VARS"; do
    [[ -r "$file" ]] || { echo "qemu-systemd-boot-smoke: missing $file" >&2; exit 1; }
done

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
(cd "$ROOT" && find . | cpio -o -H newc 2>/dev/null | gzip -9) > "$WORK/initramfs.cpio.gz"

truncate -s 128M "$WORK/store.img"
docker run --rm --privileged -v "$WORK:/output" -v "$PWD:/workspace:ro" rust:latest bash -lc '
set -eu
apt-get update -qq
apt-get install -y -qq btrfs-progs >/dev/null
loopdev=$(losetup --find --show /output/store.img)
cleanup() { mountpoint -q /mnt/store && umount /mnt/store || true; losetup -d "$loopdev" || true; }
trap cleanup EXIT
mkfs.btrfs -q "$loopdev"
mkdir -p /mnt/store
mount "$loopdev" /mnt/store
mkdir -p /mnt/store/{base/{sbin,dev,proc,run,sys},override,data,work}
cp /workspace/target/x86_64-unknown-linux-musl/release/examples/switched_root_init /mnt/store/base/sbin/init
printf "override\n" > /mnt/store/override/handoff-marker
'

docker run --rm -v "$WORK:/output" archlinux:latest bash -lc '
set -eu
pacman -Sy --noconfirm --needed systemd >/dev/null
cp /usr/lib/systemd/boot/efi/systemd-bootx64.efi /output/BOOTX64.EFI
'

truncate -s 64M "$WORK/esp.img"
mkfs.fat "$WORK/esp.img" >/dev/null
mmd -i "$WORK/esp.img" ::/EFI ::/EFI/BOOT ::/loader ::/loader/entries
mcopy -i "$WORK/esp.img" "$WORK/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI
mcopy -i "$WORK/esp.img" "$KERNEL" ::/vmlinuz
mcopy -i "$WORK/esp.img" "$WORK/initramfs.cpio.gz" ::/initramfs.img
printf 'default layerfs-normal.conf\ntimeout 0\n' > "$WORK/loader.conf"
printf 'title LayerFS Linux\nlinux /vmlinuz\ninitrd /initramfs.img\noptions console=ttyS0 rdinit=/init layerfs.checkpoint=normal layerfs.store=/dev/vdb\n' > "$WORK/layerfs-normal.conf"
mcopy -i "$WORK/esp.img" "$WORK/loader.conf" ::/loader/loader.conf
mcopy -i "$WORK/esp.img" "$WORK/layerfs-normal.conf" ::/loader/entries/layerfs-normal.conf
cp "$OVMF_VARS" "$WORK/OVMF_VARS.fd"

ARGS=(
    -drive "if=pflash,format=raw,readonly=on,file=$OVMF_CODE"
    -drive "if=pflash,format=raw,file=$WORK/OVMF_VARS.fd"
    -drive "file=$WORK/esp.img,format=raw,if=virtio"
    -drive "file=$WORK/store.img,format=raw,if=virtio"
    -nographic -serial mon:stdio -no-reboot -m 512M
)
[[ -r /dev/kvm && -w /dev/kvm ]] && ARGS+=(-enable-kvm -cpu host)
OUTPUT="$(timeout 90s qemu-system-x86_64 "${ARGS[@]}" 2>&1 || true)"
grep -E 'layerfs:|QEMU-SWITCH-ROOT|Kernel panic|qemu-system' <<<"$OUTPUT" || true
grep -q 'QEMU-SWITCH-ROOT: PASS' <<<"$OUTPUT" || { echo "qemu-systemd-boot-smoke: FAIL" >&2; exit 1; }
echo "qemu-systemd-boot-smoke: PASS"
