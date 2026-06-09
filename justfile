# Jetson Orin Nano flash driver — Ubuntu 24.04 host, terminal only.
# Defaults match Orin Nano 8GB Super devkit booting from NVMe.

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true

# -- Configurable knobs (override in .env or on the command line) -----------

L4T_VERSION       := env_var_or_default("L4T_VERSION", "39.2.0")
BSP_URL           := env_var_or_default("BSP_URL", "https://developer.nvidia.com/downloads/embedded/L4T/r39_Release_v2.0/release/Jetson_Linux_R39.2.0_aarch64.tbz2")
ROOTFS_URL        := env_var_or_default("ROOTFS_URL", "https://developer.nvidia.com/downloads/embedded/L4T/r39_Release_v2.0/release/Tegra_Linux_Sample-Root-Filesystem_R39.2.0_aarch64.tbz2")
BOARD             := env_var_or_default("BOARD", "jetson-orin-nano-devkit-super")
EXTERNAL_DEVICE   := env_var_or_default("EXTERNAL_DEVICE", "nvme0n1p1")
WORK              := justfile_directory() + "/work"
L4T_DIR           := WORK + "/Linux_for_Tegra"

# -- Default target --------------------------------------------------------

default:
    @just --list

# -- Pipeline --------------------------------------------------------------

# Full pipeline: deps -> fetch -> stage -> preconfig -> check -> flash.
all: deps fetch stage preconfig check flash

# Install host apt dependencies for L4T flash on Ubuntu 24.04.
deps:
    bash scripts/install-deps.sh

# Download BSP + sample rootfs tarballs into work/.
fetch:
    mkdir -p {{WORK}}
    bash scripts/fetch-bsp.sh "{{WORK}}" "{{BSP_URL}}" "{{ROOTFS_URL}}"

# Extract BSP, populate rootfs, run apply_binaries.sh.
stage:
    bash scripts/stage-rootfs.sh "{{WORK}}" "{{L4T_DIR}}"

# Bake user/password/hostname/headless/static-IP into the rootfs.
preconfig:
    bash scripts/preconfigure-rootfs.sh "{{L4T_DIR}}"

# Verify Jetson is in recovery mode (USB 0955:7023).
check:
    bash scripts/check-recovery.sh

# Flash to NVMe (Orin Nano Super devkit default).
flash:
    bash scripts/flash-nvme.sh "{{L4T_DIR}}" "{{BOARD}}" "{{EXTERNAL_DEVICE}}"

# Alternative: flash to internal QSPI + microSD only (no NVMe).
flash-sd:
    cd {{L4T_DIR}} && sudo ./flash.sh {{BOARD}} mmcblk0p1

# -- Convenience -----------------------------------------------------------

# Wipe the staging directory. Tarballs gone, must re-fetch.
clean:
    sudo rm -rf {{WORK}}

# Tail udev to watch USB transitions during recovery + flash.
watch-usb:
    sudo udevadm monitor --udev --subsystem-match=usb

# Show current Jetson USB state (recovery vs L4T).
usb-state:
    @lsusb | grep -i -E 'nvidia|0955' || echo "No Jetson on USB."

# Disable USB autosuspend (recommended before flashing).
no-autosuspend:
    echo -1 | sudo tee /sys/module/usbcore/parameters/autosuspend

# After flash: list serial consoles to attach (target reboots into oem-config).
serial:
    @ls -l /dev/serial/by-id/ 2>/dev/null || echo "No USB serial devices."

# Tail the latest log for a step: deps|fetch|stage|preconfig|check|flash.
logs step="flash":
    @f=$(ls -t logs/{{step}}-*.log 2>/dev/null | head -1); \
        [ -n "$f" ] && { echo "== $f =="; tail -n 40 "$f"; } || echo "No {{step}} logs yet."
