#!/usr/bin/env bash
# Runs a real `dnf install` through layerfs-dnf inside a privileged Fedora
# container: a minimal base is bootstrapped with dnf's own --installroot
# (so this doesn't try to copy /proc, /sys, ... like a live root would),
# then layerfs-dnf drives the real dnf binary through an actual transaction.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo build --release -p layerctl -p layerfs-dnf

docker run --rm --privileged --network bridge \
    -v "$PWD:/workspace" -w /workspace fedora:42 bash -lc '
set -euo pipefail
dnf -qy install dnf coreutils findutils >/dev/null

STORE=$(mktemp -d)
mount -t tmpfs tmpfs "$STORE"

INSTALLROOT=$(mktemp -d)
mount -t tmpfs tmpfs "$INSTALLROOT"
dnf -qy --installroot="$INSTALLROOT" --releasever=42 --use-host-config \
    --setopt=install_weak_deps=False install dnf coreutils >/dev/null
cp /etc/resolv.conf "$INSTALLROOT/etc/resolv.conf"

target/release/layerctl --store "$STORE" install --source "$INSTALLROOT" >/dev/null

LAYERFS_STORE="$STORE" \
LAYERFS_DNF_BIN=/usr/bin/dnf \
LAYERFS_LAYERCTL_BIN="$PWD/target/release/layerctl" \
target/release/layerfs-dnf -qy --use-host-config install which

target/release/layerctl --store "$STORE" status | grep "^  update-head:" | grep -qv "(absent)"
test ! -e "$INSTALLROOT/usr/bin/which"

echo "dnf-adapter-smoke: PASS"
'
