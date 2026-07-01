use crate::{
    error::{Error, IoContext, Result},
    logging::Logger,
    Config, Paths,
};
use std::fs;
use std::path::{Path, PathBuf};

/// Bake first-boot identity, headless target, static IPs, WiFi, and mDNS into
/// the staged L4T rootfs before flashing. Native port of
/// scripts/preconfigure-rootfs.sh. Networking uses NetworkManager keyfiles
/// (stock Jetson L4T runs NM; netplan/networkd YAML is ignored on first boot).
pub fn run(cfg: &Config, paths: &Paths, log: &Logger) -> Result<()> {
    let rootfs = paths.rootfs();
    if !rootfs.join("etc").is_dir() {
        return Err(Error::Missing(format!(
            "{} not staged. Run `stage` first.",
            rootfs.display()
        )));
    }
    log.sudo_validate()?;

    // 1. Pre-create user, skip oem-config wizard. Not idempotent, so guard on
    // whether the user is already baked into the rootfs (safe to re-run).
    if user_baked(&rootfs, &cfg.identity.username) {
        log.info(&format!(
            "user {} already baked into rootfs; skipping create_default_user",
            cfg.identity.username
        ));
    } else {
        log.step(&format!(
            "creating default user {} on host {}",
            cfg.identity.username, cfg.identity.hostname
        ));
        log.run_in(
            "sudo",
            &[
                "./tools/l4t_create_default_user.sh",
                "-u",
                &cfg.identity.username,
                "-p",
                &cfg.identity.password,
                "-n",
                &cfg.identity.hostname,
                "--accept-license",
            ],
            &paths.l4t_dir,
        )?;
    }

    let sys = rootfs.join("etc/systemd/system");

    // 2. Headless: multi-user.target + mask display managers.
    if cfg.identity.headless {
        log.step("headless mode: multi-user.target + mask gdm3");
        symlink(log, "/lib/systemd/system/multi-user.target", &sys.join("default.target"))?;
        symlink(log, "/dev/null", &sys.join("gdm3.service"))?;
        symlink(log, "/dev/null", &sys.join("gdm.service"))?;
        // oem-config gui may be absent; ignore failure.
        let _ = symlink(log, "/dev/null", &sys.join("nv-oem-config-gui.service"));
    }

    // 3. tty1 autologin.
    if cfg.identity.autologin {
        log.step(&format!("enabling tty1 autologin for {}", cfg.identity.username));
        let dir = sys.join("getty@tty1.service.d");
        log.run("sudo", &["mkdir", "-p", dir.to_str().unwrap()])?;
        let override_conf = format!(
            "[Service]\nExecStart=\nExecStart=-/sbin/agetty --autologin {} --noclear %I $TERM\n",
            cfg.identity.username
        );
        write_root_file(log, &dir.join("override.conf"), "644", &override_conf)?;
    }

    // 4. Networking via NetworkManager keyfiles.
    let nm_dir = rootfs.join("etc/NetworkManager/system-connections");
    let dns_nm = cfg.dns_nm();

    // 4a. Ethernet static IP.
    let eth = &cfg.network.ethernet;
    if !eth.static_ip.is_empty() {
        log.step(&format!("eth static {} on {}", eth.static_ip, eth.dev));
        log.run("sudo", &["mkdir", "-p", nm_dir.to_str().unwrap()])?;
        let (addr, never_default) = if eth.gateway.is_empty() {
            (eth.static_ip.clone(), "never-default=true")
        } else {
            (format!("{},{}", eth.static_ip, eth.gateway), "")
        };
        let content = format!(
            "[connection]\nid=eth-static\ntype=ethernet\nuuid={}\ninterface-name={}\nautoconnect=true\n\n\
             [ethernet]\n\n\
             [ipv4]\nmethod=manual\naddress1={}\ndns={};\n{}\n\n\
             [ipv6]\nmethod=auto\n",
            new_uuid(),
            eth.dev,
            addr,
            dns_nm,
            never_default,
        );
        write_root_file(log, &nm_dir.join("eth-static.nmconnection"), "600", &content)?;
    }

    // 4b. WiFi.
    let wifi = &cfg.network.wifi;
    if !wifi.ssid.is_empty() {
        if wifi.psk.is_empty() {
            return Err(Error::WifiPskMissing);
        }
        log.run("sudo", &["mkdir", "-p", nm_dir.to_str().unwrap()])?;
        let iface_line = if wifi.dev.is_empty() {
            String::new()
        } else {
            format!("interface-name={}", wifi.dev)
        };
        let ipv4_block = if !wifi.static_ip.is_empty() {
            log.step(&format!("WiFi {} static {} (metric 200)", wifi.ssid, wifi.static_ip));
            let addr = if wifi.gateway.is_empty() {
                wifi.static_ip.clone()
            } else {
                format!("{},{}", wifi.static_ip, wifi.gateway)
            };
            format!("[ipv4]\nmethod=manual\naddress1={}\ndns={};\nroute-metric=200", addr, dns_nm)
        } else if wifi.dhcp {
            log.step(&format!("WiFi {} DHCP (metric 200)", wifi.ssid));
            "[ipv4]\nmethod=auto\nroute-metric=200".to_string()
        } else {
            "[ipv4]\nmethod=disabled".to_string()
        };
        let content = format!(
            "[connection]\nid=wifi-home\ntype=wifi\nuuid={}\n{}\nautoconnect=true\n\n\
             [wifi]\nmode=infrastructure\nssid={}\n\n\
             [wifi-security]\nkey-mgmt=wpa-psk\npsk={}\n\n\
             {}\n\n\
             [ipv6]\nmethod=auto\n",
            new_uuid(),
            iface_line,
            wifi.ssid,
            wifi.psk,
            ipv4_block,
        );
        write_root_file(log, &nm_dir.join("wifi-home.nmconnection"), "600", &content)?;
    }

    // 4c. mDNS / avahi.
    if cfg.services.avahi {
        log.step(&format!("enabling avahi-daemon (mDNS for {}.local)", cfg.identity.hostname));
        enable_service(log, &rootfs, "avahi-daemon.service")?;
        patch_nsswitch(log, &rootfs)?;
    }

    // 5. SSH.
    log.step("enabling ssh");
    enable_service(log, &rootfs, "ssh.service")?;

    log.ok("Rootfs preconfigured. Ready to flash.");
    Ok(())
}

fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Is `user` already present in the rootfs `/etc/passwd`? Read needs root
/// (rootfs is root-owned); relies on the sudo cache primed by `sudo_validate`.
fn user_baked(rootfs: &Path, user: &str) -> bool {
    let passwd = rootfs.join("etc/passwd");
    std::process::Command::new("sudo")
        .args(["grep", "-q", &format!("^{user}:"), passwd.to_str().unwrap()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn symlink(log: &Logger, target: &str, link: &Path) -> Result<()> {
    log.run("sudo", &["ln", "-sf", target, link.to_str().unwrap()])
}

/// Write `content` to a root-owned file with `mode` (via a temp file + install).
fn write_root_file(log: &Logger, dst: &Path, mode: &str, content: &str) -> Result<()> {
    let tmp = temp_path();
    fs::write(&tmp, content).ctx(|| format!("write temp {}", tmp.display()))?;
    let res = log.run(
        "sudo",
        &[
            "install",
            "-m",
            mode,
            "-o",
            "root",
            "-g",
            "root",
            tmp.to_str().unwrap(),
            dst.to_str().unwrap(),
        ],
    );
    let _ = fs::remove_file(&tmp);
    res
}

fn temp_path() -> PathBuf {
    std::env::temp_dir().join(format!("jetson-flash-{}.tmp", uuid::Uuid::new_v4()))
}

/// `chroot rootfs systemctl enable <svc>`, falling back to a wants/ symlink.
fn enable_service(log: &Logger, rootfs: &Path, svc: &str) -> Result<()> {
    let rootfs_s = rootfs.to_str().unwrap();
    let cmd = format!("systemctl enable {svc}");
    if log
        .run("sudo", &["chroot", rootfs_s, "/bin/bash", "-c", &cmd])
        .is_ok()
    {
        return Ok(());
    }
    let link = rootfs
        .join("etc/systemd/system/multi-user.target.wants")
        .join(svc);
    symlink(log, &format!("/lib/systemd/system/{svc}"), &link)
}

fn patch_nsswitch(log: &Logger, rootfs: &Path) -> Result<()> {
    let nss = rootfs.join("etc/nsswitch.conf");
    let Ok(body) = fs::read_to_string(&nss) else {
        return Ok(());
    };
    if body.contains("mdns") {
        return Ok(());
    }
    log.run(
        "sudo",
        &[
            "sed",
            "-i",
            "s/^hosts:.*$/hosts:          files mdns4_minimal [NOTFOUND=return] dns/",
            nss.to_str().unwrap(),
        ],
    )
}
