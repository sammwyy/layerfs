#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo build -p layerfs-init --bin layerfs-init --target x86_64-unknown-linux-musl --release

docker run --rm -v "$PWD:/workspace:ro" archlinux:latest bash -lc '
set -eu
export TERM=dumb
pacman -Sy --noconfirm --needed mkinitcpio >/dev/null
pacman -Sw --noconfirm linux >/dev/null
kernel_package=$(find /var/cache/pacman/pkg -name "linux-[0-9]*.pkg.tar.*" | head -n1)
mkdir /kernel
bsdtar -xf "$kernel_package" -C /kernel
version=$(basename /kernel/usr/lib/modules/*)
mkdir -p /usr/lib/modules
ln -s "/kernel/usr/lib/modules/$version" "/usr/lib/modules/$version"
depmod "$version"
install -Dm755 /workspace/integrations/mkinitcpio/install/layerfs /etc/initcpio/install/layerfs
printf "%s\n" "HOOKS=(base layerfs)" "COMPRESSION=\"zstd\"" > /tmp/mkinitcpio.conf
LAYERFS_INIT=/workspace/target/x86_64-unknown-linux-musl/release/layerfs-init mkinitcpio -c /tmp/mkinitcpio.conf -k "$version" -g /tmp/layerfs-initramfs.img
contents=$(lsinitcpio -l -n /tmp/layerfs-initramfs.img)
printf "%s\n" "$contents" | grep -F layerfs-init
printf "%s\n" "$contents" | grep -E "overlay\.ko"
'

echo "mkinitcpio-smoke: PASS"
