//! Lightweight structured runtime logging.
//!
//! PadProxy emits leveled, timestamped log lines to stderr so remap sessions
//! can be diagnosed without a heavyweight logging dependency. The active level
//! is read once from the `PADPROXY_LOG` environment variable (default `info`).
//!
//! Lines are formatted as `[<unix_ms>] <LEVEL> <target>: <message>` so they are
//! easy to grep and machine-parse.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }

    /// Parse a level name. Unknown values (and `off`) return `None`.
    pub fn parse(value: &str) -> Option<LogLevel> {
        match value.trim().to_ascii_lowercase().as_str() {
            "error" | "err" => Some(LogLevel::Error),
            "warn" | "warning" => Some(LogLevel::Warn),
            "info" => Some(LogLevel::Info),
            "debug" => Some(LogLevel::Debug),
            "trace" => Some(LogLevel::Trace),
            _ => None,
        }
    }
}

static MAX_LEVEL: OnceLock<Option<LogLevel>> = OnceLock::new();

/// The maximum level emitted, from `PADPROXY_LOG` (default `info`). `off`,
/// empty, or unrecognized-as-off values disable logging.
fn max_level() -> Option<LogLevel> {
    *MAX_LEVEL.get_or_init(|| match std::env::var("PADPROXY_LOG") {
        Ok(value) if value.trim().eq_ignore_ascii_case("off") => None,
        Ok(value) if !value.trim().is_empty() => {
            Some(LogLevel::parse(&value).unwrap_or(LogLevel::Info))
        }
        _ => Some(LogLevel::Info),
    })
}

/// Returns true if a line at `level` would currently be emitted.
pub fn enabled(level: LogLevel) -> bool {
    max_level().map(|max| level <= max).unwrap_or(false)
}

/// Format a structured log line. Exposed for testing.
pub fn format_line(unix_ms: u128, level: LogLevel, target: &str, message: &str) -> String {
    format!("[{unix_ms}] {} {target}: {message}", level.as_str())
}

/// Emit a log line to stderr if `level` is enabled.
pub fn log(level: LogLevel, target: &str, message: &str) {
    if !enabled(level) {
        return;
    }
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    eprintln!("{}", format_line(unix_ms, level, target, message));
}

/// Log an info-level message, formatting lazily only when enabled.
#[macro_export]
macro_rules! log_info {
    ($target:expr, $($arg:tt)*) => {
        if $crate::logging::enabled($crate::logging::LogLevel::Info) {
            $crate::logging::log($crate::logging::LogLevel::Info, $target, &format!($($arg)*));
        }
    };
}

/// Log a warn-level message, formatting lazily only when enabled.
#[macro_export]
macro_rules! log_warn {
    ($target:expr, $($arg:tt)*) => {
        if $crate::logging::enabled($crate::logging::LogLevel::Warn) {
            $crate::logging::log($crate::logging::LogLevel::Warn, $target, &format!($($arg)*));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{format_line, LogLevel};

    #[test]
    fn parses_and_orders_levels() {
        assert_eq!(LogLevel::parse("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse("warning"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse("nonsense"), None);
        assert!(LogLevel::Error < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Trace);
    }

    #[test]
    fn formats_structured_line() {
        let line = format_line(1234, LogLevel::Warn, "remap", "source grabbed");
        assert_eq!(line, "[1234] WARN remap: source grabbed");
    }
}
