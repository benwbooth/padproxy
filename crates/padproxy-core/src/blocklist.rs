//! Blocklist of apps/games where remapping must not be applied.
//!
//! reWASD lets users mark applications where remap should stay off. PadProxy
//! stores a blocklist as process name/glob patterns (one per line) and the
//! `watch` auto-apply loop refuses to apply, and stops, while a blocked process
//! is running.

use crate::profiles::ProcessMatch;
use std::path::PathBuf;

/// Path to the blocklist file: `$PADPROXY_BLOCKLIST_FILE`, else
/// `$XDG_CONFIG_HOME/padproxy/blocklist.txt`, else
/// `$HOME/.config/padproxy/blocklist.txt`.
pub fn blocklist_path() -> PathBuf {
    if let Some(path) = std::env::var_os("PADPROXY_BLOCKLIST_FILE") {
        return PathBuf::from(path);
    }

    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("padproxy/blocklist.txt");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/padproxy/blocklist.txt");
    }

    PathBuf::from("blocklist.txt")
}

/// Load the blocklist patterns. A missing file is an empty blocklist.
pub fn load_blocklist() -> ProcessMatch {
    let path = blocklist_path();
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    parse_blocklist(&contents)
}

/// Parse blocklist text: one pattern per line, `#` comments and blank lines
/// ignored.
pub fn parse_blocklist(contents: &str) -> ProcessMatch {
    let patterns = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.to_string())
        .collect();
    ProcessMatch { patterns }
}

/// Returns true if any running process name matches the blocklist.
pub fn is_blocked(blocklist: &ProcessMatch, process_names: &[String]) -> bool {
    !blocklist.is_empty() && process_names.iter().any(|name| blocklist.matches(name))
}

#[cfg(test)]
mod tests {
    use super::{is_blocked, parse_blocklist};

    #[test]
    fn parses_patterns_ignoring_comments_and_blanks() {
        let blocklist = parse_blocklist(
            "\
# games where remap should stay off
obs
  *steam*

# end
",
        );
        assert_eq!(blocklist.patterns, vec!["obs", "*steam*"]);
    }

    #[test]
    fn detects_blocked_running_processes() {
        let blocklist = parse_blocklist("obs\n*steam*\n");
        assert!(is_blocked(&blocklist, &["/usr/bin/obs".to_string()]));
        assert!(is_blocked(&blocklist, &["steamwebhelper".to_string()]));
        assert!(!is_blocked(&blocklist, &["retroarch".to_string()]));
    }

    #[test]
    fn empty_blocklist_blocks_nothing() {
        let blocklist = parse_blocklist("\n# only comments\n");
        assert!(blocklist.is_empty());
        assert!(!is_blocked(&blocklist, &["anything".to_string()]));
    }
}
