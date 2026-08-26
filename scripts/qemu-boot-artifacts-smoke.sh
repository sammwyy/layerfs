#!/usr/bin/env bash
# Boots the Normal and Base Recovery GRUB entries and checks each printed
# its own registered boot generation's artifact name.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

KERNEL="${LAYERFS_QEMU_KERNEL:-/boot/vmlinuz-$(uname -r)}"
if [[ ! -r "$KERNEL" ]]; then
    echo "qemu-boot-artifacts-smoke: cannot read kernel at $KERNEL" >&2
    exit 1
fi

echo "qemu-boot-artifacts-smoke: building layerfs-grub-entries, layerctl, and artifact_marker_init"
cargo build -p layerfs-grub -p layerctl --release
cargo build -p layerfs-init --example artifact_marker_init \
    --target x86_64-unknown-linux-musl --release

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

STORE="$WORK/store"
mkdir -p "$STORE/base"

pack_initramfs() {
    local artifact_name="$1" out="$2"
    local root="$WORK/initramfs-root-$artifact_name"
    mkdir -p "$root"
    cp target/x86_64-unknown-linux-musl/release/examples/artifact_marker_init "$root/init"
    chmod +x "$root/init"
    echo -n "$artifact_name" > "$root/artifact-name"
    (cd "$root" && find . | cpio -o -H newc 2>/dev/null | gzip -9) > "$out"
}

pack_initramfs "head" "$WORK/initramfs-head.img"
pack_initramfs "base" "$WORK/initramfs-base.img"

target/release/layerctl --store "$STORE" boot-register base \
    --kernel "$KERNEL" --initramfs "$WORK/initramfs-base.img"
target/release/layerctl --store "$STORE" boot-register head \
    --kernel "$KERNEL" --initramfs "$WORK/initramfs-head.img"

ISOROOT="$WORK/isoroot"
mkdir -p "$ISOROOT/boot/grub"
cp -rL "$STORE/boot" "$ISOROOT/boot-store"

render_cfg() {
    local default_index="$1"
    {
        echo "set default=$default_index"
        echo "set timeout=1"
        echo 'terminal_output console'
        target/release/layerfs-grub-entries \
            --boot-store "$STORE/boot" \
            --store "$STORE" \
            --extra-cmdline "console=ttyS0" \
        | sed "s|$STORE/boot|/boot-store|g"
    } > "$ISOROOT/boot/grub/grub.cfg"
}

run_case() {
    local label="$1" default_index="$2" expected="$3"
    render_cfg "$default_index"

    grub2-script-check "$ISOROOT/boot/grub/grub.cfg"
    grub2-mkrescue -o "$WORK/test.iso" "$ISOROOT" >/dev/null 2>&1

    QEMU_ARGS=(-cdrom "$WORK/test.iso" -boot d -nographic -serial mon:stdio -no-reboot -m 512M)
    if [[ -r /dev/kvm && -w /dev/kvm ]]; then
        QEMU_ARGS+=(-enable-kvm -cpu host)
    fi

    echo "qemu-boot-artifacts-smoke: booting '$label', expecting ARTIFACT=$expected"
    OUTPUT="$(timeout 90s qemu-system-x86_64 "${QEMU_ARGS[@]}" 2>&1 || true)"
    echo "$OUTPUT" | tail -10

    if echo "$OUTPUT" | grep -q "ARTIFACT=$expected"; then
        echo "qemu-boot-artifacts-smoke: $label PASS"
    else
        echo "qemu-boot-artifacts-smoke: $label FAIL (expected ARTIFACT=$expected)" >&2
        exit 1
    fi
}

# entry indices in the fixed order layerfs-grub-entries emits them
run_case "Fedora Linux (normal, should boot HEAD)" 0 "head"
run_case "Fedora Linux — Base Recovery (should boot BASE)" 4 "base"

echo "qemu-boot-artifacts-smoke: PASS"
