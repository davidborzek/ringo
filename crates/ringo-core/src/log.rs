use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Process-global log sink. The log is process-global by design (one RE thread
/// serves all UAs, so writes carry no per-agent context). Uninitialized = the
/// log is silent (e.g. ringo-flow without `--log`). The binary picks the
/// destination — ringo-core never writes to a fixed path.
static LOG_SINK: OnceLock<Mutex<Box<dyn Write + Send>>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
        }
    }
}

/// Log to `path` (created, truncated; parent dirs made). First init wins; a
/// failure to open leaves the log silent rather than crashing.
pub fn init_file(path: impl AsRef<Path>) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(f) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
    {
        let _ = LOG_SINK.set(Mutex::new(Box::new(f)));
    }
}

/// Log to stderr. First init wins.
pub fn init_stderr() {
    let _ = LOG_SINK.set(Mutex::new(Box::new(std::io::stderr())));
}

/// Write a single log line. No-op until a sink is initialized.
pub fn write(level: Level, msg: &str) {
    if let Some(mtx) = LOG_SINK.get() {
        if let Ok(mut w) = mtx.lock() {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(w, "[{}] {}: {}", ts, level.as_str(), msg);
        }
    }
}

/// Log with `format!`-style syntax. No-op until a sink is initialized.
///
/// Usage: `rlog!(info, "baresip pid={}", pid);`
#[macro_export]
macro_rules! rlog {
    ($level:ident, $($arg:tt)+) => {
        $crate::log::write($crate::log::Level::$level, &format!($($arg)+))
    };
}
