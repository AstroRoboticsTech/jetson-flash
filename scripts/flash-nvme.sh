#!/usr/bin/env bash
set -euo pipefail

L4T_DIR="${1:?l4t dir required}"
BOARD="${2:?board config required}"
EXTERNAL_DEVICE="${3:?external device required}"

cd "$L4T_DIR"

if [[ ! -x tools/kernel_flash/l4t_initrd_flash.sh ]]; then
    echo "[fail] $L4T_DIR/tools/kernel_flash/l4t_initrd_flash.sh missing. Run 'just stage' first."
    exit 1
fi

echo "[flash] board=$BOARD device=$EXTERNAL_DEVICE"
sudo ./tools/kernel_flash/l4t_initrd_flash.sh \
    --external-device "$EXTERNAL_DEVICE" \
    -c tools/kernel_flash/flash_l4t_t234_nvme.xml \
    -p "-c bootloader/generic/cfg/flash_t234_qspi.xml" \
    --showlogs \
    --network usb0 \
    "$BOARD" internal

echo "[ok] Flash complete. Disconnect USB-C cable, reboot board, run oem-config on serial console."
