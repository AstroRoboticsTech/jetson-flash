use crate::{
    error::{RecoveryError, Result},
    logging::Logger,
    Config, Paths,
};

const NVIDIA: u16 = 0x0955;

/// Jetson USB state on the T234 family, as seen from the flashing host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbState {
    /// In APX recovery, ready to flash (with the detected model).
    Recovery(Model),
    /// Booted and running L4T (RNDIS, product 0x7020) — NOT flashable.
    RunningL4t,
    /// No Jetson on the USB bus.
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    AgxOrin,
    OrinNx,
    OrinNano,
}

impl Model {
    fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            0x7023 => Some(Model::AgxOrin),
            0x7423 => Some(Model::OrinNx),
            0x7523 => Some(Model::OrinNano),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Model::AgxOrin => "Jetson AGX Orin",
            Model::OrinNx => "Jetson Orin NX",
            Model::OrinNano => "Jetson Orin Nano",
        }
    }
}

/// Probe the USB bus (native libusb, no `lsusb` dependency).
pub fn detect() -> Result<UsbState> {
    let mut running = false;
    for dev in rusb::devices()?.iter() {
        let Ok(d) = dev.device_descriptor() else {
            continue;
        };
        if d.vendor_id() != NVIDIA {
            continue;
        }
        if let Some(model) = Model::from_pid(d.product_id()) {
            return Ok(UsbState::Recovery(model));
        }
        if d.product_id() == 0x7020 {
            running = true;
        }
    }
    Ok(if running {
        UsbState::RunningL4t
    } else {
        UsbState::Absent
    })
}

pub fn run(_cfg: &Config, _paths: &Paths, log: &Logger) -> Result<()> {
    match detect()? {
        UsbState::Recovery(model) => {
            let pid = match model {
                Model::AgxOrin => "0955:7023",
                Model::OrinNx => "0955:7423",
                Model::OrinNano => "0955:7523",
            };
            log.ok(&format!("{} in APX recovery ({pid}).", model.name()));
            Ok(())
        }
        UsbState::RunningL4t => {
            log.err("Jetson is running L4T (0955:7020), not in recovery.");
            log.info("Hold the REC (recovery) button while pressing RST (reset). Then re-run.");
            Err(RecoveryError::RunningL4t.into())
        }
        UsbState::Absent => {
            log.err("No Jetson detected on USB. Power on the board, hold REC, tap RST.");
            Err(RecoveryError::Absent.into())
        }
    }
}
