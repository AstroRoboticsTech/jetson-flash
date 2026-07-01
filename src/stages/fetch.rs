use crate::{
    error::{IoContext, Result},
    logging::Logger,
    Config, Paths,
};
use std::fs;

pub fn run(cfg: &Config, paths: &Paths, log: &Logger) -> Result<()> {
    fs::create_dir_all(&paths.work).ctx(|| format!("mkdir {}", paths.work.display()))?;

    fetch_one(log, &paths.work, &cfg.l4t.bsp_url)?;
    fetch_one(log, &paths.work, &cfg.l4t.rootfs_url)?;

    log.ok(&format!("Tarballs in {}.", paths.work.display()));
    Ok(())
}

fn fetch_one(log: &Logger, work: &std::path::Path, url: &str) -> Result<()> {
    let file = url.rsplit('/').next().unwrap_or(url);
    let dest = work.join(file);
    if dest.exists() {
        log.info(&format!("skip {file} (already present)"));
        return Ok(());
    }
    log.step(&format!("fetch {url}"));
    // Live stdio so wget's own progress bar shows through (resumable download).
    log.run_live("wget", &["--content-disposition", "-O", file, url], work)
}
