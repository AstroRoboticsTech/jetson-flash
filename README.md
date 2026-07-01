# jetson-flash

Headless flashing pipeline for NVIDIA Jetson Orin devkits (Tegra234) from a
Linux host. No SDK Manager, no Nix — a Rust CLI + library driving the official
NVIDIA L4T `l4t_initrd_flash.sh` / `apply_binaries.sh`.

The result is a fully headless board that boots straight into a logged-in
shell, joins WiFi + wired LAN automatically, and answers to
`ssh <user>@<hostname>.local`. No display, no first-boot wizard.

> A legacy `just`/bash pipeline (flat `.env`) also lives in-tree — see
> [docs/legacy-just.md](docs/legacy-just.md). The Rust CLI is the primary path.

## Supported boards

| Board                      | JetPack        | Status                       | Doc |
|----------------------------|----------------|------------------------------|-----|
| Jetson Orin Nano 8GB Super | 7.2 (L4T r39.2)| Validated on hardware        | [docs/orin-nano.md](docs/orin-nano.md) |
| Jetson AGX Orin devkit     | 7.2 (L4T r39.2)| Validated on hardware        | [docs/orin-agx.md](docs/orin-agx.md) |

JetPack 6.2.1 (L4T r36.4.4) is available as a profile preset but not yet
hardware-validated.

## Prerequisites

- **Build/install:** `cargo` and `libusb-1.0-0-dev` (the `rusb` link dep) —
  that's it. The crate builds anywhere `rusb` runs (Linux/macOS/Windows), and
  `check` / `profiles` / `init` / `edit` work on any of them.
- **Flashing:** verified on Linux x86_64 — `stage`/`preconfig`/`flash` drive
  NVIDIA's L4T bash tooling (qemu, chroot, `sudo`), and `deps` installs its
  packages via `apt` (Ubuntu 24.04 is the reference host for JetPack 7.2).
  ~10 GB free disk, a USB-C cable.
- **Target:** a Jetson Orin devkit with an NVMe SSD in the M.2 M-key slot (see
  the per-board doc for WiFi card, boot device, and recovery buttons).

## Install

```bash
cargo install jetson-flash          # from crates.io
# or from a checkout:
cargo install --path .
```

## Quick start

```bash
jetson-flash init                   # seed ./jetson-flash.toml (embedded template)
jetson-flash edit                   # edit it in $EDITOR (add/adjust profiles)
jetson-flash profiles               # list board profiles

# Board in APX recovery, then:
JETSON_IDENTITY_PASSWORD=secret \
  jetson-flash --profile orin-nano all
```

`all` runs `deps → fetch → stage → preconfig → check → flash`; each stage is
also a standalone subcommand. Bare `jetson-flash` prints help.

## Configuration

Profile-based TOML: a `[default]` base table plus one `[<name>]` table per
board. Select with `--profile` / `JETSON_PROFILE` (required for every stage).

```toml
[default]
[default.identity]
username = "jetson"
headless = true
autologin = true

[orin-nano]
jetpack = "7.2"                     # "6.2.1" => L4T r36.4.4
[orin-nano.board]
name = "jetson-orin-nano-devkit-super"
external_device = "nvme0n1p1"
[orin-nano.identity]
hostname = "orin-nano"
[orin-nano.network.ethernet]
dev = "enP8p1s0"
static_ip = "10.42.0.10/24"         # "" for DHCP
```

- **`jetpack`** (`"6.2.1"` | `"7.2"`) resolves the L4T version + BSP/rootfs URLs
  from a built-in preset (`7.2` → r39.2 / Ubuntu 24.04; `6.2.1` → r36.4.4 /
  Ubuntu 22.04). Pin custom values with `[<profile>.l4t]`.
- **Discovery:** `--config` → `./jetson-flash.toml` →
  `~/.config/jetson-flash/jetson-flash.toml`. `init` writes to `--config <path>`,
  else `--global` (XDG), else `./`.
- **Secrets** stay out of the file — `JETSON_IDENTITY_PASSWORD`,
  `JETSON_NETWORK_WIFI_PSK`. Any `JETSON_*` var fills a key the profile leaves
  unset.

## Workspace / cache

Downloads, staging, and logs live under a base dir: `--work-dir` /
`JETSON_WORK_DIR` if set, else the repo when run from a checkout (`Cargo.toml`
present), else `~/.cache/jetson-flash`. The layout is namespaced so boards and
JetPack versions never collide:

```text
<base>/
  downloads/<l4t_version>/*.tbz2              # shared across boards
  work/<profile>-<l4t_version>/Linux_for_Tegra
  logs/<profile>-<l4t_version>/<step>-<ts>.log
```

Tarballs are keyed by L4T version only (the BSP is board-agnostic → downloaded
once per version); staging + logs are keyed by profile+version because
`preconfig` bakes board-specific identity into the rootfs. Output is captured
to the per-slot log with a spinner; `-v` streams subprocess output live.

Recovery detection and NetworkManager-keyfile / identity baking are native
Rust; NVIDIA's `l4t_*.sh` / `apply_binaries.sh` and `apt`/`wget`/`tar` are
driven as subprocesses.

## What gets baked into the rootfs

`preconfig` runs against the staged `Linux_for_Tegra/rootfs/` and: creates the
default user (skipping oem-config); sets `multi-user.target` and masks gdm /
oem-config GUI; adds a tty1 autologin override; writes NetworkManager keyfiles
(eth pinned by interface name, WiFi matched by SSID, mode 600, WiFi route metric
200 so eth stays primary); enables `ssh` + `avahi-daemon`; patches `nsswitch`
so `mdns4_minimal` resolves first. It is idempotent — re-runs skip user creation
when the user is already baked.

## Recovery mode

The board must be in APX recovery before `flash`. Button location differs per
board (see the per-board doc). `check` confirms via libusb:

| USB ID       | Board                     |
|--------------|---------------------------|
| `0955:7523`  | Orin Nano in APX recovery |
| `0955:7423`  | Orin NX in APX recovery   |
| `0955:7023`  | AGX Orin in APX recovery  |

`0955:7020` = L4T already running, **not** recovery — re-trigger.

After `flash`: unplug the USB-C data cable, power-cycle, and
`ssh <user>@<hostname>.local`.

## Library

Three moves: load a profile, build the workspace, run stages.

```rust
use jetson_flash::{run_all, run_step, Config, Paths, Step};
use std::path::Path;

let cfg = Config::load(Path::new("jetson-flash.toml"), "orin-nano")?;
let paths = Paths::new(Path::new("."), "orin-nano", cfg.l4t.version());

run_step(Step::Check, &cfg, &paths, false)?;   // one stage
run_all(&cfg, &paths, false)?;                 // deps → fetch → stage → … → flash
```

`run_step` / `run_all` open the per-slot log and run the stage(s); each stage is
also a bare `stages::<name>::run(&Config, &Paths, &Logger)` if you manage the
`Logger` yourself. Errors are a typed enum (`jetson_flash::Error`); recovery
state is `stages::check::UsbState` / `Model`. Full runnable example:
[`examples/pipeline.rs`](examples/pipeline.rs) — `cargo run --example pipeline -- orin-nano`.

## Troubleshooting

- **`sudo: a terminal is required` / `sudo authentication failed`:** the CLI
  validates sudo with `sudo -v`, which needs an interactive terminal. Run from a
  real terminal (it prompts once), or grant NOPASSWD sudo for headless/CI.
  `check`, `profiles`, `init`, `edit`, `fetch` need no sudo; `deps`, `stage`,
  `preconfig`, `flash` do.
- **Board detected as `0955:7020`, "not in APX recovery":** it booted L4T
  instead of recovery. Re-enter recovery (hold FC, tap RST) so it enumerates as
  `0955:7523` (Nano) / `0955:7023` (AGX).
- **Flash hangs / USB drops mid-transfer:** autosuspend or a flaky cable/hub.
  Use a direct host USB port, a data-rated cable; `echo -1 | sudo tee
  /sys/module/usbcore/parameters/autosuspend` before flashing.
- **`<hostname>.local` won't resolve from the host:** the host also needs avahi.
- **AppArmor blocks the initrd NFS transfer:** `sudo systemctl stop apparmor`.

## License

Apache-2.0.
