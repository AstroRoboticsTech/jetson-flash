# jetson-flash

Terminal-only flashing pipeline for NVIDIA Jetson Orin devkits (Tegra234)
from an Ubuntu 24.04 host. No SDK Manager, no Nix. Drives the official
NVIDIA L4T `flash.sh` / `l4t_initrd_flash.sh` scripts through `just`.

The end result is a fully headless board that boots straight into a
logged-in shell, joins WiFi + wired LAN automatically, and answers to
`ssh <user>@<hostname>.local`. No display, no first-boot wizard.

## Supported boards

The pipeline is board-agnostic via the `.env` knobs. Per-board values
(board conf, iface names, boot device, recovery) live in their own doc:

| Board                          | Doc                                  | Status         |
|--------------------------------|--------------------------------------|----------------|
| Jetson Orin Nano 8GB Super     | [docs/orin-nano.md](docs/orin-nano.md) | Validated r39.2 |
| Jetson AGX Orin devkit         | [docs/orin-agx.md](docs/orin-agx.md)   | Board-ready (not yet flash-validated) |

## Prerequisites

- **Host:** Ubuntu 24.04 x86_64. ~10 GB free disk. USB-C cable to the board.
- **Target:** a supported Jetson Orin devkit with an NVMe SSD in the M.2
  M-key slot. See the per-board doc above for board-specific hardware
  (WiFi card, boot device, recovery buttons).
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

## Rust CLI (alternative to `just`)

The same pipeline is available as a Rust crate + CLI (`src/`, `Cargo.toml`),
for integration with other Rust commissioning tooling. It reads TOML config
(`jetson-flash.toml`) instead of `.env`; `JETSON_*` env vars override.

```bash
cargo install jetson-flash            # from crates.io (needs libusb-1.0-0-dev)
# or from a checkout:
cargo install --path .
jetson-flash check                    # deps|fetch|stage|preconfig|check|flash|all
```

| bash / just        | Rust CLI                     |
|--------------------|------------------------------|
| `just <step>`      | `jetson-flash <step>`        |
| `.env`             | `jetson-flash.toml`          |
| `lsusb` parse      | native libusb (`rusb`)       |
| `uuidgen`, keyfiles| generated in-process         |

Recovery detection and NM-keyfile/identity baking are native Rust; NVIDIA's
`l4t_*.sh` / `apply_binaries.sh` and `apt`/`wget`/`tar` are still driven as
subprocesses. Logs land in `logs/<step>-<ts>.log` (same as bash). As a library:

```rust
use jetson_flash::{Config, Paths, stages, logging::Logger};
let paths = Paths::new(std::path::Path::new("."));
let cfg = Config::load(&paths.repo_root.join("jetson-flash.toml"))?;
stages::check::run(&cfg, &paths, &Logger::init("check", &paths.repo_root)?)?;
```

When the flash finishes, unplug the USB-C data cable, power-cycle the
board, and SSH in:

```bash
ssh beppo@beppo.local
```

## Defaults

| Knob              | Default                              |
|-------------------|--------------------------------------|
| `L4T_VERSION`     | `39.2.0` (JetPack 7.2)               |
| `BOARD`           | `jetson-orin-nano-devkit-super` (see per-board doc) |
| `EXTERNAL_DEVICE` | `nvme0n1p1`                          |

> **JetPack 7.2 note.** JetPack 7.2 ships an interactive *Jetson ISO
> USB installer* as the consumer flow (SD-card images are dropped). That
> installer cannot bake user/hostname/headless/WiFi config and is not
> scriptable. This repo deliberately stays on the host-side BSP +
> `l4t_initrd_flash.sh` path, which still ships in the r39.2 Driver
> Package and is the only way to produce an unattended headless image.
> Rootfs is now Ubuntu 24.04 (was 22.04); kernel is 6.8.

`BOARD` is the one knob that changes per board — set it from the
[per-board doc](#supported-boards). Override via `.env` or on the command
line (`just BOARD=jetson-agx-orin-devkit flash`).

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

## Recovery mode

The board must be in APX recovery before `just flash`. Button/jumper
location differs per board — see the per-board doc. If the board is already
running and reachable, software-trigger it:

```bash
ssh <user>@<host> 'sudo reboot --force forced-recovery'
```

`just check` confirms recovery via `lsusb`:

| USB ID       | Board                       |
|--------------|-----------------------------|
| `0955:7523`  | Orin Nano in APX recovery   |
| `0955:7423`  | Orin NX in APX recovery     |
| `0955:7023`  | AGX Orin in APX recovery    |

`0955:7020` means L4T is already running — NOT recovery; re-trigger.

Per-board reachability (mDNS / eth / WiFi addresses) is documented in each
[board doc](#supported-boards).

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
