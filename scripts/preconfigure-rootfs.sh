#!/usr/bin/env bash
set -euo pipefail

# Bake first-boot identity, headless target, static IPs, WiFi, and mDNS
# into the L4T rootfs BEFORE flashing. After flashing the board boots
# straight to a logged-in shell, joins WiFi + wired LAN automatically,
# and answers to ssh <user>@<hostname>.local — no manual fix-up needed.
#
# Networking is configured via NetworkManager keyfiles (not netplan).
# Stock Jetson L4T runs NetworkManager as the active stack with
# systemd-networkd masked; netplan YAML with `renderer: networkd` is
# silently ignored on first boot. Keyfiles in
# /etc/NetworkManager/system-connections/ are picked up directly.

L4T_DIR="${1:?l4t dir required}"
ROOTFS="$L4T_DIR/rootfs"

: "${JETSON_USERNAME:?JETSON_USERNAME required}"
: "${JETSON_PASSWORD:?JETSON_PASSWORD required}"
: "${JETSON_HOSTNAME:?JETSON_HOSTNAME required}"
JETSON_HEADLESS="${JETSON_HEADLESS:-true}"
JETSON_AUTOLOGIN="${JETSON_AUTOLOGIN:-true}"
# Jetson uses PCIe-prefixed predictable iface names, NOT eth0/wlan0.
# Defaults below are correct for the Orin Nano devkit (Realtek RTL8111 +
# Realtek RTL8822CE). For other modules: lspci + ip link from a booted
# stock image to discover the names, then override here.
JETSON_ETH_DEV="${JETSON_ETH_DEV:-enP8p1s0}"
JETSON_WIFI_DEV="${JETSON_WIFI_DEV:-}"   # blank = match by SSID only
JETSON_STATIC_IP="${JETSON_STATIC_IP:-}"
JETSON_GATEWAY="${JETSON_GATEWAY:-}"
JETSON_DNS="${JETSON_DNS:-1.1.1.1,8.8.8.8}"
JETSON_WIFI_SSID="${JETSON_WIFI_SSID:-}"
JETSON_WIFI_PSK="${JETSON_WIFI_PSK:-}"
JETSON_WIFI_DHCP="${JETSON_WIFI_DHCP:-true}"
JETSON_WIFI_STATIC_IP="${JETSON_WIFI_STATIC_IP:-}"
JETSON_WIFI_GATEWAY="${JETSON_WIFI_GATEWAY:-}"
JETSON_AVAHI="${JETSON_AVAHI:-true}"

cd "$L4T_DIR"

if [[ ! -d "$ROOTFS/etc" ]]; then
    echo "[fail] $ROOTFS not staged. Run 'just stage' first."
    exit 1
fi

# --- 1. Pre-create user, skip oem-config wizard ---------------------------
echo "[preconfig] creating default user $JETSON_USERNAME on host $JETSON_HOSTNAME"
sudo ./tools/l4t_create_default_user.sh \
    -u "$JETSON_USERNAME" \
    -p "$JETSON_PASSWORD" \
    -n "$JETSON_HOSTNAME" \
    --accept-license

# --- 2. Headless: drop GUI target, mask display managers ------------------
if [[ "$JETSON_HEADLESS" == "true" ]]; then
    echo "[preconfig] headless mode: multi-user.target + mask gdm3"
    sudo ln -sf /lib/systemd/system/multi-user.target "$ROOTFS/etc/systemd/system/default.target"
    sudo ln -sf /dev/null "$ROOTFS/etc/systemd/system/gdm3.service"
    sudo ln -sf /dev/null "$ROOTFS/etc/systemd/system/gdm.service"
    sudo ln -sf /dev/null "$ROOTFS/etc/systemd/system/nv-oem-config-gui.service" 2>/dev/null || true
fi

# --- 3. Auto-login on tty1 -----------------------------------------------
if [[ "$JETSON_AUTOLOGIN" == "true" ]]; then
    echo "[preconfig] enabling tty1 autologin for $JETSON_USERNAME"
    OVERRIDE_DIR="$ROOTFS/etc/systemd/system/getty@tty1.service.d"
    sudo mkdir -p "$OVERRIDE_DIR"
    sudo tee "$OVERRIDE_DIR/override.conf" >/dev/null <<EOF
[Service]
ExecStart=
ExecStart=-/sbin/agetty --autologin $JETSON_USERNAME --noclear %I \$TERM
EOF
fi

# --- 4. Networking via NetworkManager keyfiles ---------------------------
NM_DIR="$ROOTFS/etc/NetworkManager/system-connections"
DNS_NM=$(echo "$JETSON_DNS" | sed 's/,/;/g')

# 4a. Ethernet static IP --------------------------------------------------
if [[ -n "$JETSON_STATIC_IP" ]]; then
    echo "[preconfig] eth static $JETSON_STATIC_IP on $JETSON_ETH_DEV"
    sudo mkdir -p "$NM_DIR"
    ETH_UUID=$(uuidgen)
    if [[ -n "$JETSON_GATEWAY" ]]; then
        ETH_ADDR="${JETSON_STATIC_IP},${JETSON_GATEWAY}"
        ETH_NEVER_DEFAULT=""
    else
        # No gateway → never-default keeps eth from hijacking the default route
        ETH_ADDR="$JETSON_STATIC_IP"
        ETH_NEVER_DEFAULT="never-default=true"
    fi
    sudo tee "$NM_DIR/eth-static.nmconnection" >/dev/null <<EOF
[connection]
id=eth-static
type=ethernet
uuid=$ETH_UUID
interface-name=$JETSON_ETH_DEV
autoconnect=true

[ethernet]

[ipv4]
method=manual
address1=$ETH_ADDR
dns=$DNS_NM;
$ETH_NEVER_DEFAULT

[ipv6]
method=auto
EOF
    sudo chmod 600 "$NM_DIR/eth-static.nmconnection"
    sudo chown root:root "$NM_DIR/eth-static.nmconnection"
fi

# 4b. WiFi via NetworkManager keyfile -------------------------------------
if [[ -n "$JETSON_WIFI_SSID" ]]; then
    if [[ -z "$JETSON_WIFI_PSK" ]]; then
        echo "[fail] JETSON_WIFI_SSID set but JETSON_WIFI_PSK empty."
        exit 1
    fi
    sudo mkdir -p "$NM_DIR"
    WIFI_UUID=$(uuidgen)
    WIFI_IFACE_LINE=""
    [[ -n "$JETSON_WIFI_DEV" ]] && WIFI_IFACE_LINE="interface-name=$JETSON_WIFI_DEV"

    if [[ -n "$JETSON_WIFI_STATIC_IP" ]]; then
        echo "[preconfig] WiFi $JETSON_WIFI_SSID static $JETSON_WIFI_STATIC_IP (metric 200)"
        if [[ -n "$JETSON_WIFI_GATEWAY" ]]; then
            WIFI_ADDR="${JETSON_WIFI_STATIC_IP},${JETSON_WIFI_GATEWAY}"
        else
            WIFI_ADDR="$JETSON_WIFI_STATIC_IP"
        fi
        IPV4_BLOCK="[ipv4]
method=manual
address1=$WIFI_ADDR
dns=$DNS_NM;
route-metric=200"
    elif [[ "$JETSON_WIFI_DHCP" == "true" ]]; then
        echo "[preconfig] WiFi $JETSON_WIFI_SSID DHCP (metric 200)"
        IPV4_BLOCK="[ipv4]
method=auto
route-metric=200"
    else
        IPV4_BLOCK="[ipv4]
method=disabled"
    fi

    sudo tee "$NM_DIR/wifi-home.nmconnection" >/dev/null <<EOF
[connection]
id=wifi-home
type=wifi
uuid=$WIFI_UUID
$WIFI_IFACE_LINE
autoconnect=true

[wifi]
mode=infrastructure
ssid=$JETSON_WIFI_SSID

[wifi-security]
key-mgmt=wpa-psk
psk=$JETSON_WIFI_PSK

$IPV4_BLOCK

[ipv6]
method=auto
EOF
    sudo chmod 600 "$NM_DIR/wifi-home.nmconnection"
    sudo chown root:root "$NM_DIR/wifi-home.nmconnection"
fi

# --- 4c. mDNS / avahi (so <hostname>.local resolves) ---------------------
if [[ "$JETSON_AVAHI" == "true" ]]; then
    echo "[preconfig] enabling avahi-daemon (mDNS for ${JETSON_HOSTNAME}.local)"
    sudo chroot "$ROOTFS" /bin/bash -c "systemctl enable avahi-daemon.service" 2>/dev/null || \
        sudo ln -sf /lib/systemd/system/avahi-daemon.service \
            "$ROOTFS/etc/systemd/system/multi-user.target.wants/avahi-daemon.service"
    if [[ -f "$ROOTFS/etc/nsswitch.conf" ]] && ! grep -q 'mdns' "$ROOTFS/etc/nsswitch.conf"; then
        sudo sed -i 's/^hosts:.*$/hosts:          files mdns4_minimal [NOTFOUND=return] dns/' \
            "$ROOTFS/etc/nsswitch.conf"
    fi
fi

# --- 5. SSH server enabled by default -------------------------------------
echo "[preconfig] enabling ssh"
sudo chroot "$ROOTFS" /bin/bash -c "systemctl enable ssh.service" 2>/dev/null || \
    sudo ln -sf /lib/systemd/system/ssh.service \
        "$ROOTFS/etc/systemd/system/multi-user.target.wants/ssh.service"

echo "[ok] Rootfs preconfigured. Ready to flash."
