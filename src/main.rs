use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use jetson_flash::{config, logging::Logger, stages, Config, Paths};
use std::path::PathBuf;

/// Headless Jetson Orin (Tegra234) flashing pipeline.
#[derive(Parser)]
#[command(name = "jetson-flash", version, about)]
struct Cli {
    /// Config file (default: <repo-root>/jetson-flash.toml). JETSON_* env vars override.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Repo/work root (default: current directory).
    #[arg(long, global = true)]
    repo_root: Option<PathBuf>,

    /// Stream all subprocess output live instead of capturing it to the log.
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install host apt dependencies (Ubuntu 24.04).
    Deps,
    /// Download BSP + sample rootfs tarballs into work/.
    Fetch,
    /// Extract BSP, populate rootfs, run apply_binaries.sh.
    Stage,
    /// Bake user/password/hostname/headless/IPs/WiFi/avahi into the rootfs.
    Preconfig,
    /// Verify a Jetson is in APX recovery (native libusb).
    Check,
    /// Flash to NVMe (initrd flash + internal QSPI).
    Flash,
    /// Full pipeline: deps -> fetch -> stage -> preconfig -> check -> flash.
    All,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let repo_root = match cli.repo_root {
        Some(p) => p,
        None => std::env::current_dir().context("resolve current dir")?,
    };
    let paths = Paths::new(&repo_root);
    let config_path = cli
        .config
        .unwrap_or_else(|| config::default_config_path(&repo_root));
    let cfg = Config::load(&config_path)?;
    let run = Runner {
        cfg: &cfg,
        paths: &paths,
        verbose: cli.verbose,
    };

    match cli.cmd {
        Cmd::Deps => run.step("deps", stages::deps::run),
        Cmd::Fetch => run.step("fetch", stages::fetch::run),
        Cmd::Stage => run.step("stage", stages::stage::run),
        Cmd::Preconfig => run.step("preconfig", stages::preconfig::run),
        Cmd::Check => run.step("check", stages::check::run),
        Cmd::Flash => run.step("flash", stages::flash::run),
        Cmd::All => {
            run.step("deps", stages::deps::run)?;
            run.step("fetch", stages::fetch::run)?;
            run.step("stage", stages::stage::run)?;
            run.step("preconfig", stages::preconfig::run)?;
            run.step("check", stages::check::run)?;
            run.step("flash", stages::flash::run)
        }
    }
}

struct Runner<'a> {
    cfg: &'a Config,
    paths: &'a Paths,
    verbose: bool,
}

impl Runner<'_> {
    fn step(
        &self,
        name: &str,
        f: impl Fn(&Config, &Paths, &Logger) -> jetson_flash::Result<()>,
    ) -> Result<()> {
        let log = Logger::init(name, &self.paths.repo_root, self.verbose)?;
        Ok(f(self.cfg, self.paths, &log)?)
    }
}
