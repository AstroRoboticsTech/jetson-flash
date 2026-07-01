use crate::{
    error::{Error, Result},
    logging::Logger,
    Config, Paths,
};
use std::fs;
use std::path::{Path, PathBuf};

pub fn run(_cfg: &Config, paths: &Paths, log: &Logger) -> Result<()> {
    let bsp = find_tarball(&paths.work, "Jetson_Linux_R", "_aarch64.tbz2").ok_or_else(|| {
        Error::Missing(
            "BSP tarball (Jetson_Linux_R*_aarch64.tbz2) not found in work/; run `fetch`".into(),
        )
    })?;
    let rootfs = find_tarball(
        &paths.work,
        "Tegra_Linux_Sample-Root-Filesystem_R",
        "_aarch64.tbz2",
    )
    .ok_or_else(|| Error::Missing("rootfs tarball not found in work/; run `fetch`".into()))?;

    log.sudo_validate()?;

    if paths.l4t_dir.exists() {
        log.info("reusing existing Linux_for_Tegra/ (BSP already extracted)");
    } else {
        log.step("extract BSP -> Linux_for_Tegra/");
        log.run_in("tar", &["xf", bsp.to_str().unwrap()], &paths.work)?;
    }

    let l4t = paths.l4t_dir.to_str().unwrap();

    if paths.rootfs().join("etc/os-release").exists() {
        log.info("rootfs already extracted; skipping");
    } else {
        log.step("extract sample rootfs -> rootfs/");
        log.run_in(
            "sudo",
            &["tar", "xpf", rootfs.to_str().unwrap(), "-C", "rootfs/"],
            &paths.l4t_dir,
        )?;
    }

    if paths
        .l4t_dir
        .join("tools/l4t_flash_prerequisites.sh")
        .is_file()
    {
        log.step("l4t_flash_prerequisites.sh");
        log.run_in(
            "sudo",
            &["./tools/l4t_flash_prerequisites.sh"],
            &paths.l4t_dir,
        )?;
    }

    if paths
        .rootfs()
        .join("usr/lib/aarch64-linux-gnu/tegra")
        .exists()
    {
        log.info("NVIDIA binaries already applied; skipping apply_binaries.sh");
    } else {
        log.step("apply_binaries.sh");
        log.run_in("sudo", &["./apply_binaries.sh"], &paths.l4t_dir)?;
    }

    log.ok(&format!("Rootfs staged at {l4t}/rootfs"));
    Ok(())
}

/// First entry in `dir` whose name starts with `prefix` and ends with `suffix`.
fn find_tarball(dir: &Path, prefix: &str, suffix: &str) -> Option<PathBuf> {
    let mut hits: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix) && n.ends_with(suffix))
        })
        .collect();
    hits.sort();
    hits.into_iter().next()
}
