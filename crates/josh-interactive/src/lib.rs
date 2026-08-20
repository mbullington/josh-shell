use std::{
    borrow::Cow,
    collections::BTreeSet,
    env, fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    sync::{Arc, RwLock, atomic::Ordering},
};

mod carapace;
mod theme;

use carapace::Carapace;
use nu_ansi_term::Style;
use theme::Palette;

use reedline::{
    ColumnarMenu, Completer, DefaultHinter, Emacs, FileBackedHistory, Highlighter, Hinter, History,
    KeyCode, KeyModifiers, ListMenu, MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch,
    Reedline, ReedlineEvent, ReedlineMenu, Signal, Span as ReedlineSpan, StyledText, Suggestion,
    ValidationResult, Validator, default_emacs_keybindings,
};

use josh_runtime::{Engine, EngineError, PrettyOptions, RunResult, Value, render_value};
use josh_syntax::{Completeness, LexMode, Parse, Span, TokenKind, parse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionKind {
    Command,
    File,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub kind: CompletionKind,
    pub span: Span,
    pub prefix: String,
}

#[derive(Debug, Clone)]
pub struct CompletionSnapshot {
    pub commands: Arc<BTreeSet<String>>,
    pub variables: Arc<BTreeSet<String>>,
    pub cwd: Arc<PathBuf>,
    pub home: Option<PathBuf>,
}

impl CompletionSnapshot {
    /// Build from the *session* shell snapshot: command lookup follows
    /// mutations of `env.PATH`, variables come from the session environment,
    /// and relative PATH entries resolve against the session cwd.
    #[must_use]
    pub fn build(lexical: &[String], snapshot: &josh_runtime::ShellSnapshot) -> Self {
        let cwd = snapshot.cwd().to_path_buf();
        let home = snapshot
            .environment_variable(std::ffi::OsStr::new("HOME"))
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(PathBuf::from));
        let fallback = env::var_os("PATH").unwrap_or_default();
        let path = snapshot
            .environment_variable(std::ffi::OsStr::new("PATH"))
            .unwrap_or(&fallback);
        let root = cwd.clone();
        let directories = env::split_paths(path).map(move |entry| {
            if entry.is_absolute() {
                entry
            } else {
                root.join(entry)
            }
        });
        Self::build_from_parts(
            lexical,
            directories,
            snapshot.environment().keys(),
            cwd,
            home,
        )
    }

    fn build_from_parts(
        lexical: &[String],
        directories: impl IntoIterator<Item = PathBuf>,
        environment: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
        cwd: PathBuf,
        home: Option<PathBuf>,
    ) -> Self {
        let mut commands = BTreeSet::from([
            "cd".into(),
            "command".into(),
            "exit".into(),
            "status".into(),
        ]);
        for directory in directories {
            if let Ok(entries) = fs::read_dir(directory) {
                for entry in entries.flatten() {
                    if josh_exec::is_executable(&entry.path())
                        && let Some(name) = entry.file_name().to_str()
                    {
                        commands.insert(name.to_owned());
                    }
                }
            }
        }
        let mut variables = lexical.iter().cloned().collect::<BTreeSet<_>>();
        variables.extend(
            environment
                .into_iter()
                .filter_map(|name| name.as_ref().to_str().map(str::to_owned)),
        );
        Self {
            commands: Arc::new(commands),
            variables: Arc::new(variables),
            cwd: Arc::new(cwd),
            home,
        }
    }
}

#[derive(Default)]
pub struct ReplAnalyzer;

impl ReplAnalyzer {
    #[must_use]
    pub fn parse(&self, buffer: &str) -> Parse {
        parse(Arc::<str>::from(buffer))
    }

    #[must_use]
    pub fn completion_context(&self, buffer: &str, cursor: usize) -> CompletionContext {
        let cursor = floor_char_boundary(buffer, cursor.min(buffer.len()));
        let start = buffer[..cursor]
            .char_indices()
            .rev()
            .find_map(|(at, ch)| {
                (ch.is_whitespace() || matches!(ch, '|' | ';' | '{' | '}'))
                    .then_some(at + ch.len_utf8())
            })
            .unwrap_or(0);
        let raw = &buffer[start..cursor];
        let (kind, replace_start, prefix) = if let Some(prefix) = raw.strip_prefix('$') {
            (CompletionKind::Variable, start + 1, prefix.to_owned())
        } else {
            let parsed = self.parse(&buffer[..cursor]);
            let previous = parsed.tokens.iter().rev().find(|token| {
                token.span.end <= start
                    && !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment)
            });
            let kind = if previous.is_none_or(|token| {
                matches!(
                    token.kind,
                    TokenKind::Pipe
                        | TokenKind::Semicolon
                        | TokenKind::Newline
                        | TokenKind::LeftBrace
                        | TokenKind::If
                )
            }) {
                CompletionKind::Command
            } else {
                CompletionKind::File
            };
            (kind, start, raw.to_owned())
        };
        CompletionContext {
            kind,
            span: Span::new(replace_start, cursor),
            prefix,
        }
    }
}

pub struct JoshValidator;
impl Validator for JoshValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if parse(Arc::<str>::from(line)).completeness == Completeness::Incomplete {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

pub struct JoshHighlighter {
    snapshot: Arc<RwLock<Arc<CompletionSnapshot>>>,
    palette: Palette,
}
impl Highlighter for JoshHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let parsed = parse(Arc::<str>::from(line));
        let snapshot = self
            .snapshot
            .read()
            .expect("completion snapshot lock poisoned")
            .clone();
        // Every statement and pipeline stage has its own command position;
        // statement boundaries mirror the completer's `completion_context`.
        let mut command_position = true;
        let mut styled = StyledText::new();
        for token in &parsed.tokens {
            let style = if command_position
                && token.mode == LexMode::Command
                && matches!(token.kind, TokenKind::Identifier | TokenKind::Word)
            {
                let name = &line[token.span.range()];
                if snapshot.commands.contains(name) {
                    self.palette.command_valid
                } else {
                    self.palette.command_invalid
                }
            } else {
                self.palette.token_style(&token.kind, token.mode)
            };
            styled.push((style, line[token.span.range()].to_owned()));
            match token.kind {
                TokenKind::Whitespace | TokenKind::Comment => {}
                TokenKind::Newline
                | TokenKind::Semicolon
                | TokenKind::Pipe
                | TokenKind::AndAnd
                | TokenKind::OrOr
                | TokenKind::LeftBrace
                | TokenKind::CaptureStart
                | TokenKind::If => command_position = true,
                _ => command_position = false,
            }
        }
        styled
    }
}

pub struct JoshCompleter {
    analyzer: ReplAnalyzer,
    snapshot: Arc<RwLock<Arc<CompletionSnapshot>>>,
    carapace: Carapace,
}
impl Completer for JoshCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let context = self.analyzer.completion_context(line, pos);
        let snapshot = self
            .snapshot
            .read()
            .expect("completion snapshot lock poisoned");

        // Command-specific completion: when an external command precedes the
        // word and carapace is installed, its suggestions win; any failure
        // degrades silently to the native file list.
        if matches!(context.kind, CompletionKind::File) && self.carapace.available() {
            let words = carapace::command_words_up_to_cursor(&line[..context.span.end]);
            if words.len() >= 2
                && let Some(mut suggestions) = self.carapace.complete(&words)
                && !suggestions.is_empty()
            {
                suggestions.truncate(200);
                for suggestion in &mut suggestions {
                    suggestion.span = ReedlineSpan::new(context.span.start, context.span.end);
                }
                return suggestions;
            }
        }
        native_completions(&context, &snapshot)
            .into_iter()
            .take(200)
            .map(|value| {
                let is_directory = value.ends_with('/');
                Suggestion {
                    value,
                    span: ReedlineSpan::new(context.span.start, context.span.end),
                    append_whitespace: !matches!(context.kind, CompletionKind::Variable)
                        && !is_directory,
                    ..Suggestion::default()
                }
            })
            .collect()
    }
}

/// Prefix-filtered native candidates for a completion context: the fallback
/// when carapace has no answer, and the source for ghost-text hints (which
/// must not spawn a process per keystroke).
fn native_completions(context: &CompletionContext, snapshot: &CompletionSnapshot) -> Vec<String> {
    match context.kind {
        CompletionKind::Command => snapshot
            .commands
            .iter()
            .filter(|x| x.starts_with(&context.prefix))
            .cloned()
            .collect(),
        CompletionKind::Variable => snapshot
            .variables
            .iter()
            .filter(|x| x.starts_with(&context.prefix))
            .cloned()
            .collect(),
        CompletionKind::File => {
            file_completions(&context.prefix, &snapshot.cwd, snapshot.home.as_deref())
        }
    }
}

/// Ghost-text typeahead behind the cursor, accepted with Right (whole
/// hint) or Alt+Right (first word). The most recent history entry with the
/// typed prefix wins, matching fish's autosuggestion priority; when no
/// history entry matches, the remainder of the first native Tab candidate
/// is offered instead.
pub struct JoshHinter {
    analyzer: ReplAnalyzer,
    snapshot: Arc<RwLock<Arc<CompletionSnapshot>>>,
    history: DefaultHinter,
    completion_hint: String,
    style: Style,
}

impl JoshHinter {
    #[must_use]
    pub fn new(snapshot: Arc<RwLock<Arc<CompletionSnapshot>>>, style: Style) -> Self {
        Self {
            analyzer: ReplAnalyzer,
            snapshot,
            history: DefaultHinter::default(),
            completion_hint: String::new(),
            style,
        }
    }

    /// Remainder of the first native completion candidate past the cursor,
    /// or empty when the cursor is mid-line or nothing completes. Command
    /// words like `l` complete constantly, so a non-empty prefix is
    /// required here even though history hints do not.
    fn completion_hint(&self, line: &str, pos: usize) -> String {
        if line.is_empty() || pos != line.len() {
            return String::new();
        }
        let context = self.analyzer.completion_context(line, pos);
        // No hints inside comments; a word prefix is required because
        // command words would otherwise complete after every space.
        if context.prefix.is_empty() || line[..context.span.start].contains(" #") {
            return String::new();
        }
        let snapshot = self
            .snapshot
            .read()
            .expect("completion snapshot lock poisoned")
            .clone();
        native_completions(&context, &snapshot)
            .first()
            .and_then(|value| value.strip_prefix(&context.prefix))
            .map_or_else(String::new, str::to_owned)
    }
}

impl Hinter for JoshHinter {
    fn handle(
        &mut self,
        line: &str,
        pos: usize,
        history: &dyn History,
        use_ansi_coloring: bool,
        cwd: &str,
    ) -> String {
        let styled = self
            .history
            .handle(line, pos, history, use_ansi_coloring, cwd);
        if !self.history.complete_hint().is_empty() {
            self.completion_hint.clear();
            return styled;
        }
        self.completion_hint = self.completion_hint(line, pos);
        if use_ansi_coloring && !self.completion_hint.is_empty() {
            self.style.paint(&self.completion_hint).to_string()
        } else {
            self.completion_hint.clone()
        }
    }

    fn complete_hint(&self) -> String {
        let history = self.history.complete_hint();
        if history.is_empty() {
            self.completion_hint.clone()
        } else {
            history
        }
    }

    fn next_hint_token(&self) -> String {
        first_hint_token(&self.complete_hint())
    }
}

/// Leading whitespace plus the first word — reedline's incremental
/// hint-accept granularity.
fn first_hint_token(hint: &str) -> String {
    let mut token = String::new();
    let mut content = false;
    for ch in hint.chars() {
        if content && ch.is_whitespace() {
            break;
        }
        content |= !ch.is_whitespace();
        token.push(ch);
    }
    token
}

pub struct JoshPrompt {
    indicator: String,
}

impl JoshPrompt {
    #[must_use]
    pub fn new(indicator: impl Into<String>) -> Self {
        Self {
            indicator: indicator.into(),
        }
    }
}

impl Default for JoshPrompt {
    fn default() -> Self {
        Self::new("josh> ")
    }
}

impl Prompt for JoshPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(&self.indicator)
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("...> ")
    }
    fn render_prompt_history_search_indicator(&self, search: PromptHistorySearch) -> Cow<'_, str> {
        Cow::Owned(format!("(history: {}) ", search.term))
    }
}

pub fn run_repl(engine: &mut Engine) -> Result<i32, Box<dyn std::error::Error>> {
    let interrupted = engine.execution_cancellation();
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&interrupted))?;
    let (palette, theme_warning) = Palette::from_environment();
    if let Some(warning) = theme_warning {
        eprintln!("{warning}");
    }
    let snapshot = Arc::new(RwLock::new(Arc::new(CompletionSnapshot::build(
        &engine.variable_names(),
        &engine.shell_snapshot(),
    ))));
    let history_path = engine
        .environment_variable_os("JOSH_HISTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            engine.environment_variable_os("HOME").map_or_else(
                || engine.shell_snapshot().cwd().join(".josh_history"),
                |home| PathBuf::from(home).join(".josh_history"),
            )
        });
    // Plain-text history stays grep-able and cheap even at this cap (~1 MB
    // worst case at ~100 bytes/command); `JOSH_HISTORY_SIZE` overrides it.
    let history_capacity = engine
        .environment_variable_os("JOSH_HISTORY_SIZE")
        .and_then(|value| value.to_string_lossy().parse::<usize>().ok())
        .filter(|capacity| *capacity > 0)
        .unwrap_or(10_000);
    let history = FileBackedHistory::with_file(history_capacity, history_path)?;
    let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));
    // Ctrl+R opens the fish-style history menu: substring search over the
    // deduplicated history, newest first, navigable with the arrow keys and
    // refiltered on every keystroke. Enter copies the match into the buffer.
    let history_menu = Box::new(ListMenu::default().with_name("history_menu"));
    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".into()),
            ReedlineEvent::MenuNext,
        ]),
    );
    keybindings.add_binding(
        KeyModifiers::CONTROL,
        KeyCode::Char('r'),
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("history_menu".into()),
            ReedlineEvent::MenuNext,
        ]),
    );
    let mut editor = Reedline::create()
        .with_history(Box::new(history))
        .with_validator(Box::new(JoshValidator))
        .with_highlighter(Box::new(JoshHighlighter {
            snapshot: Arc::clone(&snapshot),
            palette: palette.clone(),
        }))
        .with_completer(Box::new(JoshCompleter {
            analyzer: ReplAnalyzer,
            snapshot: Arc::clone(&snapshot),
            carapace: Carapace::new(),
        }))
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_menu(ReedlineMenu::HistoryMenu(history_menu))
        .with_edit_mode(Box::new(Emacs::new(keybindings)))
        .with_hinter(Box::new(JoshHinter::new(
            Arc::clone(&snapshot),
            palette.hint,
        )));
    loop {
        let prompt = match engine.prompt() {
            Ok(Some(prompt)) => JoshPrompt::new(prompt),
            Ok(None) => JoshPrompt::default(),
            Err(error) => {
                eprintln!("error: invalid prompt(): {error}; using `josh> `");
                JoshPrompt::default()
            }
        };
        match editor.read_line(&prompt)? {
            Signal::Success(line) => {
                interrupted.store(false, Ordering::Release);
                // Persist the accepted line now so a crash or kill does not
                // lose the session's history (FileBackedHistory only syncs
                // to disk on Drop).
                let _ = editor.sync_history();
                match engine.run_source(Arc::<str>::from(line)) {
                    Ok(RunResult::Exit(code)) => {
                        return Ok(code);
                    }
                    Ok(RunResult::Value(value)) => {
                        if value != Value::Null {
                            println!("{}", render_value(&value, &pretty_options()));
                        }
                    }
                    Err(error) => print_engine_error(&error),
                }
                interrupted.store(false, Ordering::Release);
                *snapshot.write().expect("completion snapshot lock poisoned") = Arc::new(
                    CompletionSnapshot::build(&engine.variable_names(), &engine.shell_snapshot()),
                );
            }
            Signal::CtrlC => {
                interrupted.store(false, Ordering::Release);
                continue;
            }
            Signal::CtrlD => {
                return Ok(0);
            }
            Signal::HostCommand(_) | Signal::ExternalBreak(_) => continue,
            _ => continue,
        }
    }
}

fn pretty_options() -> PrettyOptions {
    let mut options = PrettyOptions::default();
    if let Ok((width, _)) = crossterm::terminal::size() {
        options.width = usize::from(width);
    }
    options.colors = std::io::stdout().is_terminal();
    options
}

pub fn print_engine_error(error: &EngineError) {
    match error {
        EngineError::Parse(failure) => {
            let origin = failure
                .origin
                .as_ref()
                .map_or_else(|| "<input>".to_owned(), |path| path.display().to_string());
            for diagnostic in &failure.diagnostics {
                eprintln!("{}", diagnostic.render(&failure.source, &origin));
            }
        }
        _ => eprintln!("error: {error}"),
    }
}

fn file_completions(prefix: &str, cwd: &Path, home: Option<&Path>) -> Vec<String> {
    // A leading `~` mirrors execution: bare `~` and `~/…` resolve against
    // the session HOME while the inserted text keeps the tilde form.
    let tilde = prefix == "~" || prefix.starts_with("~/");
    if tilde && home.is_none() {
        return vec![];
    }
    // Resolve relative prefixes against the session cwd; an empty prefix (or
    // one whose parent is empty, like `comp_a`) scans the cwd itself.
    let candidate = if tilde {
        let rest = prefix
            .strip_prefix('~')
            .unwrap_or(prefix)
            .trim_start_matches('/');
        home.expect("tilde completion requires HOME").join(rest)
    } else if prefix.is_empty() {
        cwd.join(".")
    } else if Path::new(prefix).is_absolute() {
        PathBuf::from(prefix)
    } else {
        cwd.join(prefix)
    };
    let path = candidate.as_path();
    let (directory, base) = if path.is_dir() {
        (path, "")
    } else {
        (
            path.parent().unwrap_or_else(|| Path::new(".")),
            path.file_name().and_then(|x| x.to_str()).unwrap_or(""),
        )
    };
    let display_parent = if tilde {
        // `~` has no path parent, but its entries are listed from HOME; keep
        // the tilde so inserted text expands the same way at execution time.
        Some(
            Path::new(prefix)
                .parent()
                .filter(|x| *x != Path::new(""))
                .map_or_else(|| PathBuf::from("~"), Path::to_path_buf),
        )
    } else {
        Path::new(prefix)
            .parent()
            .filter(|x| *x != Path::new(""))
            .map(Path::to_path_buf)
    };
    let Ok(entries) = fs::read_dir(directory) else {
        return vec![];
    };
    let mut results = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with(base) {
                return None;
            }
            let mut result = display_parent
                .as_ref()
                .map_or_else(|| PathBuf::from(&name), |parent| parent.join(&name))
                .to_string_lossy()
                .into_owned();
            if entry.path().is_dir() {
                result.push('/');
            }
            Some(result)
        })
        .collect::<Vec<_>>();
    results.sort();
    results
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt};

    use tempfile::tempdir;

    use super::CompletionSnapshot;

    #[test]
    fn completion_snapshot_uses_only_executable_path_entries() {
        let directory = tempdir().unwrap();
        let executable = directory.path().join("runs");
        let inert = directory.path().join("does-not-run");
        fs::write(&executable, "").unwrap();
        fs::write(&inert, "").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&inert, fs::Permissions::from_mode(0o600)).unwrap();

        let snapshot = CompletionSnapshot::build_from_parts(
            &[],
            [directory.path().to_path_buf()],
            std::iter::empty::<String>(),
            std::env::temp_dir(),
            None,
        );
        assert!(snapshot.commands.contains("runs"));
        assert!(!snapshot.commands.contains("does-not-run"));
    }

    #[test]
    fn completion_snapshot_follows_session_path_and_environment() {
        let directory = tempdir().unwrap();
        let bin = directory.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let command = bin.join("brandnewcmd99");
        fs::write(&command, "").unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o700)).unwrap();

        let context =
            josh_runtime::ShellContext::new(directory.path().to_path_buf(), std::iter::empty());
        context
            .set_environment_variable("PATH", Some(bin.as_os_str().to_owned()))
            .unwrap();
        context
            .set_environment_variable("BRANDNEW_VARIABLE", Some("1".into()))
            .unwrap();
        let snapshot = CompletionSnapshot::build(&[], &context.snapshot());
        assert!(snapshot.commands.contains("brandnewcmd99"));
        assert!(snapshot.variables.contains("BRANDNEW_VARIABLE"));
    }

    #[test]
    fn file_completions_resolve_against_session_cwd() {
        let directory = tempdir().unwrap();
        let file = directory.path().join("comp_alpha.txt");
        fs::write(&file, "").unwrap();

        assert!(
            super::file_completions("comp_a", directory.path(), None)
                .into_iter()
                .any(|value| value == "comp_alpha.txt")
        );
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("unique-nested-file.txt"), "").unwrap();
        assert!(
            super::file_completions("nested/uniq", directory.path(), None)
                .into_iter()
                .any(|value| value == "nested/unique-nested-file.txt")
        );
    }

    #[test]
    fn file_completions_expand_leading_tilde_against_home() {
        let directory = tempdir().unwrap();
        let home = directory.path().join("home");
        let config = home.join(".config");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("settings.toml"), "").unwrap();
        fs::write(home.join("notes.txt"), "").unwrap();
        let elsewhere = directory.path().join("cwd");
        fs::create_dir(&elsewhere).unwrap();

        // `~/` lists HOME entries with the tilde preserved for insertion.
        let values = super::file_completions("~/.con", &elsewhere, Some(&home));
        assert_eq!(values, vec!["~/.config/".to_string()]);
        // Bare `~` completes to HOME's entries.
        assert!(
            super::file_completions("~/notes", &elsewhere, Some(&home))
                .into_iter()
                .any(|value| value == "~/notes.txt")
        );
        // Deeper prefixes keep displaying the tilde form.
        assert_eq!(
            super::file_completions("~/.config/sett", &elsewhere, Some(&home)),
            vec!["~/.config/settings.toml".to_string()]
        );
        // Without HOME there is nothing sensible to offer.
        assert!(super::file_completions("~/.con", &elsewhere, None).is_empty());
    }
}

fn floor_char_boundary(text: &str, mut at: usize) -> usize {
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

#[cfg(test)]
mod highlight_tests {
    use std::collections::BTreeSet;
    use std::sync::{Arc, RwLock};

    use nu_ansi_term::{Color, Style};
    use reedline::Highlighter;

    use super::{CompletionSnapshot, JoshHighlighter, Palette};

    fn highlight(line: &str) -> Vec<(Style, String)> {
        let snapshot = Arc::new(RwLock::new(Arc::new(CompletionSnapshot {
            commands: Arc::new(BTreeSet::from(["echo".to_string()])),
            variables: Arc::new(BTreeSet::new()),
            cwd: Arc::new(std::env::temp_dir()),
            home: None,
        })));
        let highlighter = JoshHighlighter {
            snapshot,
            palette: Palette::from_environment().0,
        };
        highlighter.highlight(line, 0).buffer
    }

    fn style_of<'a>(buffer: &'a [(Style, String)], text: &str) -> Option<&'a Style> {
        buffer
            .iter()
            .find(|(_, chunk)| chunk == text)
            .map(|(style, _)| style)
    }

    #[test]
    fn every_command_head_gets_validity_styling() {
        let valid = Style::new().fg(Color::Green).bold();
        let invalid = Style::new().fg(Color::Red).bold();
        let buffer = highlight("echo hi | jqrq .x && mounty && echo done");
        assert_eq!(style_of(&buffer, "echo"), Some(&valid));
        assert_eq!(style_of(&buffer, "jqrq"), Some(&invalid));
        assert_eq!(style_of(&buffer, "mounty"), Some(&invalid));
        let echo_styles: Vec<_> = buffer
            .iter()
            .filter(|(_, chunk)| chunk == "echo")
            .map(|(style, _)| style)
            .collect();
        assert!(echo_styles.iter().all(|style| **style == valid));
        // Arguments are not command positions.
        assert_eq!(style_of(&buffer, "hi"), Some(&Style::new()));
        assert_eq!(style_of(&buffer, "x"), Some(&Style::new()));
    }

    #[test]
    fn expression_identifiers_are_not_command_positions() {
        let buffer = highlight("let zyzyx = 5");
        assert_eq!(
            style_of(&buffer, "zyzyx"),
            Some(&Style::new().fg(Color::LightBlue))
        );
    }
}
