//! Optional command-specific completion through carapace
//! (<https://carapace-sh.github.io/carapace/>). When a `carapace` binary
//! resolves on PATH (and `JOSH_CARAPACE` is not `0`), argument completions
//! for external commands are asked of `carapace <name> export <name>
//! <args…prefix>` and its JSON `values` become suggestions. Every
//! failure mode — missing binary, nonzero exit, malformed JSON — falls back
//! silently to the native file completer.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

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

pub struct Carapace {
    binary: OnceLock<Option<PathBuf>>,
}

impl Carapace {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            binary: OnceLock::new(),
        }
    }

    /// Test seam: skip environment lookup and pin the resolution result.
    #[cfg(test)]
    #[must_use]
    pub fn with_binary(binary: Option<PathBuf>) -> Self {
        let cell = OnceLock::new();
        cell.set(binary).expect("fresh OnceLock");
        Self { binary: cell }
    }

    #[must_use]
    pub fn available(&self) -> bool {
        self.binary().is_some()
    }

    fn binary(&self) -> Option<&PathBuf> {
        self.binary
            .get_or_init(|| {
                resolve_binary(std::env::var_os("JOSH_CARAPACE"), std::env::var_os("PATH"))
            })
            .as_ref()
    }

    /// `words` are the command-line words up to the cursor (command name
    /// first, current partial word last). Returns `None` on any failure so
    /// callers fall back to native completion.
    #[must_use]
    pub fn complete(&self, words: &[String]) -> Option<Vec<Suggestion>> {
        let binary = self.binary()?;
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
    override_env: Option<std::ffi::OsString>,
    path: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    match override_env {
        Some(value) if value == "0" => return None,
        Some(value) if !value.is_empty() => {
            let candidate = PathBuf::from(value);
            return candidate.is_file().then_some(candidate);
        }
        _ => {}
    }
    let path = path?;
    std::env::split_paths(&path).find_map(|directory| {
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
    for ch in buffer.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                has_word = true;
            }
            None if ch.is_whitespace() => {
                if has_word {
                    words.push(std::mem::take(&mut current));
                }
                has_word = false;
            }
            None => {
                current.push(ch);
                has_word = true;
            }
        }
    }
    // The cursor-adjacent word (possibly empty after a trailing space) is
    // always the final argv slot: carapace completes that word.
    if has_word || buffer.chars().last().is_some_and(char::is_whitespace) || words.is_empty() {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn fake_carapace(directory: &std::path::Path, script: &str) -> PathBuf {
        let binary = directory.join("carapace");
        fs::write(&binary, script).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        binary
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
        let carapace = Carapace::with_binary(Some(binary));
        let suggestions = carapace
            .complete(&["fakecmd".to_string(), "che".to_string()])
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
        let carapace = Carapace::with_binary(Some(binary));
        let suggestions = carapace
            .complete(&["fakecmd".to_string(), "s".to_string()])
            .unwrap();
        assert!(!suggestions[0].append_whitespace);
    }

    #[test]
    fn failures_fall_back_silently() {
        let directory = tempdir().unwrap();
        let binary = fake_carapace(directory.path(), "#!/bin/sh\nexit 3\n");
        let carapace = Carapace::with_binary(Some(binary.clone()));
        assert!(
            carapace
                .complete(&["fakecmd".to_string(), "x".to_string()])
                .is_none()
        );

        fs::write(&binary, "#!/bin/sh\nprintf '{\"values\":[]}'\n").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let suggestions = Carapace::with_binary(Some(binary.clone()))
            .complete(&["fakecmd".to_string(), "x".to_string()])
            .unwrap();
        assert!(suggestions.is_empty());

        fs::write(&binary, "#!/bin/sh\nprintf 'not json'\n").unwrap();
        assert!(
            Carapace::with_binary(Some(binary))
                .complete(&["fakecmd".to_string(), "x".to_string()])
                .is_none()
        );
    }

    #[test]
    fn resolution_honors_override_zero_path_and_override_path() {
        let directory = tempdir().unwrap();
        let binary = fake_carapace(directory.path(), "#!/bin/sh\ntrue\n");
        let path = std::env::join_paths(std::iter::once(directory.path())).unwrap();

        assert_eq!(
            resolve_binary(Some("0".into()), Some(path.clone())),
            None,
            "JOSH_CARAPACE=0 disables even a present binary"
        );
        assert_eq!(
            resolve_binary(None, Some(path.clone())),
            Some(binary.clone())
        );
        assert_eq!(
            resolve_binary(Some("".into()), Some(path)),
            Some(binary.clone()),
            "empty override behaves like unset"
        );
        assert_eq!(
            resolve_binary(Some(binary.clone().into_os_string()), None),
            Some(binary)
        );
        assert_eq!(resolve_binary(None, None), None);
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
        Carapace::with_binary(Some(binary))
            .complete(&["fakecmd".to_string(), "sub".to_string(), "pr".to_string()])
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
        let carapace = Carapace::with_binary(Some(binary.clone()));
        assert!(
            carapace
                .complete(&["./tool".to_string(), "x".to_string()])
                .is_none()
        );
        assert!(
            carapace
                .complete(&["$x".to_string(), "x".to_string()])
                .is_none()
        );
        assert!(
            carapace
                .complete(&["name=value".to_string(), "x".to_string()])
                .is_none()
        );
    }

    #[test]
    fn buffer_words_keep_trailing_empty_slot_and_strip_quotes() {
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
        assert_eq!(command_words_up_to_cursor(""), vec![""]);
    }
}
