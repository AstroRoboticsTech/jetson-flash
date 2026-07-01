use crate::{error::Result, logging::Logger, Config, Paths};
use std::fs;
use std::path::Path;

const PKGS: &[&str] = &[
    "qemu-user-static",
    "lz4",
    "libxml2-utils",
    "abootimg",
    "sshpass",
    "python3",
    "python3-yaml",
    "python3-setuptools",
    "binfmt-support",
    "nfs-kernel-server",
    "dosfstools",
    "uuid-runtime",
    "wget",
    "curl",
    "ca-certificates",
];

pub fn run(_cfg: &Config, _paths: &Paths, log: &Logger) -> Result<()> {
    match fs::read_to_string("/etc/os-release") {
        Ok(s) if s.contains("VERSION_ID=\"24.04\"") => {}
        _ => log.warn("Host is not Ubuntu 24.04. Continuing anyway."),
    }

    log.sudo_validate()?;
    log.step("apt-get update");
    log.run("sudo", &["apt-get", "update"])?;

    log.step(&format!("apt-get install ({} packages)", PKGS.len()));
    let mut args = vec!["apt-get", "install", "-y"];
    args.extend_from_slice(PKGS);
    log.run("sudo", &args)?;

    if !Path::new("/proc/sys/fs/binfmt_misc/qemu-aarch64").exists() {
        log.info("Registering qemu-aarch64 binfmt handler.");
        let _ = log.run("sudo", &["systemctl", "restart", "systemd-binfmt"]);
    }

    log.ok("Host dependencies installed.");
    Ok(())
}
