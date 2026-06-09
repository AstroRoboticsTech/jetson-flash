#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"
log_init flash

L4T_DIR="${1:?l4t dir required}"
BOARD="${2:?board config required}"
EXTERNAL_DEVICE="${3:?external device required}"

cd "$L4T_DIR"

if [[ ! -x tools/kernel_flash/l4t_initrd_flash.sh ]]; then
    log_err "$L4T_DIR/tools/kernel_flash/l4t_initrd_flash.sh missing. Run 'just stage' first."
    exit 1
fi

log_step "flash board=$BOARD device=$EXTERNAL_DEVICE (NVMe + internal QSPI; ~15-25 min)"
sudo ./tools/kernel_flash/l4t_initrd_flash.sh \
    --external-device "$EXTERNAL_DEVICE" \
    -c tools/kernel_flash/flash_l4t_t234_nvme.xml \
    -p "-c bootloader/generic/cfg/flash_t234_qspi.xml" \
    --showlogs \
    --network usb0 \
    "$BOARD" internal

log_ok "Flash complete. Disconnect USB-C cable and power-cycle the board; it boots headless to the baked user."
