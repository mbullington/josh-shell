//! Optional command-specific completion through carapace
//! (<https://carapace-sh.github.io/carapace/>). When a `carapace` binary
//! resolves on PATH (and `JOSH_CARAPACE` is not `0`), argument completions
//! for external commands are asked of `carapace <name> export <name>
//! <args…prefix>` and its JSON `values` become suggestions. Every
//! failure mode — missing binary, nonzero exit, malformed JSON — falls back
//! silently to the native file completer.

use std::path::{Path, PathBuf};
use std::process::Command;

use josh_runtime::ShellSnapshot;
use reedline::Suggestion;

#[derive(Clone, Debug, serde::Deserialize)]
struct ExportValue {
    value: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct Export {
    #[serde(default)]
    nospace: String,
    #[serde(default)]
    values: Vec<ExportValue>,
}

pub struct Carapace;

impl Carapace {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn binary(&self, snapshot: &ShellSnapshot) -> Option<PathBuf> {
        resolve_binary(
            snapshot.environment_variable(std::ffi::OsStr::new("JOSH_CARAPACE")),
            snapshot.environment_variable(std::ffi::OsStr::new("PATH")),
            snapshot.cwd(),
        )
    }

    /// `words` are the command-line words up to the cursor (command name
    /// first, current partial word last). Returns `None` on any failure so
    /// callers fall back to native completion.
    #[must_use]
    pub fn complete(&self, words: &[String], snapshot: &ShellSnapshot) -> Option<Vec<Suggestion>> {
        let binary = self.binary(snapshot)?;
        let name = words.first()?;
        // Only simple command names have carapace completers; anything else
        // (paths, assignments, flags sneaking into word 0) goes native.
        if name.is_empty()
            || name
                .chars()
                .any(|ch| ch.is_whitespace() || matches!(ch, '/' | '$' | '=' | ';' | '|' | '`'))
        {
            return None;
        }
        let output = Command::new(binary)
            .arg(name)
            .arg("export")
            .args(words)
            .current_dir(snapshot.cwd())
            .env_clear()
            .envs(snapshot.environment())
            .stdin(std::process::Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let export: Export = serde_json::from_slice(&output.stdout).ok()?;
        let nospace_all = export.nospace.contains('*');
        let suggestions = export
            .values
            .into_iter()
            .map(|entry| Suggestion {
                append_whitespace: !nospace_all
                    && entry
                        .value
                        .chars()
                        .last()
                        .is_some_and(|last| !export.nospace.contains(last)),
                description: entry.description,
                value: entry.value,
                ..Suggestion::default()
            })
            .collect();
        Some(suggestions)
    }
}

fn resolve_binary(
    override_env: Option<&std::ffi::OsStr>,
    path: Option<&std::ffi::OsStr>,
    cwd: &Path,
) -> Option<PathBuf> {
    match override_env {
        Some(value) if value == "0" => return None,
        Some(value) if !value.is_empty() => {
            let candidate = PathBuf::from(value);
            let candidate = if candidate.is_absolute() {
                candidate
            } else {
                cwd.join(candidate)
            };
            return josh_exec::is_executable(&candidate).then_some(candidate);
        }
        _ => {}
    }
    let path = path?;
    std::env::split_paths(path).find_map(|directory| {
        let directory = if directory.is_absolute() {
            directory
        } else {
            cwd.join(directory)
        };
        let candidate = directory.join("carapace");
        josh_exec::is_executable(&candidate).then_some(candidate)
    })
}

/// Best-effort split of the buffer up to the cursor into command-line words.
/// Quotes are stripped; this is a completion convenience, not a parser —
/// malformed input degrades through the native completer instead.
#[must_use]
pub fn command_words_up_to_cursor(buffer: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut has_word = false;
    let mut trailing_space = false;
    let mut chars = buffer.chars().peekable();
    while let Some(ch) = chars.next() {
        match quote {
            Some(q) if ch == q => {
                quote = None;
                trailing_space = false;
            }
            Some(_) if ch == '\\' => {
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
                has_word = true;
                trailing_space = false;
            }
            Some(_) => {
                current.push(ch);
                has_word = true;
                trailing_space = false;
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                has_word = true;
                trailing_space = false;
            }
            None if ch == '\\' => {
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
                has_word = true;
                trailing_space = false;
            }
            None if ch == '\n' || ch == '\r' => {
                words.clear();
                current.clear();
                has_word = false;
                trailing_space = false;
            }
            None if ch.is_whitespace() => {
                if has_word {
                    words.push(std::mem::take(&mut current));
                }
                has_word = false;
                trailing_space = true;
            }
            None if matches!(ch, ';' | '|' | '&' | '{' | '}' | ')' | '<' | '>') => {
                words.clear();
                current.clear();
                has_word = false;
                trailing_space = false;
                if matches!(ch, '|' | '&') && chars.peek() == Some(&ch) {
                    chars.next();
                }
            }
            None if ch == '$' && chars.peek() == Some(&'(') => {
                chars.next();
                words.clear();
                current.clear();
                has_word = false;
                trailing_space = false;
            }
            None => {
                current.push(ch);
                has_word = true;
                trailing_space = false;
            }
        }
    }
    if has_word || trailing_space || words.is_empty() {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use josh_runtime::ShellContext;
    use tempfile::tempdir;

    fn fake_carapace(directory: &std::path::Path, script: &str) -> PathBuf {
        let binary = directory.join("carapace");
        fs::write(&binary, script).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        binary
    }

    fn session(directory: &Path, binary: &Path) -> ShellSnapshot {
        ShellContext::new(
            directory,
            [(
                OsString::from("JOSH_CARAPACE"),
                binary.as_os_str().to_owned(),
            )],
        )
        .snapshot()
    }

    #[test]
    fn export_json_maps_to_suggestions() {
        let directory = tempdir().unwrap();
        let binary = fake_carapace(
            directory.path(),
            r#"#!/bin/sh
printf '%s' '{"nospace":"","values":[{"value":"checkout","display":"checkout","description":"switch branches"},{"value":"cherry-pick","display":"cherry-pick"}]}'
"#,
        );
        let snapshot = session(directory.path(), &binary);
        let suggestions = Carapace::new()
            .complete(&["fakecmd".to_string(), "che".to_string()], &snapshot)
            .expect("fake binary answers");
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].value, "checkout");
        assert_eq!(
            suggestions[0].description.as_deref(),
            Some("switch branches")
        );
        assert!(suggestions[0].append_whitespace);
        assert_eq!(suggestions[1].value, "cherry-pick");
    }

    #[test]
    fn nospace_suffixes_suppress_trailing_space() {
        let directory = tempdir().unwrap();
        let binary = fake_carapace(
            directory.path(),
            r#"#!/bin/sh
printf '%s' '{"nospace":"/","values":[{"value":"src/","display":"src/"}]}'
"#,
        );
        let snapshot = session(directory.path(), &binary);
        let suggestions = Carapace::new()
            .complete(&["fakecmd".to_string(), "s".to_string()], &snapshot)
            .unwrap();
        assert!(!suggestions[0].append_whitespace);
    }

    #[test]
    fn failures_fall_back_silently() {
        let complete_with = |script: &str| {
            let directory = tempdir().unwrap();
            let binary = fake_carapace(directory.path(), script);
            let snapshot = session(directory.path(), &binary);
            Carapace::new().complete(&["fakecmd".to_string(), "x".to_string()], &snapshot)
        };

        assert!(complete_with("#!/bin/sh\nexit 3\n").is_none());
        assert!(
            complete_with("#!/bin/sh\nprintf '{\"values\":[]}'\n")
                .unwrap()
                .is_empty()
        );
        assert!(complete_with("#!/bin/sh\nprintf 'not json'\n").is_none());
    }

    #[test]
    fn resolution_honors_override_zero_path_and_override_path() {
        let directory = tempdir().unwrap();
        let binary = fake_carapace(directory.path(), "#!/bin/sh\ntrue\n");
        let path = std::env::join_paths(std::iter::once(directory.path())).unwrap();

        assert_eq!(
            resolve_binary(Some(OsStr::new("0")), Some(&path), directory.path()),
            None,
            "JOSH_CARAPACE=0 disables even a present binary"
        );
        assert_eq!(
            resolve_binary(None, Some(&path), directory.path()),
            Some(binary.clone())
        );
        assert_eq!(
            resolve_binary(Some(OsStr::new("")), Some(&path), directory.path()),
            Some(binary.clone()),
            "empty override behaves like unset"
        );
        assert_eq!(
            resolve_binary(Some(binary.as_os_str()), None, directory.path()),
            Some(binary)
        );
        assert_eq!(resolve_binary(None, None, directory.path()), None);
    }

    #[test]
    fn argv_is_completer_name_then_full_words() {
        let directory = tempdir().unwrap();
        let record = directory.path().join("argv");
        let binary = fake_carapace(
            directory.path(),
            &format!(
                "#!/bin/sh\nprintf '%s' \"$*\" > {}\nprintf '{{\"values\":[]}}'\n",
                record.display()
            ),
        );
        let snapshot = session(directory.path(), &binary);
        Carapace::new()
            .complete(
                &["fakecmd".to_string(), "sub".to_string(), "pr".to_string()],
                &snapshot,
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(&record).unwrap(),
            "fakecmd export fakecmd sub pr"
        );
    }

    #[test]
    fn first_word_restrictions_keep_paths_native() {
        let directory = tempdir().unwrap();
        let binary = fake_carapace(directory.path(), "#!/bin/sh\nprintf '{\"values\":[]}'\n");
        let snapshot = session(directory.path(), &binary);
        let carapace = Carapace::new();
        assert!(
            carapace
                .complete(&["./tool".to_string(), "x".to_string()], &snapshot)
                .is_none()
        );
        assert!(
            carapace
                .complete(&["$x".to_string(), "x".to_string()], &snapshot)
                .is_none()
        );
        assert!(
            carapace
                .complete(&["name=value".to_string(), "x".to_string()], &snapshot)
                .is_none()
        );
    }

    #[test]
    fn completion_process_uses_session_cwd_and_environment() {
        let directory = tempdir().unwrap();
        let record = directory.path().join("session");
        let binary = fake_carapace(
            directory.path(),
            &format!(
                "#!/bin/sh\nprintf '%s\\n%s' \"$PWD\" \"$SESSION_MARKER\" > {}\nprintf '{{\"values\":[]}}'\n",
                record.display()
            ),
        );
        let snapshot = ShellContext::new(
            directory.path(),
            [
                (OsString::from("JOSH_CARAPACE"), binary.into_os_string()),
                (
                    OsString::from("SESSION_MARKER"),
                    OsString::from("from-josh"),
                ),
            ],
        )
        .snapshot();
        Carapace::new()
            .complete(&["fakecmd".to_string(), "x".to_string()], &snapshot)
            .unwrap();
        assert_eq!(
            fs::read_to_string(record).unwrap(),
            format!("{}\nfrom-josh", directory.path().display())
        );
    }

    #[test]
    fn buffer_words_keep_only_the_current_command() {
        assert_eq!(
            command_words_up_to_cursor("git checkout pr"),
            vec!["git", "checkout", "pr"]
        );
        assert_eq!(
            command_words_up_to_cursor("git checkout "),
            vec!["git", "checkout", ""]
        );
        assert_eq!(
            command_words_up_to_cursor("git \"my bra"),
            vec!["git", "my bra"]
        );
        assert_eq!(
            command_words_up_to_cursor("printf x | git ch"),
            vec!["git", "ch"]
        );
        assert_eq!(
            command_words_up_to_cursor("false && git ch"),
            vec!["git", "ch"]
        );
        assert_eq!(
            command_words_up_to_cursor("printf $(git ch"),
            vec!["git", "ch"]
        );
        assert_eq!(
            command_words_up_to_cursor("printf x; git foo\\ bar"),
            vec!["git", "foo bar"]
        );
        assert_eq!(command_words_up_to_cursor("git status > out"), vec!["out"]);
        assert_eq!(command_words_up_to_cursor(""), vec![""]);
    }
}
