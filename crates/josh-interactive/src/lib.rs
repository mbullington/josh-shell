use std::{
    borrow::Cow,
    collections::BTreeSet,
    env, fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    sync::{Arc, RwLock, atomic::Ordering},
};

mod carapace;

use carapace::Carapace;
use nu_ansi_term::{Color, Style};
use reedline::{
    ColumnarMenu, Completer, DefaultHinter, Emacs, FileBackedHistory, Highlighter, KeyCode,
    KeyModifiers, MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch, Reedline,
    ReedlineEvent, ReedlineMenu, Signal, Span as ReedlineSpan, StyledText, Suggestion,
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
}

impl CompletionSnapshot {
    /// Build from the *session* shell snapshot: command lookup follows
    /// mutations of `env.PATH`, variables come from the session environment,
    /// and relative PATH entries resolve against the session cwd.
    #[must_use]
    pub fn build(lexical: &[String], snapshot: &josh_runtime::ShellSnapshot) -> Self {
        let cwd = snapshot.cwd().to_path_buf();
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
        Self::build_from_parts(lexical, directories, snapshot.environment().keys(), cwd)
    }

    fn build_from_parts(
        lexical: &[String],
        directories: impl IntoIterator<Item = PathBuf>,
        environment: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
        cwd: PathBuf,
    ) -> Self {
        let mut commands = BTreeSet::from(["cd".into(), "exit".into(), "status".into()]);
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
}
impl Highlighter for JoshHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let parsed = parse(Arc::<str>::from(line));
        let snapshot = self
            .snapshot
            .read()
            .expect("completion snapshot lock poisoned")
            .clone();
        let first_command = parsed
            .tokens
            .iter()
            .position(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Word));
        let mut styled = StyledText::new();
        for (index, token) in parsed.tokens.iter().enumerate() {
            let style = if Some(index) == first_command && token.mode == LexMode::Command {
                let name = &line[token.span.range()];
                if snapshot.commands.contains(name) {
                    Style::new().fg(Color::Green).bold()
                } else {
                    Style::new().fg(Color::Red).bold()
                }
            } else {
                token_style(&token.kind, token.mode)
            };
            styled.push((style, line[token.span.range()].to_owned()));
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
        let native_values = || -> Vec<String> {
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
                CompletionKind::File => file_completions(&context.prefix, &snapshot.cwd),
            }
        };
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
        native_values()
            .into_iter()
            .take(200)
            .map(|value| Suggestion {
                value,
                span: ReedlineSpan::new(context.span.start, context.span.end),
                append_whitespace: !matches!(context.kind, CompletionKind::Variable),
                ..Suggestion::default()
            })
            .collect()
    }
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
    let history = FileBackedHistory::with_file(1_000, history_path)?;
    let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));
    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".into()),
            ReedlineEvent::MenuNext,
        ]),
    );
    let mut editor = Reedline::create()
        .with_history(Box::new(history))
        .with_validator(Box::new(JoshValidator))
        .with_highlighter(Box::new(JoshHighlighter {
            snapshot: Arc::clone(&snapshot),
        }))
        .with_completer(Box::new(JoshCompleter {
            analyzer: ReplAnalyzer,
            snapshot: Arc::clone(&snapshot),
            carapace: Carapace::new(),
        }))
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_edit_mode(Box::new(Emacs::new(keybindings)))
        .with_hinter(Box::new(DefaultHinter::default()));
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
        EngineError::Parse(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("error: {diagnostic}");
            }
        }
        _ => eprintln!("error: {error}"),
    }
}

fn token_style(kind: &TokenKind, mode: LexMode) -> Style {
    match kind {
        TokenKind::Whitespace | TokenKind::Newline => Style::new(),
        TokenKind::Comment => Style::new().fg(Color::DarkGray),
        TokenKind::SingleQuoted | TokenKind::DoubleQuoted | TokenKind::String => {
            Style::new().fg(Color::Yellow)
        }
        TokenKind::Number | TokenKind::True | TokenKind::False | TokenKind::Null => {
            Style::new().fg(Color::Cyan)
        }
        TokenKind::DollarVariable | TokenKind::CaptureStart => Style::new().fg(Color::Purple),
        TokenKind::Let
        | TokenKind::Fn
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::While
        | TokenKind::Loop
        | TokenKind::Try
        | TokenKind::Catch
        | TokenKind::Throw
        | TokenKind::Return
        | TokenKind::Break
        | TokenKind::Continue
        | TokenKind::Status
        | TokenKind::Typeof => Style::new().fg(Color::Blue).bold(),
        TokenKind::Unsupported(_) | TokenKind::Unknown => Style::new().fg(Color::Red).underline(),
        _ if mode == LexMode::Expression => Style::new().fg(Color::LightBlue),
        _ => Style::new(),
    }
}

fn file_completions(prefix: &str, cwd: &Path) -> Vec<String> {
    // Resolve relative prefixes against the session cwd; an empty prefix (or
    // one whose parent is empty, like `comp_a`) scans the cwd itself.
    let candidate = if prefix.is_empty() {
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
    let display_parent = Path::new(prefix)
        .parent()
        .filter(|x| *x != Path::new(""))
        .map(Path::to_path_buf);
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
            super::file_completions("comp_a", directory.path())
                .into_iter()
                .any(|value| value == "comp_alpha.txt")
        );
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("unique-nested-file.txt"), "").unwrap();
        assert!(
            super::file_completions("nested/uniq", directory.path())
                .into_iter()
                .any(|value| value == "nested/unique-nested-file.txt")
        );
    }
}

fn floor_char_boundary(text: &str, mut at: usize) -> usize {
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}
