use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

/// Typed errors for the flashing pipeline. The binary wraps these in
/// `anyhow`; library consumers can match on the variant.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config: {0}")]
    Config(Box<figment::Error>),

    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("usb: {0}")]
    Usb(#[from] rusb::Error),

    #[error("failed to spawn `{cmd}`: {source}")]
    Spawn {
        cmd: String,
        #[source]
        source: std::io::Error,
    },

    #[error("`{cmd}` exited with {status} (see {log})")]
    Command {
        cmd: String,
        status: String,
        log: PathBuf,
    },

    #[error("sudo authentication failed")]
    Sudo,

    /// A required input is missing (tarball, script, or unstaged rootfs).
    #[error("{0}")]
    Missing(String),

    #[error("wifi ssid set but wifi psk is empty (set JETSON_NETWORK_WIFI_PSK)")]
    WifiPskMissing,

    #[error("unknown profile `{name}` (available: {})", .known.join(", "))]
    UnknownProfile { name: String, known: Vec<String> },

    #[error("unknown jetpack `{name}` (known: {})", .known.join(", "))]
    UnknownJetpack { name: String, known: Vec<String> },

    #[error("{0} is required — supply it via environment (e.g. JETSON_IDENTITY_PASSWORD)")]
    MissingSecret(&'static str),

    #[error("Jetson not ready to flash: {0}")]
    Recovery(#[from] RecoveryError),
}

impl From<figment::Error> for Error {
    fn from(e: figment::Error) -> Self {
        Error::Config(Box::new(e))
    }
}

/// Why a board can't be flashed right now.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum RecoveryError {
    #[error("board is running L4T (0955:7020), not in APX recovery")]
    RunningL4t,
    #[error("no Jetson detected on USB")]
    Absent,
}

/// Attach a context string to an `io::Result`, mapping into [`Error::Io`].
pub(crate) trait IoContext<T> {
    fn ctx(self, f: impl FnOnce() -> String) -> Result<T>;
}

impl<T> IoContext<T> for std::result::Result<T, std::io::Error> {
    fn ctx(self, f: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|source| Error::Io {
            context: f(),
            source,
        })
    }
}
