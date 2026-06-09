#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"
log_init check

# NVIDIA Jetson APX (recovery) USB IDs on T234 family:
#   0955:7023  Jetson AGX Orin
#   0955:7423  Jetson Orin NX
#   0955:7523  Jetson Orin Nano
# Running L4T (RNDIS over USB):
#   0955:7020
RECOVERY_RE='0955:(7023|7423|7523)'
L4T_ID='0955:7020'

if lsusb | grep -qE "$RECOVERY_RE"; then
    MATCH=$(lsusb | grep -oE "$RECOVERY_RE" | head -n1)
    case "$MATCH" in
        0955:7023) MODEL="Jetson AGX Orin" ;;
        0955:7423) MODEL="Jetson Orin NX" ;;
        0955:7523) MODEL="Jetson Orin Nano" ;;
    esac
    log_ok "$MODEL in APX recovery ($MATCH)."
    exit 0
fi

if lsusb | grep -q "$L4T_ID"; then
    log_err "Jetson is running L4T ($L4T_ID), not in recovery."
    log_info "Hold the REC (recovery) button while pressing RST (reset). Then re-run."
    exit 1
fi

log_err "No Jetson detected on USB. Power on the board, hold REC, tap RST."
exit 1
