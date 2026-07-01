//! Stage orchestration for library consumers. Each [`Step`] is one stage of
//! the flash pipeline; [`run_step`] sets up the per-slot log and runs it, and
//! [`run_all`] runs the whole sequence in order. The binary uses these too.

use crate::{logging::Logger, stages, Config, Paths, Result};

/// One stage of the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Install host apt dependencies.
    Deps,
    /// Download BSP + sample rootfs tarballs.
    Fetch,
    /// Extract BSP, populate rootfs, run `apply_binaries.sh`.
    Stage,
    /// Bake identity / headless / network into the rootfs.
    Preconfig,
    /// Verify the board is in APX recovery.
    Check,
    /// Flash to NVMe + internal QSPI.
    Flash,
}

/// The full pipeline in execution order (what `run_all` does).
pub const ALL: [Step; 6] = [
    Step::Deps,
    Step::Fetch,
    Step::Stage,
    Step::Preconfig,
    Step::Check,
    Step::Flash,
];

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::Deps => "deps",
            Step::Fetch => "fetch",
            Step::Stage => "stage",
            Step::Preconfig => "preconfig",
            Step::Check => "check",
            Step::Flash => "flash",
        }
    }

    fn func(self) -> fn(&Config, &Paths, &Logger) -> Result<()> {
        match self {
            Step::Deps => stages::deps::run,
            Step::Fetch => stages::fetch::run,
            Step::Stage => stages::stage::run,
            Step::Preconfig => stages::preconfig::run,
            Step::Check => stages::check::run,
            Step::Flash => stages::flash::run,
        }
    }
}

/// Run one stage: open its per-slot log (`<base>/logs/<profile>-<ver>/`) and
/// execute it. `verbose` streams subprocess output live instead of capturing.
pub fn run_step(step: Step, cfg: &Config, paths: &Paths, verbose: bool) -> Result<()> {
    let log = Logger::init(step.name(), &paths.logs, verbose)?;
    step.func()(cfg, paths, &log)
}

/// Run the whole pipeline in order, stopping at the first error.
pub fn run_all(cfg: &Config, paths: &Paths, verbose: bool) -> Result<()> {
    for step in ALL {
        run_step(step, cfg, paths, verbose)?;
    }
    Ok(())
}
