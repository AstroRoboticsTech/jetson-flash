# Jetson Orin Nano 8GB Super devkit

Board-specific knobs and procedure. Generic pipeline lives in the
[README](../README.md).

## `.env` knobs

| Knob              | Value                              |
|-------------------|------------------------------------|
| `BOARD`           | `jetson-orin-nano-devkit-super`    |
| `EXTERNAL_DEVICE` | `nvme0n1p1`                        |
| `JETSON_ETH_DEV`  | `enP8p1s0`                         |

- **`-super` conf** flashes the MAXN SUPER BPMP/DTB → uplifted clocks (25W
  mode). Use plain `jetson-orin-nano-devkit` for stock clocks.
- **WiFi** needs an M.2 E-key card (`wlP1p1s0`); the bare module has no
  radio. Leave `JETSON_WIFI_SSID` blank to skip.
- Both ifaces use PCIe-path predictable names (not `eth0`/`wlan0`); they
  are stable across kernel 6.8.

## Hardware

- NVMe SSD seated in the M.2 **M-key** slot (boots from NVMe; module has no
  eMMC).
- Optional M.2 **E-key** WiFi card in the small slot.

## Recovery mode

Hold **REC** (force-recovery) on the carrier, tap **RST**, release REC.
Or, if it is already running and reachable:

```bash
ssh <user>@<host> 'sudo reboot --force forced-recovery'
```

`lsusb` should show `0955:7523` (Orin Nano in APX). `0955:7020` = L4T
running, not recovery — re-cycle REC+RST.

## Reachability (example `.env`)

| Address                  | Path                                |
|--------------------------|-------------------------------------|
| `ssh beppo@beppo.local`  | mDNS (avahi) — any iface            |
| `ssh beppo@10.42.0.10`   | Direct Ethernet cable to dev host   |
| `ssh beppo@192.168.1.101`| Home LAN via WiFi                   |

Host side, set the dev cable end to `10.42.0.1/24`.

## Validated — JetPack 7.2 / L4T r39.2

| Check          | Result                          |
|----------------|---------------------------------|
| L4T / JetPack  | R39.2.0 / 7.2                   |
| OS / kernel    | Ubuntu 24.04.4 / 6.8.12-tegra  |
| Boot device    | `nvme0n1p1` ext4, expanded 227G |
| Headless       | `multi-user.target`, tty1 autologin |
| Power model    | 25W MAXN SUPER                  |
| Net            | WiFi static metric 200, eth keyfile, mDNS+ssh |
