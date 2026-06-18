//! Process autodetection: match running game/app processes to profiles so the
//! right remapping can be applied automatically.
//!
//! Profiles declare `process:` patterns (see [`crate::profiles::ProcessMatch`]).
//! This module enumerates running processes from `/proc` and reports which
//! profile matches, mirroring reWASD's "autodetect game process and apply
//! profile" behavior.

use crate::profiles::Profile;
use std::path::Path;

/// A profile matched against a running process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMatch {
    pub profile_id: String,
    /// The running process name that triggered the match.
    pub process_name: String,
}

/// Enumerate process names of currently running processes from `/proc`.
///
/// Each process contributes its `comm` (short name) and the basename of its
/// `cmdline` argv[0], so both short and full executable names can be matched.
pub fn running_process_names() -> Vec<String> {
    running_process_names_in(Path::new("/proc"))
}

fn running_process_names_in(proc_dir: &Path) -> Vec<String> {
    let mut names = Vec::new();

    let Ok(entries) = std::fs::read_dir(proc_dir) else {
        return names;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let pid = file_name.to_string_lossy();
        if !pid.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let dir = entry.path();
        if let Ok(comm) = std::fs::read_to_string(dir.join("comm")) {
            let comm = comm.trim();
            if !comm.is_empty() {
                names.push(comm.to_string());
            }
        }
        if let Ok(cmdline) = std::fs::read(dir.join("cmdline")) {
            if let Some(argv0) = cmdline.split(|byte| *byte == 0).next() {
                if let Ok(argv0) = std::str::from_utf8(argv0) {
                    let argv0 = argv0.trim();
                    if !argv0.is_empty() {
                        names.push(argv0.to_string());
                    }
                }
            }
        }
    }

    names
}

/// Return the first profile whose `process:` patterns match any of the given
/// running process names. Profiles without process patterns are skipped.
pub fn match_profile<'a>(
    profiles: &'a [Profile],
    process_names: &[String],
) -> Option<(&'a Profile, ProfileMatch)> {
    for profile in profiles {
        if profile.process_match.is_empty() {
            continue;
        }
        for name in process_names {
            if profile.process_match.matches(name) {
                return Some((
                    profile,
                    ProfileMatch {
                        profile_id: profile.id.clone(),
                        process_name: name.clone(),
                    },
                ));
            }
        }
    }
    None
}

/// Detect the profile that matches the currently running processes.
pub fn detect_profile(profiles: &[Profile]) -> Option<(&Profile, ProfileMatch)> {
    let names = running_process_names();
    match_profile(profiles, &names)
}

/// What a watch loop should do after re-scanning running processes, given the
/// profile (if any) it is currently applying.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchDecision {
    /// Keep the current state (matched profile still running, or nothing to do).
    Keep,
    /// Start applying the named profile (a new or different match appeared).
    Switch(String),
    /// Stop the active remap (its matched process is gone).
    Stop,
}

/// Decide how a watch loop should react to the current process list.
///
/// `active_profile_id` is the profile currently being applied, if any.
pub fn decide_watch(
    profiles: &[Profile],
    process_names: &[String],
    active_profile_id: Option<&str>,
) -> WatchDecision {
    let best = match_profile(profiles, process_names).map(|(profile, _)| profile.id.clone());
    match (active_profile_id, best) {
        (Some(active), Some(best)) if active == best => WatchDecision::Keep,
        (Some(_), Some(best)) => WatchDecision::Switch(best),
        (Some(_), None) => WatchDecision::Stop,
        (None, Some(best)) => WatchDecision::Switch(best),
        (None, None) => WatchDecision::Keep,
    }
}

#[cfg(test)]
mod tests {
    use super::{match_profile, running_process_names_in, ProfileMatch};
    use crate::profiles::parse_profile_bytes;
    use std::path::Path;

    fn profile(id: &str, processes: &str) -> crate::profiles::Profile {
        let yaml = format!("id: {id}\nprocess: {processes}\nmappings: []\n");
        parse_profile_bytes(yaml.as_bytes(), Path::new("p.yaml")).unwrap()
    }

    #[test]
    fn matches_profile_by_process_basename() {
        let profiles = vec![
            profile("retro", "[retroarch]"),
            profile("mednafen", "[\"*mednafen*\"]"),
        ];

        let names = vec!["/usr/bin/retroarch".to_string()];
        let (matched, detail) = match_profile(&profiles, &names).unwrap();
        assert_eq!(matched.id, "retro");
        assert_eq!(
            detail,
            ProfileMatch {
                profile_id: "retro".to_string(),
                process_name: "/usr/bin/retroarch".to_string(),
            }
        );

        let glob_names = vec!["mednafen-wrapper".to_string()];
        let (matched, _) = match_profile(&profiles, &glob_names).unwrap();
        assert_eq!(matched.id, "mednafen");
    }

    #[test]
    fn watch_decisions_track_running_processes() {
        use super::{decide_watch, WatchDecision};

        let profiles = vec![profile("retro", "[retroarch]"), profile("game", "[mygame]")];

        // Nothing active, a match appears -> switch to it.
        assert_eq!(
            decide_watch(&profiles, &["retroarch".to_string()], None),
            WatchDecision::Switch("retro".to_string())
        );
        // Same profile still matches -> keep.
        assert_eq!(
            decide_watch(&profiles, &["retroarch".to_string()], Some("retro")),
            WatchDecision::Keep
        );
        // A different profile now matches -> switch.
        assert_eq!(
            decide_watch(&profiles, &["mygame".to_string()], Some("retro")),
            WatchDecision::Switch("game".to_string())
        );
        // Active profile's process is gone -> stop.
        assert_eq!(
            decide_watch(&profiles, &["someoneelse".to_string()], Some("retro")),
            WatchDecision::Stop
        );
        // Nothing active and nothing matches -> keep idling.
        assert_eq!(
            decide_watch(&profiles, &["someoneelse".to_string()], None),
            WatchDecision::Keep
        );
    }

    #[test]
    fn skips_profiles_without_process_patterns_and_unmatched() {
        let profiles = vec![profile("plain", "[]"), profile("game", "[mygame]")];
        let names = vec!["someotherapp".to_string()];
        assert!(match_profile(&profiles, &names).is_none());
    }

    #[test]
    fn reads_process_names_from_proc_layout() {
        let dir = std::env::temp_dir().join(format!("padproxy-proc-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let proc_pid = dir.join("4242");
        std::fs::create_dir_all(&proc_pid).unwrap();
        std::fs::write(proc_pid.join("comm"), "retroarch\n").unwrap();
        std::fs::write(
            proc_pid.join("cmdline"),
            b"/usr/bin/retroarch\0--fullscreen\0",
        )
        .unwrap();
        // A non-pid directory should be ignored.
        std::fs::create_dir_all(dir.join("bus")).unwrap();

        let names = running_process_names_in(&dir);
        assert!(names.contains(&"retroarch".to_string()));
        assert!(names.contains(&"/usr/bin/retroarch".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
