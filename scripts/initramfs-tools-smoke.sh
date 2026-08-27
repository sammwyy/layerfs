#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo build -p layerfs-init --bin layerfs-init --target x86_64-unknown-linux-musl --release

docker run --rm -v "$PWD:/workspace:ro" debian:bookworm bash -lc '
set -eu
apt-get update -qq
apt-get install -y -qq initramfs-tools-core kmod >/dev/null
kernel=$(apt-cache depends --important linux-image-amd64 | awk "/Depends:/ { print \$2; exit }")
apt-get download "$kernel" >/dev/null
install -Dm755 /workspace/integrations/initramfs-tools/hooks/layerfs /etc/initramfs-tools/hooks/layerfs
mkdir /kernel
dpkg-deb -x "$kernel"_*.deb /kernel
version=$(basename /kernel/lib/modules/*)
mkdir -p /lib/modules
ln -s "/kernel/lib/modules/$version" "/lib/modules/$version"
ln -s "/kernel/boot/config-$version" "/boot/config-$version"
LAYERFS_INIT=/workspace/target/x86_64-unknown-linux-musl/release/layerfs-init mkinitramfs -o /tmp/layerfs-initramfs.img "$version"
lsinitramfs /tmp/layerfs-initramfs.img | grep -Fx usr/sbin/layerfs-init
lsinitramfs /tmp/layerfs-initramfs.img | grep -E "overlay\.ko|btrfs\.ko"
'

echo "initramfs-tools-smoke: PASS"
