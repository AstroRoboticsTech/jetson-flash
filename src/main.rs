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

    /// Board profile to load from the config (a [<name>] table). Required for
    /// every stage; set once via the JETSON_PROFILE env var if you prefer.
    #[arg(short, long, global = true, env = "JETSON_PROFILE")]
    profile: Option<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write a starter jetson-flash.toml (embedded template) to seed config.
    /// Destination: --config <path> if given, else --global, else ./.
    Init {
        /// Write to the XDG config dir (~/.config/jetson-flash/) instead of ./.
        #[arg(long)]
        global: bool,
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
    /// Open the resolved config file in $EDITOR (to add/edit profiles).
    Edit,
    /// List the board profiles defined in the config.
    Profiles,
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

    if let Cmd::Init { global, force } = cli.cmd {
        return init_config(cli.config.clone(), &repo_root, global, force);
    }

    let config_path = config::resolve_config_path(cli.config.clone(), &repo_root);

    if matches!(cli.cmd, Cmd::Edit) {
        return edit_config(&config_path);
    }

    if matches!(cli.cmd, Cmd::Profiles) {
        let names = Config::profiles(&config_path);
        if names.is_empty() {
            println!("no profiles defined in {}", config_path.display());
        } else {
            println!("profiles in {}:", config_path.display());
            for n in names {
                println!("  {n}");
            }
        }
        return Ok(());
    }

    if !config_path.exists() {
        anyhow::bail!(
            "no config found at {}. Run `jetson-flash init` (or `--global`) to seed one.",
            config_path.display()
        );
    }

    let profile = cli.profile.clone().ok_or_else(|| {
        let known = Config::profiles(&config_path).join(", ");
        anyhow::anyhow!("--profile is required (or set JETSON_PROFILE). Available: {known}")
    })?;
    let cfg = Config::load(&config_path, &profile)?;
    let run = Runner {
        cfg: &cfg,
        paths: &paths,
        verbose: cli.verbose,
    };

    match cli.cmd {
        Cmd::Init { .. } | Cmd::Edit | Cmd::Profiles => unreachable!("handled above"),
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

/// Seed a jetson-flash.toml from the embedded template. Destination:
/// `--config <path>` if given, else `--global` (XDG), else `<repo_root>/`.
fn init_config(
    explicit: Option<PathBuf>,
    repo_root: &std::path::Path,
    global: bool,
    force: bool,
) -> Result<()> {
    let dst = match explicit {
        Some(p) => p,
        None if global => config::xdg_config_path()
            .context("cannot resolve XDG config dir (set XDG_CONFIG_HOME or HOME)")?,
        None => config::default_config_path(repo_root),
    };
    if dst.exists() && !force {
        anyhow::bail!(
            "{} already exists (pass --force to overwrite)",
            dst.display()
        );
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    std::fs::write(&dst, config::TEMPLATE).with_context(|| format!("write {}", dst.display()))?;
    println!("wrote {}", dst.display());
    println!("edit it (or `jetson-flash edit`), then: jetson-flash --profile <name> <cmd>");
    Ok(())
}

/// Open the resolved config in `$EDITOR` (falls back to `$VISUAL`, then `vi`).
fn edit_config(path: &std::path::Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!(
            "no config at {}. Run `jetson-flash init` first.",
            path.display()
        );
    }
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor)
        .arg(path)
        .status()
        .with_context(|| format!("launch editor `{editor}`"))?;
    if !status.success() {
        anyhow::bail!("editor `{editor}` exited with {status}");
    }
    Ok(())
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
