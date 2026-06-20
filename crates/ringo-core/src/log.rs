use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

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

/// Open (or create) `/tmp/ringo-{profile}.log` in append mode.
/// Safe to call multiple times — only the first call takes effect.
pub fn init(profile_name: &str) {
    let path = format!("/tmp/ringo-{}.log", profile_name);
    let _ = LOG_FILE.set(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map(Mutex::new)
            .expect("failed to open ringo log file"),
    );
}

/// Write a single log line. No-op before `init()` is called.
pub fn write(level: Level, msg: &str) {
    if let Some(mtx) = LOG_FILE.get() {
        if let Ok(mut f) = mtx.lock() {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(f, "[{}] {}: {}", ts, level.as_str(), msg);
        }
    }
}

/// Log with `format!`-style syntax. No-op before `init()`.
///
/// Usage: `rlog!(info, "baresip pid={}", pid);`
#[macro_export]
macro_rules! rlog {
    ($level:ident, $($arg:tt)+) => {
        $crate::log::write($crate::log::Level::$level, &format!($($arg)+))
    };
}
