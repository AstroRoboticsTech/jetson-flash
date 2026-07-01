# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning is
[SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0]

First release. A library crate (`jetson_flash`) plus a CLI (`jetson-flash`)
porting the `just`/bash flashing pipeline, kept alongside the existing scripts.

### Added
- CLI with the six pipeline stages plus `all`: `deps`, `fetch`, `stage`,
  `preconfig`, `check`, `flash`.
- Profile-based TOML config: a `[default]` base table plus one `[<name>]`
  table per board (`orin-nano`, `orin-agx`), selected with `--profile` /
  `JETSON_PROFILE` (required for every stage). `profiles` lists them.
- `jetpack` field per profile (`"6.2.1"` / `"7.2"`) that resolves the L4T
  version + BSP/rootfs URLs from a built-in preset; explicit `l4t.*` overrides.
- `init` seeds a `jetson-flash.toml` from a template embedded in the binary —
  destination is `--config <path>`, else `--global` (XDG), else `./`.
- `edit` opens the resolved config in `$EDITOR`.
- Config discovery: `--config` → `./jetson-flash.toml` →
  `~/.config/jetson-flash/jetson-flash.toml`.
- Namespaced workspace/cache: downloads, staging, and logs live under a base
  (`--work-dir`/`JETSON_WORK_DIR`, else the repo when run from a checkout, else
  `~/.cache/jetson-flash`). Downloads keyed by L4T version (`downloads/<ver>/`);
  staging + logs keyed by profile+version (`work|logs/<profile>-<ver>/`) so
  boards and JetPack versions never collide.
- Native USB recovery detection (libusb via `rusb`) exposing typed
  `UsbState` / `Model`; no `lsusb` parsing.
- Native NetworkManager keyfile and first-boot identity baking.
- Typed error enum (`Error`, `RecoveryError`) via `thiserror`; `anyhow` only
  in the binary.
- Clean terminal UX: subprocess output captured to `logs/<step>-<ts>.log`
  with an `indicatif` spinner; `-v/--verbose` streams live; downloads and the
  flash use live stdio; on failure the captured log tail is printed.
- Idempotency guards: `stage` reuses an existing `Linux_for_Tegra/`, rootfs,
  and applied binaries; `preconfig` skips user creation when the user is
  already baked into the rootfs.
- Secrets are kept out of the config file: `identity.password` /
  `network.wifi.psk` come from `JETSON_IDENTITY_PASSWORD` /
  `JETSON_NETWORK_WIFI_PSK`.
- Library pipeline API: `Step`, `run_step`, `run_all` drive stages
  programmatically (open the per-slot log + dispatch); `examples/pipeline.rs`
  is a runnable reference.
- CI (fmt, clippy `-D warnings`, build, test) and a tag-driven crates.io
  publish workflow.

[Unreleased]: https://github.com/AstroRoboticsTech/jetson-flash/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/AstroRoboticsTech/jetson-flash/releases/tag/v1.0.0
