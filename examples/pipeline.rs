//! Drive the flash pipeline as a library — the three-move pattern other Rust
//! commissioning tooling uses: load a profile, build the workspace, run stages.
//!
//! Run a single stage (default: check that a board is in recovery):
//!     cargo run --example pipeline -- orin-nano
//! Run the whole pipeline (deps → fetch → stage → preconfig → check → flash):
//!     JETSON_IDENTITY_PASSWORD=secret cargo run --example pipeline -- orin-nano all

use jetson_flash::{
    run_all, run_step,
    stages::check::{self, UsbState},
    Config, Paths, Step,
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "orin-nano".into());
    let full = std::env::args().nth(2).as_deref() == Some("all");

    // 1. Load a profile from the TOML config (jetpack/board/network/identity).
    let cfg = Config::load(Path::new("jetson-flash.toml"), &profile)?;

    // 2. Build the workspace: downloads/staging/logs keyed by profile + L4T version.
    let paths = Paths::new(Path::new("."), &profile, cfg.l4t.version());

    if full {
        // 3a. Whole pipeline, stopping at the first error. Logs land under
        //     <base>/logs/<profile>-<version>/.
        run_all(&cfg, &paths, /* verbose */ false)?;
    } else {
        // 3b. One stage at a time — here, recovery detection.
        run_step(Step::Check, &cfg, &paths, false)?;

        // Or call a stage's detection directly, without running the CLI stage:
        match check::detect()? {
            UsbState::Recovery(model) => println!("ready: {} in recovery", model.name()),
            UsbState::RunningL4t => println!("board is running L4T, not recovery"),
            UsbState::Absent => println!("no board on USB"),
        }
    }
    Ok(())
}
