#!/usr/bin/env bash
set -euo pipefail

if ! grep -q 'VERSION_ID="24.04"' /etc/os-release; then
    echo "[warn] Host is not Ubuntu 24.04. Continuing anyway."
fi

PKGS=(
    qemu-user-static
    lz4
    libxml2-utils
    abootimg
    sshpass
    python3
    python3-yaml
    python3-setuptools
    binfmt-support
    nfs-kernel-server
    dosfstools
    uuid-runtime
    wget
    curl
    ca-certificates
)

sudo apt-get update
sudo apt-get install -y "${PKGS[@]}"

if [[ ! -e /proc/sys/fs/binfmt_misc/qemu-aarch64 ]]; then
    echo "[info] Registering qemu-aarch64 binfmt handler."
    sudo systemctl restart systemd-binfmt || true
fi

echo "[ok] Host dependencies installed."
