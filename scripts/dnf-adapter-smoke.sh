#!/usr/bin/env bash
# Runs a real `dnf install` through the *installed* layerfs-dnf adapter
# inside a privileged Fedora container: a minimal base is bootstrapped with
# dnf's own --installroot (so this doesn't try to copy /proc, /sys, ... like
# a live root would), `layerctl install --integrations dnf` activates the
# adapter the same way a real retrofit install would, and then plain `dnf`
# (now a symlink to layerfs-dnf, with the real binary preserved alongside
# it) drives an actual transaction.
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

target/release/layerctl --store "$STORE" install --source "$INSTALLROOT" \
    --integrations dnf --adapter-bin "dnf=$PWD/target/release/layerfs-dnf"

test "$(readlink "$STORE/base/usr/bin/dnf")" = "layerfs-dnf"
test -x "$STORE/base/usr/bin/dnf.layerfs-real"

LAYERFS_STORE="$STORE" \
LAYERFS_LAYERCTL_BIN="$PWD/target/release/layerctl" \
"$STORE/base/usr/bin/dnf" -qy --use-host-config install which

target/release/layerctl --store "$STORE" status | grep "^  update-head:" | grep -qv "(absent)"
test ! -e "$STORE/base/usr/bin/which"

echo "dnf-adapter-smoke: PASS"
'
