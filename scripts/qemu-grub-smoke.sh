#!/usr/bin/env bash
# Proves the generated grub.cfg is not just syntactically valid but
# actually boots: builds a real GRUB CD image (via grub2-mkrescue) whose
# menu entries come from `layerfs-grub-entries`, boots it under QEMU/KVM
# selecting different checkpoint entries, and lets GRUB itself chainload
# the kernel + layerfs-init initramfs used by scripts/qemu-smoke.sh.
#
# Never touches the host's own GRUB configuration or boot devices.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KERNEL="${LAYERFS_QEMU_KERNEL:-/boot/vmlinuz-$(uname -r)}"
OVERLAY_KO_XZ="${LAYERFS_QEMU_OVERLAY_KO:-/lib/modules/$(uname -r)/kernel/fs/overlayfs/overlay.ko.xz}"
for f in "$KERNEL" "$OVERLAY_KO_XZ"; do
    if [[ ! -r "$f" ]]; then
        echo "qemu-grub-smoke: cannot read $f" >&2
        exit 1
    fi
done

echo "qemu-grub-smoke: building layerctl, layerfs-grub-entries, and qemu_smoke_init"
cargo build -p layerctl -p layerfs-grub --release
cargo build -p layerfs-init --example qemu_smoke_init \
    --target x86_64-unknown-linux-musl --release

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

INITRAMFS_ROOT="$WORK/initramfs"
mkdir -p "$INITRAMFS_ROOT"/{proc,sys,dev,run}
mkdir -p "$INITRAMFS_ROOT"/store/{base,override,data/home,work}

cp target/x86_64-unknown-linux-musl/release/examples/qemu_smoke_init "$INITRAMFS_ROOT/init"
chmod +x "$INITRAMFS_ROOT/init"
xz -dc "$OVERLAY_KO_XZ" > "$INITRAMFS_ROOT/overlay.ko"

echo -n "base-a" > "$INITRAMFS_ROOT/store/base/a.txt"
echo -n "base-b" > "$INITRAMFS_ROOT/store/base/b.txt"
echo -n "modified" > "$INITRAMFS_ROOT/store/override/a.txt"
mknod "$INITRAMFS_ROOT/store/override/b.txt" c 0 0
echo -n "persisted" > "$INITRAMFS_ROOT/store/data/home/user.txt"

(cd "$INITRAMFS_ROOT" && find . | cpio -o -H newc 2>/dev/null | gzip -9) > "$WORK/initramfs.cpio.gz"

BOOT_STORE="$WORK/boot-artifacts"
target/release/layerctl --store "$BOOT_STORE" boot-register base \
    --kernel "$KERNEL" --initramfs "$WORK/initramfs.cpio.gz"

ISOROOT="$WORK/isoroot"
mkdir -p "$ISOROOT/boot/grub"
cp -rL "$BOOT_STORE/boot" "$ISOROOT/boot-store"

NORMAL_INDEX=0
BASE_INDEX=4

render_cfg() {
    local default_index="$1"
    {
        echo "set default=$default_index"
        echo "set timeout=1"
        echo 'terminal_output console'
        target/release/layerfs-grub-entries \
            --boot-store "$BOOT_STORE/boot" \
            --store /store \
            --extra-cmdline "console=ttyS0" \
        | sed "s|$BOOT_STORE/boot|/boot-store|g"
    } > "$ISOROOT/boot/grub/grub.cfg"
}

run_case() {
    local label="$1" default_index="$2"
    render_cfg "$default_index"

    echo "qemu-grub-smoke: checking generated grub.cfg syntax"
    grub2-script-check "$ISOROOT/boot/grub/grub.cfg"

    echo "qemu-grub-smoke: building ISO for '$label'"
    grub2-mkrescue -o "$WORK/test.iso" "$ISOROOT" >/dev/null 2>&1

    QEMU_ARGS=(
        -cdrom "$WORK/test.iso"
        -boot d
        -nographic
        -serial mon:stdio
        -no-reboot
        -m 512M
    )
    if [[ -r /dev/kvm && -w /dev/kvm ]]; then
        QEMU_ARGS+=(-enable-kvm -cpu host)
    fi

    echo "qemu-grub-smoke: booting '$label' through real GRUB"
    OUTPUT="$(timeout 90s qemu-system-x86_64 "${QEMU_ARGS[@]}" 2>&1 || true)"
    echo "$OUTPUT" | tail -20

    if echo "$OUTPUT" | grep -q "QEMU-SMOKE: PASS"; then
        echo "qemu-grub-smoke: $label PASS"
    else
        echo "qemu-grub-smoke: $label FAIL" >&2
        exit 1
    fi
}

run_case "Fedora Linux (normal)" "$NORMAL_INDEX"
run_case "Fedora Linux — Base Recovery" "$BASE_INDEX"

echo "qemu-grub-smoke: PASS"
