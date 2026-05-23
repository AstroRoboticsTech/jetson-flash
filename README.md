# jetson-flash

Terminal-only flashing pipeline for the Jetson Orin Nano 8GB Super devkit
from an Ubuntu 24.04 host. No SDK Manager, no Nix. Drives the official
NVIDIA L4T `flash.sh` / `l4t_initrd_flash.sh` scripts through `just`.

The end result is a fully headless Orin Nano that boots straight into a
logged-in shell, joins WiFi + wired LAN automatically, and answers to
`ssh <user>@<hostname>.local`. No display, no first-boot wizard.

## Prerequisites

- **Host:** Ubuntu 24.04 x86_64. ~10 GB free disk. USB-C cable to the board.
- **Target:** Jetson Orin Nano 8GB devkit. NVMe SSD installed in the M.2
  M-key slot. (For WiFi: M.2 E-key WiFi card seated in the small slot —
  the bare Nano module has no radio.)
- `just` installed: `cargo install just` or `apt install just` (24.04
  ships ≥1.34).
- `git`, `wget`, `sudo`.

## Getting started

```bash
git clone <this repo> ~/dev/jetson-flash
cd ~/dev/jetson-flash
cp .env.example .env
# Edit .env — at minimum set JETSON_USERNAME / PASSWORD / HOSTNAME and
# the WiFi credentials.
$EDITOR .env
```

Then walk the pipeline:

```bash
just deps        # apt deps for Ubuntu 24.04 host (sudo)
just fetch       # download BSP + sample rootfs (~2.5 GB)
just stage       # extract, apply_binaries.sh (sudo, ~3 min)
just preconfig   # bake user, hostname, headless target, IPs, WiFi, avahi
just check       # confirm board is in APX recovery
just no-autosuspend  # keep USB awake during flash
just flash       # initrd flash to NVMe (~15-25 min)
```

Or all in one:

```bash
just all
```

When the flash finishes, unplug the USB-C data cable, power-cycle the
board, and SSH in:

```bash
ssh beppo@beppo.local
```

## Defaults

| Knob              | Default                              |
|-------------------|--------------------------------------|
| `L4T_VERSION`     | `36.4.4` (JetPack 6.2.1)             |
| `BOARD`           | `jetson-orin-nano-devkit-super`      |
| `EXTERNAL_DEVICE` | `nvme0n1p1`                          |

Override via `.env` (copy from `.env.example`) or on the command line:

```bash
just BOARD=jetson-orin-nano-devkit flash
```

## First-boot identity (set in `.env`)

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

After `just flash` the board boots directly to a logged-in tty on the
configured IP. SSH is enabled. No display, no oem-config wizard.

## Reachability

With the default `.env`:

| Address                  | Path                                  |
|--------------------------|---------------------------------------|
| `ssh beppo@beppo.local`  | mDNS (avahi) — works on any iface     |
| `ssh beppo@10.42.0.10`   | Direct Ethernet cable to dev host     |
| `ssh beppo@192.168.1.101`| Home LAN via WiFi (primary internet)  |

Host side, set the dev cable end to `10.42.0.1/24` (NetworkManager or
`nmcli con add type ethernet ifname enpXsY ipv4.addresses 10.42.0.1/24`).

## Recovery mode

Hold the **REC** (force-recovery) button on the carrier, tap **RST**
(reset), release REC. `lsusb` should show one of:

| USB ID       | Board                       |
|--------------|-----------------------------|
| `0955:7523`  | Orin Nano in APX recovery   |
| `0955:7423`  | Orin NX in APX recovery     |
| `0955:7023`  | AGX Orin in APX recovery    |

`0955:7020` means L4T is already running — NOT recovery; cycle REC+RST.

## Ubuntu 24.04 host gotchas

- Python 3.12 dropped `distutils`. `python3-setuptools` (in `just deps`)
  covers it.
- AppArmor on 24.04 can block NFS during initrd flash; stop it if the
  rootfs transfer hangs: `sudo systemctl stop apparmor`.
- USB autosuspend can interrupt flashing: run `just no-autosuspend`
  before `just flash`.

## What gets baked into the rootfs

`just preconfig` runs against the staged `Linux_for_Tegra/rootfs/` and:

1. Creates the default user (`l4t_create_default_user.sh`), skipping
   oem-config.
2. Sets `default.target` to `multi-user.target`, masks `gdm3` and
   `nv-oem-config-gui`.
3. Drops a getty `agetty --autologin` override on tty1.
4. Writes NetworkManager keyfiles into
   `/etc/NetworkManager/system-connections/` (`eth-static.nmconnection`,
   `wifi-home.nmconnection`, mode 600). Eth keyfile pins by
   `interface-name=$JETSON_ETH_DEV`; WiFi keyfile matches by SSID and
   stores the PSK plaintext. WiFi default route uses metric 200 so eth
   stays primary when both ifaces have gateways.
5. Enables `ssh.service` and `avahi-daemon.service`. NetworkManager is
   already active in stock L4T so wpa_supplicant runs on demand via NM.
6. Patches `/etc/nsswitch.conf` so `mdns4_minimal` resolves before DNS.

## Troubleshooting

- **Flash hangs on "Sending bootloader and pre-requisite binaries":** USB
  autosuspend or a flaky cable. Re-cycle REC+RST, `just no-autosuspend`,
  swap USB ports (prefer a direct host port over a hub).
- **`<hostname>.local` does not resolve from host:** host needs avahi
  too (`sudo apt install avahi-daemon` on Linux dev box; macOS has it
  built-in).
- **WiFi connects but no internet:** verify `JETSON_WIFI_GATEWAY` is
  reachable; `ip route` on the board should show a default route via
  wlan0.
- **Re-flash after a tweak:** `just preconfig` is idempotent for most
  steps but `l4t_create_default_user.sh` may complain on second run.
  `just clean` + start over if needed.
