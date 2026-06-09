#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"
log_init stage

WORK="${1:?work dir required}"
L4T_DIR="${2:?l4t dir required}"

cd "$WORK"

BSP_TARBALL=$(ls Jetson_Linux_R*_aarch64.tbz2 | head -n1)
ROOTFS_TARBALL=$(ls Tegra_Linux_Sample-Root-Filesystem_R*_aarch64.tbz2 | head -n1)

if [[ ! -d "$L4T_DIR" ]]; then
    log_step "extract BSP -> Linux_for_Tegra/"
    tar xf "$BSP_TARBALL"
fi

cd "$L4T_DIR"

if [[ ! -e rootfs/etc/os-release ]]; then
    log_step "extract sample rootfs -> rootfs/"
    sudo tar xpf "$WORK/$ROOTFS_TARBALL" -C rootfs/
fi

if [[ -x tools/l4t_flash_prerequisites.sh ]]; then
    sudo ./tools/l4t_flash_prerequisites.sh
fi

if [[ ! -e rootfs/usr/lib/aarch64-linux-gnu/tegra ]]; then
    log_step "apply_binaries.sh"
    sudo ./apply_binaries.sh
fi

log_ok "Rootfs staged at $L4T_DIR/rootfs"
