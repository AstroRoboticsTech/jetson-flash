use crate::error::{Error, Result};
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub l4t: L4t,
    pub board: Board,
    pub identity: Identity,
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub services: Services,
}

#[derive(Debug, Clone, Deserialize)]
pub struct L4t {
    pub version: String,
    pub bsp_url: String,
    pub rootfs_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Board {
    pub name: String,
    pub external_device: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Identity {
    pub username: String,
    /// Kept out of the config file; supply via `JETSON_IDENTITY_PASSWORD`.
    #[serde(default)]
    pub password: Option<String>,
    pub hostname: String,
    #[serde(default = "yes")]
    pub headless: bool,
    #[serde(default = "yes")]
    pub autologin: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Network {
    #[serde(default)]
    pub ethernet: Ethernet,
    #[serde(default)]
    pub wifi: Wifi,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ethernet {
    #[serde(default = "nano_eth")]
    pub dev: String,
    #[serde(default)]
    pub static_ip: String,
    #[serde(default)]
    pub gateway: String,
    #[serde(default = "default_dns")]
    pub dns: Vec<String>,
}

impl Default for Ethernet {
    fn default() -> Self {
        Self {
            dev: nano_eth(),
            static_ip: String::new(),
            gateway: String::new(),
            dns: default_dns(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Wifi {
    #[serde(default)]
    pub dev: String,
    #[serde(default)]
    pub ssid: String,
    #[serde(default)]
    pub psk: String,
    #[serde(default = "yes")]
    pub dhcp: bool,
    #[serde(default)]
    pub static_ip: String,
    #[serde(default)]
    pub gateway: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Services {
    #[serde(default = "yes")]
    pub avahi: bool,
}

impl Default for Services {
    fn default() -> Self {
        Self { avahi: true }
    }
}

fn yes() -> bool {
    true
}
fn nano_eth() -> String {
    "enP8p1s0".to_string()
}
fn default_dns() -> Vec<String> {
    vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()]
}

impl Config {
    /// Load the given profile from a nested-TOML config: the `[default]` table
    /// is the shared base, and `[<profile>]` overrides it. `JETSON_`-prefixed
    /// env vars fill any key the profile leaves unset (e.g. secrets like
    /// `JETSON_IDENTITY_PASSWORD`). Errors if `profile` is not a defined table.
    pub fn load(path: &Path, profile: &str) -> Result<Self> {
        let fig = base_figment(path);
        let known = profile_names(&fig);
        if !known.iter().any(|p| p == profile) {
            return Err(Error::UnknownProfile {
                name: profile.to_string(),
                known,
            });
        }
        Ok(fig.select(profile).extract()?)
    }

    /// Profile names defined in the config file (excludes `default`).
    pub fn profiles(path: &Path) -> Vec<String> {
        profile_names(&base_figment(path))
    }

    /// DNS as a NetworkManager keyfile list ("1.1.1.1;8.8.8.8;").
    pub fn dns_nm(&self) -> String {
        let mut s = self.network.ethernet.dns.join(";");
        if !s.is_empty() {
            s.push(';');
        }
        s
    }
}

/// The canonical config, embedded at build time so an installed binary can
/// seed one (`jetson-flash init`). Ships the shared `[default]` base + the
/// board presets, and evolves with each release.
pub const TEMPLATE: &str = include_str!("../jetson-flash.toml");

/// `$XDG_CONFIG_HOME/jetson-flash/jetson-flash.toml` (falls back to
/// `$HOME/.config/...`). `None` if neither env var is set.
pub fn xdg_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("jetson-flash").join("jetson-flash.toml"))
}

/// Resolve which config file to use: explicit `--config`, else
/// `<repo_root>/jetson-flash.toml`, else the XDG path. Returns the local path
/// as the default even when nothing exists, so callers can report it.
pub fn resolve_config_path(explicit: Option<PathBuf>, repo_root: &Path) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    let local = default_config_path(repo_root);
    if local.exists() {
        return local;
    }
    match xdg_config_path() {
        Some(x) if x.exists() => x,
        _ => local,
    }
}

/// Nested-TOML + env figment, before a profile is selected.
fn base_figment(path: &Path) -> Figment {
    Figment::new()
        .merge(Toml::file(path).nested())
        .merge(Env::prefixed("JETSON_").split("_"))
}

fn profile_names(fig: &Figment) -> Vec<String> {
    fig.profiles()
        .map(|p| p.to_string())
        .filter(|p| p != "default")
        .collect()
}

/// Default config path: ./jetson-flash.toml under the given repo root.
pub fn default_config_path(repo_root: &Path) -> PathBuf {
    repo_root.join("jetson-flash.toml")
}
