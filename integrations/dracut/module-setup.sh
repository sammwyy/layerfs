#!/usr/bin/bash

check() {
    return 255
}

depends() {
    echo "base kernel-modules"
    return 0
}

install() {
    local init_bin=${LAYERFS_INIT:-/usr/libexec/layerfs/layerfs-init}
    [[ -x $init_bin ]] || dfatal "layerfs-init not found: $init_bin"
    inst_simple "$init_bin" /sbin/layerfs-init
}

installkernel() {
    hostonly='' instmods overlay
}

cmdline() {
    printf ' rdinit=/sbin/layerfs-init'
}
