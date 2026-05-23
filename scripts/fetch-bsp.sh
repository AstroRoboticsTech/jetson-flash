#!/usr/bin/env bash
set -euo pipefail

WORK="${1:?work dir required}"
BSP_URL="${2:?bsp url required}"
ROOTFS_URL="${3:?rootfs url required}"

mkdir -p "$WORK"
cd "$WORK"

BSP_FILE="$(basename "$BSP_URL")"
ROOTFS_FILE="$(basename "$ROOTFS_URL")"

fetch() {
    local url="$1" file="$2"
    if [[ -f "$file" ]]; then
        echo "[skip] $file already present."
        return
    fi
    echo "[fetch] $url"
    wget --content-disposition -O "$file" "$url"
}

fetch "$BSP_URL" "$BSP_FILE"
fetch "$ROOTFS_URL" "$ROOTFS_FILE"

echo "[ok] Tarballs in $WORK."
ls -lh "$BSP_FILE" "$ROOTFS_FILE"
