# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning is
[SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

First Rust release. A library crate (`jetson_flash`) plus a CLI (`jetson-flash`)
porting the `just`/bash flashing pipeline, kept alongside the existing scripts.

### Added
- CLI with the six pipeline stages plus `all`: `deps`, `fetch`, `stage`,
  `preconfig`, `check`, `flash`.
- TOML configuration (`jetson-flash.toml`) via figment, with `JETSON_*`
  environment-variable overrides.
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
- CI (fmt, clippy `-D warnings`, build, test) and a tag-driven crates.io
  publish workflow.

[Unreleased]: https://github.com/AstroRoboticsTech/jetson-flash/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/AstroRoboticsTech/jetson-flash/releases/tag/v0.1.0
