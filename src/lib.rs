//! Headless Jetson Orin (Tegra234) flashing pipeline as a library.
//!
//! Rust port of the `just`/bash pipeline. Each stage is a free function
//! taking `&Config`, `&Paths`, and a `&Logger`, so other Rust commissioning
//! tooling can drive the pipeline programmatically:
//!
//! ```no_run
//! use jetson_flash::{Config, Paths, stages, logging::Logger};
//! use std::path::Path;
//! let paths = Paths::new(Path::new("."));
//! let cfg = Config::load(&paths.repo_root.join("jetson-flash.toml"), "orin-nano")?;
//! let log = Logger::init("check", &paths.repo_root, false)?;
//! stages::check::run(&cfg, &paths, &log)?;
//! # jetson_flash::Result::Ok(())
//! ```

pub mod config;
pub mod error;
pub mod logging;
pub mod stages;

pub use config::Config;
pub use error::{Error, RecoveryError, Result};

use std::path::{Path, PathBuf};

/// Canonical directory layout, derived from the repo root.
#[derive(Debug, Clone)]
pub struct Paths {
    pub repo_root: PathBuf,
    pub work: PathBuf,
    pub l4t_dir: PathBuf,
}

impl Paths {
    pub fn new(repo_root: &Path) -> Self {
        let repo_root = repo_root.to_path_buf();
        let work = repo_root.join("work");
        let l4t_dir = work.join("Linux_for_Tegra");
        Self {
            repo_root,
            work,
            l4t_dir,
        }
    }

    pub fn rootfs(&self) -> PathBuf {
        self.l4t_dir.join("rootfs")
    }
}
