# Legacy `just` / bash pipeline

> Superseded by the Rust CLI (`jetson-flash`, see the [README](../README.md)).
> The `justfile` + `scripts/` remain in-tree and functional; this doc is the
> reference for that path. New work should use the Rust CLI.

Terminal-only flashing pipeline driving NVIDIA's L4T `flash.sh` /
`l4t_initrd_flash.sh` through `just`, configured via a flat `.env`.

## Prerequisites

- **Host:** Ubuntu 24.04 x86_64, ~10 GB free, USB-C to the board.
- `just`: `cargo install just` or `apt install just` (24.04 ships ≥1.34).
- `git`, `wget`, `sudo`.

## Getting started

```bash
cp .env.example .env
$EDITOR .env        # set JETSON_USERNAME / PASSWORD / HOSTNAME, WiFi, etc.
```

```bash
just deps            # apt deps for the Ubuntu 24.04 host (sudo)
just fetch           # download BSP + sample rootfs (~2.5 GB)
just stage           # extract, apply_binaries.sh (sudo, ~3 min)
just preconfig       # bake user, hostname, headless, IPs, WiFi, avahi
just check           # confirm APX recovery
just no-autosuspend  # keep USB awake during flash
just flash           # initrd flash to NVMe (~15-25 min)
# or: just all
```

## Defaults

| Knob              | Default                                             |
|-------------------|-----------------------------------------------------|
| `L4T_VERSION`     | `39.2.0` (JetPack 7.2)                              |
| `BOARD`           | `jetson-orin-nano-devkit-super` (see per-board doc) |
| `EXTERNAL_DEVICE` | `nvme0n1p1`                                         |

Override via `.env` or the command line (`just BOARD=jetson-agx-orin-devkit flash`).

## First-boot identity (`.env`)

| Knob               | Effect                                                         |
|--------------------|----------------------------------------------------------------|
| `JETSON_USERNAME`  | Default user, baked via `l4t_create_default_user.sh`.          |
| `JETSON_PASSWORD`  | Initial password for that user.                                |
| `JETSON_HOSTNAME`  | `/etc/hostname`.                                               |
| `JETSON_HEADLESS`  | `true` → multi-user.target; mask gdm/oem-config GUI.           |
| `JETSON_AUTOLOGIN` | `true` → systemd getty autologin on tty1.                      |
| `JETSON_ETH_DEV`   | PCIe iface name. Orin Nano: `enP8p1s0`. Find via `ip -br link`.|
| `JETSON_STATIC_IP` | CIDR, e.g. `10.42.0.10/24`. Leave blank for DHCP.              |
| `JETSON_GATEWAY`   | Default route (leave blank → eth never owns default route).    |
| `JETSON_DNS`       | Comma-separated DNS servers.                                   |
| `JETSON_WIFI_SSID` | SSID. Blank → skip WiFi config. Requires M.2 WiFi card.        |
| `JETSON_WIFI_PSK`  | WPA2 passphrase.                                               |
| `JETSON_WIFI_DEV`  | Blank → NetworkManager matches by SSID only (recommended).     |
| `JETSON_WIFI_DHCP` | `true` (default) or `false`. Ignored when static IP is set.    |
| `JETSON_WIFI_STATIC_IP` | Static CIDR for WiFi, e.g. `192.168.1.101/24`.            |
| `JETSON_WIFI_GATEWAY`   | WiFi default route gateway (route metric 200, fallback).  |
| `JETSON_AVAHI`     | `true` enables avahi-daemon → reach as `<hostname>.local`.     |

## What `just preconfig` bakes into the rootfs

1. Creates the default user (`l4t_create_default_user.sh`), skipping oem-config.
2. `default.target` → `multi-user.target`; masks `gdm3` and `nv-oem-config-gui`.
3. getty `agetty --autologin` override on tty1.
4. NetworkManager keyfiles in `/etc/NetworkManager/system-connections/`
   (`eth-static.nmconnection`, `wifi-home.nmconnection`, mode 600). Eth pins by
   `interface-name=$JETSON_ETH_DEV`; WiFi matches by SSID, PSK plaintext, default
   route metric 200 so eth stays primary when both have gateways.
5. Enables `ssh.service` and `avahi-daemon.service`.
6. Patches `/etc/nsswitch.conf` so `mdns4_minimal` resolves before DNS.

## Ubuntu 24.04 host gotchas

- Python 3.12 dropped `distutils`; `python3-setuptools` (in `just deps`) covers it.
- AppArmor can block NFS during initrd flash; `sudo systemctl stop apparmor` if
  the rootfs transfer hangs.
- USB autosuspend can interrupt flashing: `just no-autosuspend` before `just flash`.

## Troubleshooting

- **Flash hangs on "Sending bootloader…":** USB autosuspend or a flaky cable.
  Re-cycle REC+RST, `just no-autosuspend`, prefer a direct host port over a hub.
- **`<hostname>.local` won't resolve:** the host also needs avahi.
- **Re-flash after a tweak:** `just preconfig` is idempotent for most steps but
  `l4t_create_default_user.sh` may complain on a second run; `just clean` + retry.
