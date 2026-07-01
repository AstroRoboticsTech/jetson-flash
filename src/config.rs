use crate::error::{Error, Result};
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// JetPack release the profile targets (`"6.2"` or `"7.2"`). Fills the
    /// `l4t` version + URLs from a built-in preset; explicit `l4t.*` overrides.
    #[serde(default)]
    pub jetpack: Option<String>,
    #[serde(default)]
    pub l4t: L4t,
    pub board: Board,
    pub identity: Identity,
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub services: Services,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct L4t {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub bsp_url: Option<String>,
    #[serde(default)]
    pub rootfs_url: Option<String>,
}

impl L4t {
    pub fn version(&self) -> &str {
        self.version.as_deref().unwrap_or_default()
    }
    pub fn bsp_url(&self) -> &str {
        self.bsp_url.as_deref().unwrap_or_default()
    }
    pub fn rootfs_url(&self) -> &str {
        self.rootfs_url.as_deref().unwrap_or_default()
    }
}

/// A JetPack release mapped to its L4T version and download URLs.
struct JetpackPreset {
    version: &'static str,
    bsp_url: &'static str,
    rootfs_url: &'static str,
}

fn jetpack_preset(v: &str) -> Option<JetpackPreset> {
    match v {
        // JetPack 7.2 => L4T r39.2 (Ubuntu 24.04 rootfs, kernel 6.8).
        "7.2" => Some(JetpackPreset {
            version: "39.2.0",
            bsp_url: "https://developer.nvidia.com/downloads/embedded/L4T/r39_Release_v2.0/release/Jetson_Linux_R39.2.0_aarch64.tbz2",
            rootfs_url: "https://developer.nvidia.com/downloads/embedded/L4T/r39_Release_v2.0/release/Tegra_Linux_Sample-Root-Filesystem_R39.2.0_aarch64.tbz2",
        }),
        // JetPack 6.2.1 => L4T r36.4.4 (Ubuntu 22.04 rootfs).
        "6.2.1" => Some(JetpackPreset {
            version: "36.4.4",
            bsp_url: "https://developer.nvidia.com/downloads/embedded/L4T/r36_Release_v4.4/release/Jetson_Linux_R36.4.4_aarch64.tbz2",
            rootfs_url: "https://developer.nvidia.com/downloads/embedded/L4T/r36_Release_v4.4/release/Tegra_Linux_Sample-Root-Filesystem_R36.4.4_aarch64.tbz2",
        }),
        _ => None,
    }
}

/// JetPack releases with a built-in preset.
pub fn known_jetpacks() -> Vec<String> {
    vec!["6.2.1".to_string(), "7.2".to_string()]
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
        let mut cfg: Config = fig.select(profile).extract()?;
        cfg.resolve_l4t()?;
        Ok(cfg)
    }

    /// Fill `l4t` from the `jetpack` preset (explicit `l4t.*` wins), then
    /// verify all three L4T fields are resolved.
    fn resolve_l4t(&mut self) -> Result<()> {
        if let Some(jp) = self.jetpack.clone() {
            let p = jetpack_preset(&jp).ok_or_else(|| Error::UnknownJetpack {
                name: jp,
                known: known_jetpacks(),
            })?;
            let l = &mut self.l4t;
            l.version.get_or_insert_with(|| p.version.to_string());
            l.bsp_url.get_or_insert_with(|| p.bsp_url.to_string());
            l.rootfs_url.get_or_insert_with(|| p.rootfs_url.to_string());
        }
        if self.l4t.version.is_none() || self.l4t.bsp_url.is_none() || self.l4t.rootfs_url.is_none()
        {
            return Err(Error::Missing(
                "l4t unresolved: set `jetpack = \"7.2\"` (or 6.2.1) on the profile, \
                 or l4t.version/bsp_url/rootfs_url explicitly"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Profile names defined in the config file (excludes `default`).
    pub fn profiles(path: &Path) -> Vec<String> {
        profile_names(&base_figment(path))
    }

    /// DNS servers joined for a NetworkManager keyfile ("1.1.1.1;8.8.8.8").
    /// The keyfile templates append the single trailing `;`.
    pub fn dns_nm(&self) -> String {
        self.network.ethernet.dns.join(";")
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

/// `$XDG_CACHE_HOME/jetson-flash` (falls back to `$HOME/.cache/jetson-flash`).
/// Where an installed binary keeps its downloads + staging. `None` if neither
/// env var is set.
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("jetson-flash"))
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
