#!/usr/bin/env bash
set -euo pipefail

# Bake first-boot identity, headless target, static IP into the L4T rootfs
# BEFORE flashing. After flashing the board boots straight to a logged-in
# tty (or SSH) on the configured IP.

L4T_DIR="${1:?l4t dir required}"
ROOTFS="$L4T_DIR/rootfs"

: "${JETSON_USERNAME:?JETSON_USERNAME required}"
: "${JETSON_PASSWORD:?JETSON_PASSWORD required}"
: "${JETSON_HOSTNAME:?JETSON_HOSTNAME required}"
JETSON_HEADLESS="${JETSON_HEADLESS:-true}"
JETSON_AUTOLOGIN="${JETSON_AUTOLOGIN:-true}"
JETSON_IFACE="${JETSON_IFACE:-eth0}"
JETSON_STATIC_IP="${JETSON_STATIC_IP:-}"
JETSON_GATEWAY="${JETSON_GATEWAY:-}"
JETSON_DNS="${JETSON_DNS:-1.1.1.1,8.8.8.8}"
JETSON_WIFI_IFACE="${JETSON_WIFI_IFACE:-wlan0}"
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

DNS_YAML=$(printf '"%s", ' ${JETSON_DNS//,/ } | sed 's/, $//')
NETPLAN_DIR="$ROOTFS/etc/netplan"

# --- 4. Static IP via netplan (eth primary, route metric 100) ------------
if [[ -n "$JETSON_STATIC_IP" ]]; then
    echo "[preconfig] static IP $JETSON_STATIC_IP on $JETSON_IFACE (metric 100)"
    sudo mkdir -p "$NETPLAN_DIR"
    # Strip any default NetworkManager-managed YAML so netplan owns $JETSON_IFACE
    sudo rm -f "$NETPLAN_DIR"/01-network-manager-all.yaml
    GATEWAY_LINE=""
    if [[ -n "$JETSON_GATEWAY" ]]; then
        GATEWAY_LINE="      routes:
        - to: default
          via: $JETSON_GATEWAY
          metric: 100"
    fi
    sudo tee "$NETPLAN_DIR/01-static-${JETSON_IFACE}.yaml" >/dev/null <<EOF
network:
  version: 2
  renderer: networkd
  ethernets:
    $JETSON_IFACE:
      dhcp4: false
      addresses:
        - $JETSON_STATIC_IP
$GATEWAY_LINE
      nameservers:
        addresses: [$DNS_YAML]
EOF
    sudo chmod 600 "$NETPLAN_DIR/01-static-${JETSON_IFACE}.yaml"
fi

# --- 4b. WiFi via netplan (skipped if SSID blank; static if IP set) ------
if [[ -n "$JETSON_WIFI_SSID" ]]; then
    if [[ -z "$JETSON_WIFI_PSK" ]]; then
        echo "[fail] JETSON_WIFI_SSID set but JETSON_WIFI_PSK empty."
        exit 1
    fi
    sudo mkdir -p "$NETPLAN_DIR"
    if [[ -n "$JETSON_WIFI_STATIC_IP" ]]; then
        echo "[preconfig] WiFi $JETSON_WIFI_SSID static $JETSON_WIFI_STATIC_IP on $JETSON_WIFI_IFACE (metric 200)"
        WIFI_ROUTES=""
        if [[ -n "$JETSON_WIFI_GATEWAY" ]]; then
            WIFI_ROUTES="      routes:
        - to: default
          via: $JETSON_WIFI_GATEWAY
          metric: 200"
        fi
        sudo tee "$NETPLAN_DIR/02-wifi-${JETSON_WIFI_IFACE}.yaml" >/dev/null <<EOF
network:
  version: 2
  renderer: networkd
  wifis:
    $JETSON_WIFI_IFACE:
      dhcp4: false
      optional: true
      addresses:
        - $JETSON_WIFI_STATIC_IP
$WIFI_ROUTES
      nameservers:
        addresses: [$DNS_YAML]
      access-points:
        "$JETSON_WIFI_SSID":
          password: "$JETSON_WIFI_PSK"
EOF
    else
        echo "[preconfig] WiFi $JETSON_WIFI_SSID DHCP on $JETSON_WIFI_IFACE"
        sudo tee "$NETPLAN_DIR/02-wifi-${JETSON_WIFI_IFACE}.yaml" >/dev/null <<EOF
network:
  version: 2
  renderer: networkd
  wifis:
    $JETSON_WIFI_IFACE:
      dhcp4: $JETSON_WIFI_DHCP
      dhcp4-overrides:
        route-metric: 200
      optional: true
      access-points:
        "$JETSON_WIFI_SSID":
          password: "$JETSON_WIFI_PSK"
EOF
    fi
    sudo chmod 600 "$NETPLAN_DIR/02-wifi-${JETSON_WIFI_IFACE}.yaml"
    sudo chroot "$ROOTFS" /bin/bash -c "systemctl enable wpa_supplicant.service" 2>/dev/null || \
        sudo ln -sf /lib/systemd/system/wpa_supplicant.service \
            "$ROOTFS/etc/systemd/system/multi-user.target.wants/wpa_supplicant.service" 2>/dev/null || true
fi

# --- 4c. mDNS / avahi (so beppo.local resolves) --------------------------
if [[ "$JETSON_AVAHI" == "true" ]]; then
    echo "[preconfig] enabling avahi-daemon (mDNS for ${JETSON_HOSTNAME}.local)"
    sudo chroot "$ROOTFS" /bin/bash -c "systemctl enable avahi-daemon.service" 2>/dev/null || \
        sudo ln -sf /lib/systemd/system/avahi-daemon.service \
            "$ROOTFS/etc/systemd/system/multi-user.target.wants/avahi-daemon.service"
    # nsswitch already includes mdns4_minimal on Ubuntu but make it explicit
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
