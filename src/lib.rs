//! Headless Jetson Orin (Tegra234) flashing pipeline as a library.
//!
//! Rust port of the `just`/bash pipeline. Other Rust commissioning tooling
//! drives it in three moves: load a [`Config`] profile, build the [`Paths`]
//! workspace, then run stages. [`run_all`] runs the whole
//! deps→fetch→stage→preconfig→check→flash sequence; [`run_step`] runs one.
//!
//! ```no_run
//! use jetson_flash::{Config, Paths, Step, run_all, run_step};
//! use std::path::Path;
//!
//! let cfg = Config::load(Path::new("jetson-flash.toml"), "orin-nano")?;
//! let paths = Paths::new(Path::new("."), "orin-nano", cfg.l4t.version());
//!
//! run_step(Step::Check, &cfg, &paths, false)?;   // one stage
//! run_all(&cfg, &paths, false)?;                 // download → stage → flash
//! # jetson_flash::Result::Ok(())
//! ```
//!
//! Each stage is also a bare `stages::<name>::run(&Config, &Paths, &Logger)`
//! if you want to manage the [`logging::Logger`] yourself.

pub mod config;
pub mod error;
pub mod logging;
pub mod pipeline;
pub mod stages;

pub use config::Config;
pub use error::{Error, RecoveryError, Result};
pub use pipeline::{run_all, run_step, Step};

use std::path::{Path, PathBuf};
/// Workspace layout under a base directory, namespaced so multiple boards and
/// JetPack versions coexist without collisions:
///
/// ```text
/// <base>/
///   downloads/<l4t_version>/*.tbz2          # shared across boards
///   work/<profile>-<l4t_version>/Linux_for_Tegra
///   logs/<profile>-<l4t_version>/<step>-<ts>.log
/// ```
///
/// In a repo checkout the base is the repo root (dev); when installed it is the
/// cache dir (`~/.cache/jetson-flash`). Downloads are keyed by L4T version only
/// (the BSP is board-agnostic); staging + logs are keyed by profile+version
/// because `preconfig` bakes board-specific identity into the rootfs.
#[derive(Debug, Clone)]
pub struct Paths {
    pub base: PathBuf,
    pub downloads: PathBuf,
    pub work: PathBuf,
    pub l4t_dir: PathBuf,
    pub logs: PathBuf,
}

impl Paths {
    pub fn new(base: &Path, profile: &str, l4t_version: &str) -> Self {
        let base = base.to_path_buf();
        let slot = format!("{profile}-{l4t_version}");
        let downloads = base.join("downloads").join(l4t_version);
        let work = base.join("work").join(&slot);
        let l4t_dir = work.join("Linux_for_Tegra");
        let logs = base.join("logs").join(&slot);
        Self {
            base,
            downloads,
            work,
            l4t_dir,
            logs,
        }
    }

    pub fn rootfs(&self) -> PathBuf {
        self.l4t_dir.join("rootfs")
    }
}
