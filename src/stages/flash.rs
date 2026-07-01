use crate::{
    error::{Error, Result},
    logging::Logger,
    Config, Paths,
};

pub fn run(cfg: &Config, paths: &Paths, log: &Logger) -> Result<()> {
    let script = paths.l4t_dir.join("tools/kernel_flash/l4t_initrd_flash.sh");
    if !script.is_file() {
        return Err(Error::Missing(format!(
            "{} missing. Run `stage` first.",
            script.display()
        )));
    }

    log.sudo_validate()?;
    log.step(&format!(
        "flash board={} device={} (NVMe + internal QSPI; ~15-25 min)",
        cfg.board.name, cfg.board.external_device
    ));

    // Mirrors scripts/flash-nvme.sh. The `-p` value is one argv token.
    // Live stdio: the flash is long and its output matters when it fails.
    log.run_live(
        "sudo",
        &[
            "./tools/kernel_flash/l4t_initrd_flash.sh",
            "--external-device",
            &cfg.board.external_device,
            "-c",
            "tools/kernel_flash/flash_l4t_t234_nvme.xml",
            "-p",
            "-c bootloader/generic/cfg/flash_t234_qspi.xml",
            "--showlogs",
            "--network",
            "usb0",
            &cfg.board.name,
            "internal",
        ],
        &paths.l4t_dir,
    )?;

    log.ok("Flash complete. Disconnect USB-C and power-cycle; the board boots headless to the baked user.");
    Ok(())
}
