use crate::error::Result;
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
    pub password: String,
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
    /// Load from a TOML file, with `JETSON_`-prefixed env vars overriding.
    /// e.g. `JETSON_BOARD_NAME=... JETSON_IDENTITY_HOSTNAME=...`.
    pub fn load(path: &Path) -> Result<Self> {
        Ok(Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("JETSON_").split("_"))
            .extract()?)
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

/// Default config path: ./jetson-flash.toml under the given repo root.
pub fn default_config_path(repo_root: &Path) -> PathBuf {
    repo_root.join("jetson-flash.toml")
}
