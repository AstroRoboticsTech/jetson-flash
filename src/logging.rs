use crate::error::{Error, IoContext, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Per-step logger. By default it runs subprocesses *quietly*: their output
/// is captured to `logs/<step>-<ts>.log` while a spinner shows progress in the
/// terminal, so the console stays clean (unlike the bash pipeline, which dumps
/// everything). `verbose` restores full live streaming. On failure the tail of
/// the captured log is printed automatically.
pub struct Logger {
    color: bool,
    verbose: bool,
    file: Arc<Mutex<File>>,
    path: PathBuf,
    bar: Mutex<Option<ProgressBar>>,
}

struct Ansi {
    reset: &'static str,
    step: &'static str,
    info: &'static str,
    ok: &'static str,
    warn: &'static str,
    err: &'static str,
}

const COLOR: Ansi = Ansi {
    reset: "\x1b[0m",
    step: "\x1b[1;35m",
    info: "\x1b[36m",
    ok: "\x1b[1;32m",
    warn: "\x1b[1;33m",
    err: "\x1b[1;31m",
};
const PLAIN: Ansi = Ansi {
    reset: "",
    step: "",
    info: "",
    ok: "",
    warn: "",
    err: "",
};

impl Logger {
    /// `log_dir` is the exact directory to write `<step>-<ts>.log` into.
    pub fn init(step: &str, log_dir: &Path, verbose: bool) -> Result<Self> {
        let dir = log_dir.to_path_buf();
        fs::create_dir_all(&dir).ctx(|| format!("mkdir {}", dir.display()))?;
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let fname = format!("{step}-{ts}.log");
        let path = dir.join(&fname);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ctx(|| format!("open log {}", path.display()))?;

        let link = dir.join(format!("{step}-latest.log"));
        let _ = fs::remove_file(&link);
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(&fname, &link);

        let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        let logger = Self {
            color,
            verbose,
            file: Arc::new(Mutex::new(file)),
            path,
            bar: Mutex::new(None),
        };
        let shown = std::env::current_dir()
            .ok()
            .and_then(|cwd| logger.path.strip_prefix(&cwd).ok().map(Path::to_path_buf))
            .unwrap_or_else(|| logger.path.clone());
        logger.info(&format!("logging {step} -> {}", shown.display()));
        Ok(logger)
    }

    fn ansi(&self) -> &'static Ansi {
        if self.color {
            &COLOR
        } else {
            &PLAIN
        }
    }

    /// Print one message: routed above the active spinner if one is running,
    /// so the bar never gets corrupted. A plain copy always goes to the log.
    fn emit(&self, tag_color: &str, tag: &str, msg: &str) {
        let a = self.ansi();
        let line = format!("{tag_color}{tag}{} {msg}", a.reset);
        match &*self.bar.lock().unwrap() {
            Some(bar) => bar.println(line),
            None => eprintln!("{line}"),
        }
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{tag} {msg}");
        }
    }

    pub fn step(&self, msg: &str) {
        let a = self.ansi();
        self.emit(a.step, "==>", msg);
    }
    pub fn info(&self, msg: &str) {
        let a = self.ansi();
        self.emit(a.info, "[info]", msg);
    }
    pub fn ok(&self, msg: &str) {
        let a = self.ansi();
        self.emit(a.ok, "[ok]", msg);
    }
    pub fn warn(&self, msg: &str) {
        let a = self.ansi();
        self.emit(a.warn, "[warn]", msg);
    }
    pub fn err(&self, msg: &str) {
        let a = self.ansi();
        self.emit(a.err, "[fail]", msg);
    }

    /// Refresh the sudo credential cache up front (prompts on the terminal),
    /// so later captured sudo commands don't silently block on a password.
    pub fn sudo_validate(&self) -> Result<()> {
        let status = Command::new("sudo")
            .arg("-v")
            .status()
            .map_err(|source| Error::Spawn {
                cmd: "sudo -v".into(),
                source,
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Sudo)
        }
    }

    /// Run a command from the current directory, capturing output (spinner UX).
    /// (Callers that care about cwd use `run_in`; these commands use absolute
    /// paths so cwd is irrelevant.)
    pub fn run(&self, program: &str, args: &[&str]) -> Result<()> {
        self.run_in(program, args, Path::new("."))
    }

    /// Run a command in `cwd`, capturing output to the log with a spinner.
    pub fn run_in(&self, program: &str, args: &[&str], cwd: &Path) -> Result<()> {
        if self.verbose {
            return self.exec(program, args, cwd, Capture::Stream);
        }
        let label = pretty(program, args);
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg} [{elapsed}]")
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
        );
        bar.set_message(label.clone());
        bar.enable_steady_tick(Duration::from_millis(120));
        *self.bar.lock().unwrap() = Some(bar.clone());

        let res = self.exec(program, args, cwd, Capture::LogOnly);

        *self.bar.lock().unwrap() = None;
        bar.finish_and_clear();
        if res.is_err() {
            self.err(&format!("`{label}` failed — tail of {}:", self.rel_log()));
            for line in self.tail(20) {
                eprintln!("    {line}");
            }
        }
        res
    }

    /// Run a command with fully inherited stdio (live terminal, no capture).
    /// Use for steps whose native progress output is worth showing directly —
    /// downloads (`wget` progress bar) and the long flash.
    pub fn run_live(&self, program: &str, args: &[&str], cwd: &Path) -> Result<()> {
        self.exec(program, args, cwd, Capture::Inherit)
    }

    fn exec(&self, program: &str, args: &[&str], cwd: &Path, cap: Capture) -> Result<()> {
        let label = pretty(program, args);
        let mut cmd = Command::new(program);
        cmd.args(args).current_dir(cwd);
        match cap {
            Capture::Inherit => {
                cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
            }
            _ => {
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
            }
        }
        let mut child = cmd.spawn().map_err(|source| Error::Spawn {
            cmd: label.clone(),
            source,
        })?;

        if !matches!(cap, Capture::Inherit) {
            let out = child.stdout.take().unwrap();
            let errp = child.stderr.take().unwrap();
            let echo = matches!(cap, Capture::Stream);
            let (f1, f2) = (Arc::clone(&self.file), Arc::clone(&self.file));
            let t1 = std::thread::spawn(move || pump(out, f1, echo, false));
            let t2 = std::thread::spawn(move || pump(errp, f2, echo, true));
            let _ = t1.join();
            let _ = t2.join();
        }

        let status = child.wait().map_err(|source| Error::Spawn {
            cmd: label.clone(),
            source,
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Command {
                cmd: label,
                status: status.to_string(),
                log: self.path.clone(),
            })
        }
    }

    fn rel_log(&self) -> String {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| self.path.strip_prefix(&cwd).ok().map(Path::to_path_buf))
            .unwrap_or_else(|| self.path.clone())
            .display()
            .to_string()
    }

    /// Last `n` lines of the captured log (best-effort, for error context).
    fn tail(&self, n: usize) -> Vec<String> {
        let mut s = String::new();
        if let Ok(mut f) = File::open(&self.path) {
            let _ = f.read_to_string(&mut s);
        }
        let lines: Vec<&str> = s.lines().collect();
        lines[lines.len().saturating_sub(n)..]
            .iter()
            .map(|l| l.to_string())
            .collect()
    }
}

#[derive(Clone, Copy)]
enum Capture {
    /// Capture to the log only (quiet; spinner shows progress).
    LogOnly,
    /// Capture to the log AND echo live to the terminal (verbose).
    Stream,
    /// Inherit stdio; child draws directly to the terminal.
    Inherit,
}

fn pretty(program: &str, args: &[&str]) -> String {
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    }
}

fn pump<R: Read>(reader: R, file: Arc<Mutex<File>>, echo: bool, to_stderr: bool) {
    let buf = BufReader::new(reader);
    for line in buf.lines().map_while(std::result::Result::ok) {
        if echo {
            if to_stderr {
                eprintln!("{line}");
            } else {
                println!("{line}");
            }
        }
        if let Ok(mut f) = file.lock() {
            let _ = writeln!(f, "{line}");
        }
    }
}
