# Jetson AGX Orin devkit

Board-specific knobs and procedure. Generic pipeline lives in the
[README](../README.md). Same Tegra234 SoC as the Orin Nano, so the BSP, the
`t234` NVMe flash XML, and all scripts are unchanged — only `.env` differs.

## `.env` knobs

| Knob              | Value                                    |
|-------------------|------------------------------------------|
| `BOARD`           | `jetson-agx-orin-devkit` (or `-super`)   |
| `EXTERNAL_DEVICE` | `nvme0n1p1`                              |
| `JETSON_ETH_DEV`  | **verify per board** (often `enP1p1s0`) |

- **`-super` conf** enables MAXN SUPER clocks (AGX Orin 64GB supports it).
  Plain `jetson-agx-orin-devkit` = stock profile.
- **`JETSON_ETH_DEV` differs from the Nano.** AGX's onboard NIC (Marvell
  AQtion) gets a different PCIe-path name than `enP8p1s0`. Do not copy the
  Nano value. Discover it (below).
- **WiFi** needs an M.2 E-key card — AGX devkit has no onboard radio. Leave
  `JETSON_WIFI_SSID` blank if absent.

## Discovering `JETSON_ETH_DEV`

The eth keyfile pins by `interface-name`, so a wrong name = no wired IP.
First pass, leave `JETSON_STATIC_IP` blank (DHCP) and keep avahi on, reach
the board as `<hostname>.local`, then read the real name:

```bash
ssh <user>@<host>.local 'ip -br link'
```

Set `JETSON_ETH_DEV` to that, then re-run `just preconfig` (+ reflash) if
you want a static eth IP.

## Hardware

- NVMe SSD seated in the M.2 **M-key** slot — the pipeline flashes NVMe
  (`--external-device`), not the AGX's internal eMMC.
- eMMC boot is a different path (`flash.sh jetson-agx-orin-devkit internal`
  → `mmcblk0`); not covered by `just flash`.

## Recovery mode

AGX Orin devkit has 3 buttons behind the front bezel: **Power**, **Force
Recovery** (middle), **Reset**. Hold Force Recovery, tap Reset, release.
Or, if running and reachable:

```bash
ssh <user>@<host> 'sudo reboot --force forced-recovery'
```

`lsusb` should show `0955:7023` (AGX Orin in APX) — `just check` confirms.

## Status

Flash-validated on hardware (JetPack 7.2 / L4T r39.2, NVMe boot).

| Check          | Result              |
|----------------|---------------------|
| L4T / JetPack  | r39.2 / JetPack 7.2 |
| Boot device    | NVMe (`nvme0n1p1`)  |
| Power model    | MAXN SUPER (`-super`) |

Verify `JETSON_ETH_DEV` per unit (AGX Marvell AQtion iface) — see below.
