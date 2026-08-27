#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo build -p layerfs-init --bin layerfs-init \
    --target x86_64-unknown-linux-musl --release

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/usr/lib"
cp -a /usr/lib/dracut "$WORK/usr/lib/dracut"
mkdir "$WORK/usr/lib/dracut/modules.d/99layerfs"
cp integrations/dracut/module-setup.sh "$WORK/usr/lib/dracut/modules.d/99layerfs/module-setup.sh"

dracutbasedir="$WORK/usr/lib/dracut" \
    LAYERFS_INIT="$PWD/target/x86_64-unknown-linux-musl/release/layerfs-init" \
    dracut --force --no-hostonly --add layerfs --kver "$(uname -r)" "$WORK/layerfs.img"

CONTENTS="$(lsinitrd "$WORK/layerfs.img")"
grep -q 'usr/bin/layerfs-init' <<<"$CONTENTS"
grep -q 'kernel/fs/overlayfs/overlay.ko' <<<"$CONTENTS"
echo "dracut-smoke: PASS"
