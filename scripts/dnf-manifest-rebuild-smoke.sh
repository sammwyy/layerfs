#!/usr/bin/env bash
# Proves rebuild works from an already-saved manifest, with no fresh
# query needed once UPDATE/UPDATE_HEAD are gone.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo build --release -p layerctl -p layerfs-dnf

docker run --rm --privileged --network bridge \
    -v "$PWD:/workspace" -w /workspace fedora:42 bash -lc '
set -euo pipefail
dnf -qy install dnf coreutils findutils python3 >/dev/null

STORE=$(mktemp -d)
mount -t tmpfs tmpfs "$STORE"

INSTALLROOT=$(mktemp -d)
mount -t tmpfs tmpfs "$INSTALLROOT"
dnf -qy --installroot="$INSTALLROOT" --releasever=42 --use-host-config \
    --setopt=install_weak_deps=False install dnf coreutils >/dev/null
cp /etc/resolv.conf "$INSTALLROOT/etc/resolv.conf"

target/release/layerctl --store "$STORE" install --source "$INSTALLROOT" \
    --integrations dnf --adapter-bin "dnf=$PWD/target/release/layerfs-dnf" >/dev/null

LAYERFS_STORE="$STORE" \
LAYERFS_LAYERCTL_BIN="$PWD/target/release/layerctl" \
LAYERFS_LIVE_ROOT="$STORE/update-head" \
"$STORE/base/usr/bin/dnf" -qy --use-host-config install which

python3 -c "
import json
m = json.load(open(\"$STORE/manifest/dnf.json\"))
assert \"which\" in m[\"packages\"], m[\"packages\"]
"

target/release/layerctl --store "$STORE" rebuild updates
test ! -e "$STORE/update-head"
test -f "$STORE/manifest/dnf.json"

LAYERFS_STORE="$STORE" \
LAYERFS_LAYERCTL_BIN="$PWD/target/release/layerctl" \
"$STORE/base/usr/bin/dnf" --layerfs-manifest-apply

test -f "$STORE/update-head/usr/bin/which"
echo "dnf-manifest-rebuild-smoke: PASS"
'
