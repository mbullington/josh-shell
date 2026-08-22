use std::{
    borrow::Cow,
    collections::BTreeSet,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock, atomic::Ordering},
};

mod carapace;
mod theme;

use carapace::Carapace;
use nu_ansi_term::{Color, Style};
use theme::Palette;

use reedline::{
    ColumnarMenu, Completer, DefaultHinter, Emacs, FileBackedHistory, Highlighter, Hinter, History,
    InputMode, KeyCode, KeyModifiers, ListMenu, MenuBuilder, Prompt, PromptEditMode,
    PromptHistorySearch, Reedline, ReedlineEvent, ReedlineMenu, Signal, Span as ReedlineSpan,
    StyledText, Suggestion, ValidationResult, Validator, default_emacs_keybindings,
};

use josh_runtime::{
    Engine, EngineError, PrettyOptions, RunResult, ShellSnapshot, Value, render_value,
};
use josh_syntax::{Completeness, LexMode, Parse, Span, TokenKind, parse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionKind {
    Command,
    File,
    Variable,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub kind: CompletionKind,
    pub span: Span,
    pub prefix: String,
    quote: Option<CompletionQuote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionQuote {
    Processed(char),
    Raw(char),
}

impl CompletionQuote {
    const fn delimiter(self) -> char {
        match self {
            Self::Processed(delimiter) | Self::Raw(delimiter) => delimiter,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionSnapshot {
    pub commands: Arc<BTreeSet<String>>,
    pub variables: Arc<BTreeSet<String>>,
    pub cwd: Arc<PathBuf>,
    pub home: Option<PathBuf>,
    session: Arc<ShellSnapshot>,
}

impl CompletionSnapshot {
    /// Build from the session shell snapshot. Command lookup follows
    /// mutations of `env.PATH`, variables come from the session environment,
    /// and relative PATH entries resolve against the session cwd.
    #[must_use]
    pub fn build(engine: &Engine) -> Self {
        let snapshot = engine.shell_snapshot();
        let cwd = snapshot.cwd().to_path_buf();
        let home = snapshot
            .environment_variable(std::ffi::OsStr::new("HOME"))
            .map(PathBuf::from)
            .map(|home| resolve_session_path(&cwd, home));
        let root = cwd.clone();
        let directories = snapshot
            .environment_variable(std::ffi::OsStr::new("PATH"))
            .into_iter()
            .flat_map(std::env::split_paths)
            .map(move |entry| {
                if entry.is_absolute() {
                    entry
                } else {
                    root.join(entry)
                }
            })
            .collect::<Vec<_>>();
        Self::build_from_parts(
            &engine.variable_names(),
            &engine.function_names(),
            directories,
            cwd,
            home,
            snapshot,
        )
    }

    fn build_from_parts(
        lexical: &[String],
        functions: &[String],
        directories: impl IntoIterator<Item = PathBuf>,
        cwd: PathBuf,
        home: Option<PathBuf>,
        session: ShellSnapshot,
    ) -> Self {
        let mut commands = BTreeSet::from([
            "cd".into(),
            "command".into(),
            "exit".into(),
            "source".into(),
            "status".into(),
        ]);
        commands.extend(functions.iter().cloned());
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
            session
                .environment()
                .keys()
                .filter_map(|name| name.to_str().map(str::to_owned)),
        );
        Self {
            commands: Arc::new(commands),
            variables: Arc::new(variables),
            cwd: Arc::new(cwd),
            home,
            session: Arc::new(session),
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
        let parsed = self.parse(&buffer[..cursor]);
        if let Some(token) = parsed.tokens.last()
            && token.span.end == cursor
        {
            if matches!(token.kind, TokenKind::Comment) {
                return CompletionContext {
                    kind: CompletionKind::None,
                    span: Span::new(cursor, cursor),
                    prefix: String::new(),
                    quote: None,
                };
            }
            let token_text = &buffer[token.span.range()];
            if matches!(token.kind, TokenKind::DollarVariable) || token_text == "$" {
                let end = self
                    .parse(buffer)
                    .tokens
                    .into_iter()
                    .find(|candidate| {
                        candidate.span.start == token.span.start
                            && matches!(candidate.kind, TokenKind::DollarVariable)
                    })
                    .map_or(cursor, |candidate| candidate.span.end);
                return CompletionContext {
                    kind: CompletionKind::Variable,
                    span: Span::new(token.span.start + 1, end),
                    prefix: buffer[token.span.start + 1..cursor].to_owned(),
                    quote: None,
                };
            }
        }

        let (start, prefix, quote) = parsed
            .tokens
            .last()
            .filter(|token| token.span.end == cursor)
            .and_then(|token| open_quote(&buffer[token.span.range()], &token.kind))
            .map_or_else(
                || {
                    let start = completion_word_start(buffer, cursor);
                    (start, unescape_bare_prefix(&buffer[start..cursor]), None)
                },
                |(opening_bytes, quote)| {
                    let start =
                        parsed.tokens.last().expect("open quote token").span.start + opening_bytes;
                    let prefix = if matches!(quote, CompletionQuote::Raw(_)) {
                        buffer[start..cursor].to_owned()
                    } else {
                        unescape_bare_prefix(&buffer[start..cursor])
                    };
                    (start, prefix, Some(quote))
                },
            );
        let previous = parsed.tokens.iter().enumerate().rev().find(|(_, token)| {
            token.span.end <= start
                && !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment)
        });
        let command_builtin = previous.is_some_and(|(index, token)| {
            matches!(token.kind, TokenKind::Identifier | TokenKind::Word)
                && &buffer[token.span.range()] == "command"
                && parsed.tokens[..index]
                    .iter()
                    .rev()
                    .find(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment))
                    .is_none_or(|token| is_command_boundary(&token.kind))
        });
        let at_command =
            command_builtin || previous.is_none_or(|(_, token)| is_command_boundary(&token.kind));
        let kind = if at_command && !prefix.contains('/') && !prefix.starts_with('~') {
            CompletionKind::Command
        } else {
            CompletionKind::File
        };
        CompletionContext {
            kind,
            span: Span::new(start, completion_word_end(buffer, cursor, quote)),
            prefix,
            quote,
        }
    }
}

fn command_word_end(buffer: &str, start: usize) -> usize {
    let mut quote = None;
    let mut escaped = false;
    for (offset, ch) in buffer[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch.is_whitespace()
            || matches!(
                ch,
                '#' | '|' | '&' | ';' | '{' | '}' | '(' | ')' | '<' | '>'
            )
        {
            return start + offset;
        }
    }
    buffer.len()
}

fn literal_command_name(word: &str) -> Option<String> {
    if word.is_empty() {
        return None;
    }
    if word.len() >= 3
        && ((word.starts_with("r'") && word.ends_with('\''))
            || (word.starts_with("r\"") && word.ends_with('"')))
    {
        return Some(word[2..word.len() - 1].to_owned());
    }
    if word.contains('$') {
        return None;
    }

    let mut result = String::with_capacity(word.len());
    let mut quote = None;
    let mut escaped = false;
    for ch in word.chars() {
        if escaped {
            result.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if quote.is_some_and(|active| ch == active) {
            quote = None;
        } else if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else {
            result.push(ch);
        }
    }
    (!result.is_empty()).then_some(result)
}

fn command_resolves(name: &str, snapshot: &CompletionSnapshot) -> bool {
    if !name.contains('/') {
        return snapshot.commands.contains(name);
    }
    let path = if name == "~" || name.starts_with("~/") {
        let Some(home) = snapshot.home.as_deref() else {
            return false;
        };
        home.join(name.trim_start_matches('~').trim_start_matches('/'))
    } else {
        PathBuf::from(name)
    };
    let path = if path.is_absolute() {
        path
    } else {
        snapshot.cwd.join(path)
    };
    josh_exec::is_executable(&path)
}

fn completion_word_start(buffer: &str, cursor: usize) -> usize {
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (at, ch) in buffer[..cursor].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch.is_whitespace() || matches!(ch, '|' | '&' | ';' | '{' | '}' | '(') {
            start = at + ch.len_utf8();
        }
    }
    start
}

fn completion_word_end(buffer: &str, cursor: usize, quote: Option<CompletionQuote>) -> usize {
    let mut escaped = false;
    for (offset, ch) in buffer[cursor..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && !matches!(quote, Some(CompletionQuote::Raw(_))) {
            escaped = true;
            continue;
        }
        if quote.is_some_and(|active| ch == active.delimiter()) {
            return cursor + offset + ch.len_utf8();
        }
        if quote.is_none()
            && (ch.is_whitespace() || matches!(ch, '|' | '&' | ';' | '{' | '}' | '(' | ')'))
        {
            return cursor + offset;
        }
    }
    buffer.len()
}

fn open_quote(text: &str, kind: &TokenKind) -> Option<(usize, CompletionQuote)> {
    let (opening_bytes, quote) = match kind {
        TokenKind::SingleQuoted => (1, CompletionQuote::Processed('\'')),
        TokenKind::DoubleQuoted => (1, CompletionQuote::Processed('"')),
        TokenKind::RawString if text.starts_with("r'") => (2, CompletionQuote::Raw('\'')),
        TokenKind::RawString if text.starts_with("r\"") => (2, CompletionQuote::Raw('"')),
        _ => return None,
    };
    let delimiter = quote.delimiter();
    let closes = text[opening_bytes..].ends_with(delimiter)
        && (matches!(quote, CompletionQuote::Raw(_))
            || text[..text.len() - delimiter.len_utf8()]
                .chars()
                .rev()
                .take_while(|ch| *ch == '\\')
                .count()
                % 2
                == 0);
    (!closes).then_some((opening_bytes, quote))
}

fn unescape_bare_prefix(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                result.push(escaped);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn is_command_boundary(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Pipe
            | TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::Semicolon
            | TokenKind::Newline
            | TokenKind::LeftBrace
            | TokenKind::CaptureStart
            | TokenKind::If
            | TokenKind::While
            | TokenKind::Status
    )
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
        let mut command_style = None;
        let mut styled = StyledText::new();
        for token in &parsed.tokens {
            if command_style.is_some_and(|(end, _)| token.span.start >= end) {
                command_style = None;
            }
            if command_position
                && token.mode == LexMode::Command
                && matches!(
                    token.kind,
                    TokenKind::Identifier
                        | TokenKind::Word
                        | TokenKind::SingleQuoted
                        | TokenKind::DoubleQuoted
                        | TokenKind::RawString
                        | TokenKind::Number
                        | TokenKind::Dot
                        | TokenKind::Slash
                )
            {
                let end = command_word_end(line, token.span.start);
                command_style = literal_command_name(&line[token.span.start..end]).map(|name| {
                    let style = if command_resolves(&name, &snapshot) {
                        self.palette.command_valid
                    } else {
                        self.palette.command_invalid
                    };
                    (end, style)
                });
            }
            let style = command_style
                .filter(|(end, _)| token.span.start < *end)
                .map_or_else(
                    || self.palette.token_style(&token.kind, token.mode),
                    |(_, style)| style,
                );
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
                | TokenKind::If
                | TokenKind::While
                | TokenKind::Status => command_position = true,
                _ => command_position = false,
            }
        }
        styled
    }
}

#[derive(Debug)]
struct CompletionPreview {
    line: String,
    cursor: usize,
    span_start: usize,
    kind: CompletionKind,
    values: Vec<String>,
}

fn history_hint_matches_preview(line: &str, hint: &str, preview: &CompletionPreview) -> bool {
    let completed = format!("{line}{hint}");
    preview.values.iter().any(|value| {
        let candidate = format!("{}{}", &line[..preview.span_start], value);
        completed.starts_with(&candidate)
            && (value.ends_with('/')
                || completed[candidate.len()..]
                    .chars()
                    .next()
                    .is_none_or(char::is_whitespace))
    })
}

pub struct JoshCompleter {
    analyzer: ReplAnalyzer,
    snapshot: Arc<RwLock<Arc<CompletionSnapshot>>>,
    preview: Arc<Mutex<Option<CompletionPreview>>>,
    carapace: Carapace,
}
impl Completer for JoshCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let preview = self
            .preview
            .lock()
            .expect("completion preview lock poisoned");
        if let Some(preview) = preview
            .as_ref()
            .filter(|preview| preview.line == line && preview.cursor == pos)
        {
            return preview
                .values
                .iter()
                .map(|value| Suggestion {
                    value: value.clone(),
                    span: ReedlineSpan::new(preview.span_start, preview.cursor),
                    append_whitespace: !matches!(preview.kind, CompletionKind::Variable)
                        && !value.ends_with('/'),
                    ..Suggestion::default()
                })
                .collect();
        }
        drop(preview);

        let context = self.analyzer.completion_context(line, pos);
        let snapshot = self
            .snapshot
            .read()
            .expect("completion snapshot lock poisoned");

        // Command-specific completion: when an external command precedes the
        // word and carapace is installed, its suggestions win; any failure
        // degrades silently to the native file list.
        if matches!(context.kind, CompletionKind::File) {
            let words = carapace::command_words_up_to_cursor(&line[..pos]);
            if words.len() >= 2
                && let Some(mut suggestions) = self.carapace.complete(&words, &snapshot.session)
                && !suggestions.is_empty()
            {
                suggestions.truncate(200);
                for suggestion in &mut suggestions {
                    suggestion.value =
                        format_candidate(&suggestion.value, &context, suggestion.append_whitespace);
                    suggestion.span = ReedlineSpan::new(context.span.start, context.span.end);
                    suggestion.append_whitespace = completion_needs_space(
                        line,
                        context.span.end,
                        suggestion.append_whitespace,
                    );
                }
                return suggestions;
            }
        }
        let suggestions: Vec<_> = native_completions(&context, &snapshot)
            .into_iter()
            .take(200)
            .map(|value| {
                let wants_space =
                    !matches!(context.kind, CompletionKind::Variable) && !value.ends_with('/');
                Suggestion {
                    value,
                    span: ReedlineSpan::new(context.span.start, context.span.end),
                    append_whitespace: completion_needs_space(line, context.span.end, wants_space),
                    ..Suggestion::default()
                }
            })
            .collect();
        if suggestions.is_empty() {
            vec![Suggestion {
                span: ReedlineSpan::new(pos, pos),
                ..Suggestion::default()
            }]
        } else {
            suggestions
        }
    }
}

/// Prefix-filtered native candidates for a completion context: the fallback
/// when carapace has no answer, and the source for ghost-text hints (which
/// must not spawn a process per keystroke).
fn native_completions(context: &CompletionContext, snapshot: &CompletionSnapshot) -> Vec<String> {
    let candidates: Vec<String> = match context.kind {
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
        CompletionKind::None => return Vec::new(),
    };
    candidates
        .into_iter()
        .map(|candidate| {
            let close_quote = !candidate.ends_with('/');
            format_candidate(&candidate, context, close_quote)
        })
        .collect()
}

fn format_candidate(candidate: &str, context: &CompletionContext, close_quote: bool) -> String {
    if let Some(quote) = context.quote {
        let delimiter = quote.delimiter();
        let mut formatted = String::with_capacity(candidate.len() + usize::from(close_quote));
        for ch in candidate.chars() {
            if matches!(quote, CompletionQuote::Processed(_))
                && (matches!(ch, '\\' | '$') || ch == delimiter)
            {
                formatted.push('\\');
            }
            formatted.push(ch);
        }
        if close_quote {
            formatted.push(delimiter);
        }
        formatted
    } else if matches!(context.kind, CompletionKind::Command | CompletionKind::File) {
        escape_bare_candidate(candidate)
    } else {
        candidate.to_owned()
    }
}

fn escape_bare_candidate(candidate: &str) -> String {
    let mut formatted = String::with_capacity(candidate.len());
    for ch in candidate.chars() {
        if ch.is_whitespace()
            || matches!(
                ch,
                '\\' | '\''
                    | '"'
                    | '$'
                    | '#'
                    | '|'
                    | '&'
                    | ';'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '*'
                    | '?'
                    | '<'
                    | '>'
                    | '!'
            )
        {
            formatted.push('\\');
        }
        formatted.push(ch);
    }
    formatted
}

fn completion_needs_space(line: &str, replaced_end: usize, wants_space: bool) -> bool {
    wants_space
        && !line[replaced_end..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
}

/// Ghost-text typeahead behind the cursor, accepted with Right (whole
/// hint) or Alt+Right (first word). The most recent history entry with the
/// typed prefix wins, matching fish's autosuggestion priority; when no
/// history entry matches, the remainder of the first native Tab candidate
/// is offered instead.
pub struct JoshHinter {
    analyzer: ReplAnalyzer,
    snapshot: Arc<RwLock<Arc<CompletionSnapshot>>>,
    preview: Arc<Mutex<Option<CompletionPreview>>>,
    history: DefaultHinter,
    current_hint: String,
    style: Style,
}

impl JoshHinter {
    #[must_use]
    pub fn new(snapshot: Arc<RwLock<Arc<CompletionSnapshot>>>, style: Style) -> Self {
        Self::with_preview(snapshot, Arc::new(Mutex::new(None)), style)
    }

    fn with_preview(
        snapshot: Arc<RwLock<Arc<CompletionSnapshot>>>,
        preview: Arc<Mutex<Option<CompletionPreview>>>,
        style: Style,
    ) -> Self {
        Self {
            analyzer: ReplAnalyzer,
            snapshot,
            preview,
            history: DefaultHinter::default(),
            current_hint: String::new(),
            style,
        }
    }

    /// Remainder of the first native completion candidate past the cursor,
    /// plus the alternatives needed to keep that first candidate in the
    /// edit buffer while Reedline's completion menu is open.
    fn completion_preview(&self, line: &str, pos: usize) -> Option<(String, CompletionPreview)> {
        if line.is_empty() || pos != line.len() {
            return None;
        }
        let context = self.analyzer.completion_context(line, pos);
        // No hints inside comments; a word prefix is required because
        // command words would otherwise complete after every space.
        if context.prefix.is_empty() || line[..context.span.start].contains(" #") {
            return None;
        }
        let snapshot = self
            .snapshot
            .read()
            .expect("completion snapshot lock poisoned")
            .clone();
        let values = native_completions(&context, &snapshot);
        let first = values.first()?;
        let replacement = &line[context.span.range()];
        let hint = first.strip_prefix(replacement)?.to_owned();
        let mut completed_line = line.to_owned();
        completed_line.replace_range(context.span.range(), first);
        let cursor = completed_line.len();
        Some((
            hint,
            CompletionPreview {
                line: completed_line,
                cursor,
                span_start: context.span.start,
                kind: context.kind,
                values,
            },
        ))
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
        let _ = self.history.handle(line, pos, history, false, cwd);
        let history_hint = self.history.complete_hint();
        let native = self.completion_preview(line, pos);
        let history_is_stale_path = native.as_ref().is_some_and(|(_, preview)| {
            matches!(preview.kind, CompletionKind::File)
                && !history_hint.is_empty()
                && !history_hint_matches_preview(line, &history_hint, preview)
        });

        let (hint, preview) = if !history_hint.is_empty() && !history_is_stale_path {
            (history_hint, None)
        } else if let Some((hint, preview)) = native {
            (hint, Some(preview))
        } else {
            (String::new(), None)
        };
        let styled = if use_ansi_coloring && !hint.is_empty() {
            self.style.paint(&hint).to_string()
        } else {
            hint.clone()
        };
        self.current_hint = hint;
        *self
            .preview
            .lock()
            .expect("completion preview lock poisoned") = preview;
        styled
    }

    fn complete_hint(&self) -> String {
        self.current_hint.clone()
    }

    fn next_hint_token(&self) -> String {
        first_hint_token(&self.current_hint)
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
    let session = engine.shell_snapshot();
    let session_cwd = session.cwd();
    let theme = engine
        .environment_variable_os("JOSH_THEME")
        .map(PathBuf::from)
        .map(|path| resolve_session_path(session_cwd, path));
    let (palette, theme_warning) = Palette::from_path(theme.as_deref().map(Path::as_os_str));
    if let Some(warning) = theme_warning {
        eprintln!("{warning}");
    }
    let snapshot = Arc::new(RwLock::new(Arc::new(CompletionSnapshot::build(engine))));
    let completion_preview = Arc::new(Mutex::new(None));
    let history_path = engine
        .environment_variable_os("JOSH_HISTORY")
        .map(PathBuf::from)
        .map(|path| resolve_session_path(session_cwd, path))
        .unwrap_or_else(|| {
            engine
                .environment_variable_os("HOME")
                .map(PathBuf::from)
                .map(|path| resolve_session_path(session_cwd, path))
                .unwrap_or_else(|| session_cwd.to_path_buf())
                .join(".josh_history")
        });
    // Plain-text history stays grep-able and cheap even at this cap (~1 MB
    // worst case at ~100 bytes/command); `JOSH_HISTORY_SIZE` overrides it.
    let history_capacity = engine
        .environment_variable_os("JOSH_HISTORY_SIZE")
        .and_then(|value| value.to_string_lossy().parse::<usize>().ok())
        .filter(|capacity| *capacity > 0)
        .unwrap_or(10_000);
    let history = FileBackedHistory::with_file(history_capacity, history_path)?;
    let menu_text = Color::DarkGray.normal();
    let menu_selection = menu_text.bold().underline();
    let completion_menu = Box::new(
        ColumnarMenu::default()
            .with_name("completion_menu")
            .with_input_mode(InputMode::FullBuffer)
            .with_text_style(menu_text)
            .with_match_text_style(menu_text)
            .with_selected_text_style(menu_selection)
            .with_selected_match_text_style(menu_selection)
            .with_description_text_style(menu_text.dimmed()),
    );
    // Ctrl+R opens the fish-style history menu: substring search over the
    // deduplicated history, newest first, navigable with the arrow keys and
    // refiltered on every keystroke. Enter copies the match into the buffer.
    let history_menu = Box::new(
        ListMenu::default()
            .with_name("history_menu")
            .with_text_style(menu_text)
            .with_match_text_style(menu_text)
            .with_selected_text_style(menu_selection)
            .with_selected_match_text_style(menu_selection)
            .with_description_text_style(menu_text.dimmed()),
    );
    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Multiple(vec![
                ReedlineEvent::HistoryHintComplete,
                ReedlineEvent::Menu("completion_menu".into()),
            ]),
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
        .with_quick_completions(true)
        .with_partial_completions(true)
        .with_history(Box::new(history))
        .with_validator(Box::new(JoshValidator))
        .with_highlighter(Box::new(JoshHighlighter {
            snapshot: Arc::clone(&snapshot),
            palette: palette.clone(),
        }))
        .with_completer(Box::new(JoshCompleter {
            analyzer: ReplAnalyzer,
            snapshot: Arc::clone(&snapshot),
            preview: Arc::clone(&completion_preview),
            carapace: Carapace::new(),
        }))
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_menu(ReedlineMenu::HistoryMenu(history_menu))
        .with_edit_mode(Box::new(Emacs::new(keybindings)))
        .with_hinter(Box::new(JoshHinter::with_preview(
            Arc::clone(&snapshot),
            completion_preview,
            palette.hint,
        )));
    let mut history_warning_printed = false;
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
                if let Err(error) = editor.sync_history()
                    && !history_warning_printed
                {
                    eprintln!("warning: could not sync REPL history: {error}");
                    history_warning_printed = true;
                }
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
                *snapshot.write().expect("completion snapshot lock poisoned") =
                    Arc::new(CompletionSnapshot::build(engine));
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

fn resolve_session_path(cwd: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
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
    let browsing_directory = path.is_dir();
    let (directory, base) = if browsing_directory {
        (path, "")
    } else {
        (
            path.parent().unwrap_or_else(|| Path::new(".")),
            path.file_name().and_then(|x| x.to_str()).unwrap_or(""),
        )
    };
    let display_parent = if browsing_directory && !prefix.is_empty() {
        Some(PathBuf::from(prefix))
    } else if tilde {
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
            if !name.starts_with(base) || (name.starts_with('.') && !base.starts_with('.')) {
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
    use std::{
        collections::BTreeSet,
        ffi::OsString,
        fs,
        os::unix::fs::PermissionsExt,
        path::Path,
        sync::{Arc, Mutex, RwLock},
    };

    use josh_exec::ProcessHost;
    use josh_runtime::{Engine, ShellContext};
    use reedline::{Completer, Span as ReedlineSpan};
    use tempfile::tempdir;

    use super::{
        Carapace, CompletionKind, CompletionPreview, CompletionSnapshot, JoshCompleter,
        ReplAnalyzer,
    };

    fn completer(cwd: &Path, commands: &[&str]) -> JoshCompleter {
        let context = ShellContext::new(cwd, []);
        let snapshot = CompletionSnapshot {
            commands: Arc::new(
                commands
                    .iter()
                    .map(|command| (*command).to_owned())
                    .collect(),
            ),
            variables: Arc::new(BTreeSet::new()),
            cwd: Arc::new(cwd.to_path_buf()),
            home: None,
            session: Arc::new(context.snapshot()),
        };
        JoshCompleter {
            analyzer: ReplAnalyzer,
            snapshot: Arc::new(RwLock::new(Arc::new(snapshot))),
            preview: Arc::new(Mutex::new(None)),
            carapace: Carapace::new(),
        }
    }

    #[test]
    fn completion_snapshot_uses_only_executable_path_entries() {
        let directory = tempdir().unwrap();
        let executable = directory.path().join("runs");
        let inert = directory.path().join("does-not-run");
        fs::write(&executable, "").unwrap();
        fs::write(&inert, "").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&inert, fs::Permissions::from_mode(0o600)).unwrap();

        let context = ShellContext::new(directory.path(), []);
        let snapshot = CompletionSnapshot::build_from_parts(
            &[],
            &[],
            [directory.path().to_path_buf()],
            directory.path().to_path_buf(),
            None,
            context.snapshot(),
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

        let context = ShellContext::new(
            directory.path().to_path_buf(),
            std::iter::empty::<(OsString, OsString)>(),
        );
        context
            .set_environment_variable("PATH", Some(bin.as_os_str().to_owned()))
            .unwrap();
        context
            .set_environment_variable("BRANDNEW_VARIABLE", Some("1".into()))
            .unwrap();
        context
            .set_environment_variable("HOME", Some("relative-home".into()))
            .unwrap();
        let mut engine = Engine::with_shell_context(ProcessHost::default(), context);
        engine
            .run_source("fn session_function() { return null }; ordinary = 1")
            .unwrap();
        let snapshot = CompletionSnapshot::build(&engine);
        assert!(snapshot.commands.contains("brandnewcmd99"));
        assert!(snapshot.commands.contains("source"));
        assert!(snapshot.commands.contains("session_function"));
        assert!(!snapshot.commands.contains("ordinary"));
        assert!(snapshot.variables.contains("BRANDNEW_VARIABLE"));
        assert_eq!(snapshot.home, Some(directory.path().join("relative-home")));
    }

    #[test]
    fn completion_snapshot_does_not_restore_unset_path_or_home() {
        let directory = tempdir().unwrap();
        let context =
            ShellContext::new(directory.path(), std::iter::empty::<(OsString, OsString)>());
        let engine = Engine::with_shell_context(ProcessHost::default(), context);
        let snapshot = CompletionSnapshot::build(&engine);
        assert!(snapshot.home.is_none());
        assert!(!snapshot.commands.contains("cargo"));
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
        assert_eq!(
            super::file_completions("nested", directory.path(), None),
            vec!["nested/unique-nested-file.txt".to_string()]
        );
    }

    #[test]
    fn completion_candidates_preserve_shell_word_boundaries() {
        let analyzer = super::ReplAnalyzer;
        let command = analyzer.completion_context("alpha", 5);
        assert_eq!(
            super::format_candidate("alpha tool", &command, true),
            "alpha\\ tool"
        );

        let bare = analyzer.completion_context("cat foo\\ b", 10);
        assert_eq!(
            super::format_candidate("foo bar.txt", &bare, true),
            "foo\\ bar.txt"
        );

        let quoted = analyzer.completion_context("cat \"foo b", 10);
        assert_eq!(
            super::format_candidate("foo bar.txt", &quoted, true),
            "foo bar.txt\""
        );
        assert_eq!(
            super::format_candidate("foo bar/", &quoted, false),
            "foo bar/"
        );

        let raw = analyzer.completion_context("cat r\"$ca", 9);
        assert_eq!(
            super::format_candidate("$cash.txt", &raw, true),
            "$cash.txt\""
        );
    }

    #[test]
    fn completion_replaces_an_existing_closing_quote() {
        let analyzer = super::ReplAnalyzer;
        let context = analyzer.completion_context("cat \"foX\" tail", 7);
        assert_eq!(context.span, josh_syntax::Span::new(5, 9));
        let candidate = super::format_candidate("foo bar.txt", &context, true);
        let mut completed = "cat \"foX\" tail".to_string();
        completed.replace_range(context.span.range(), &candidate);
        assert_eq!(completed, "cat \"foo bar.txt\" tail");
    }

    #[test]
    fn completion_does_not_duplicate_existing_whitespace() {
        assert!(super::completion_needs_space("printf", 6, true));
        assert!(!super::completion_needs_space("printf arg", 6, true));
        assert!(!super::completion_needs_space("printf", 6, false));

        let directory = tempdir().unwrap();
        let mut completer = completer(directory.path(), &["printf"]);
        let suggestion = completer.complete("printf arg", 6).remove(0);
        assert_eq!(suggestion.value, "printf");
        assert!(!suggestion.append_whitespace);
    }

    #[test]
    fn inline_preview_keeps_all_menu_alternatives() {
        let directory = tempdir().unwrap();
        let mut completer = completer(directory.path(), &[]);
        *completer
            .preview
            .lock()
            .expect("completion preview lock poisoned") = Some(CompletionPreview {
            line: "cat local-one".to_string(),
            cursor: 13,
            span_start: 4,
            kind: CompletionKind::File,
            values: vec!["local-one".to_string(), "local-two".to_string()],
        });

        let suggestions = completer.complete("cat local-one", 13);
        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.value.as_str())
                .collect::<Vec<_>>(),
            vec!["local-one", "local-two"]
        );
        assert!(
            suggestions
                .iter()
                .all(|suggestion| suggestion.span == ReedlineSpan::new(4, 13))
        );
    }

    #[test]
    fn empty_completion_is_a_quick_no_op() {
        let directory = tempdir().unwrap();
        let mut completer = completer(directory.path(), &["printf"]);
        for line in ["# no completion", "missing-command"] {
            let suggestions = completer.complete(line, line.len());
            assert_eq!(suggestions.len(), 1);
            assert!(suggestions[0].value.is_empty());
            assert_eq!(suggestions[0].span.start, line.len());
            assert_eq!(suggestions[0].span.end, line.len());
            assert!(!suggestions[0].append_whitespace);
        }
    }

    #[test]
    fn file_completions_hide_dotfiles_until_requested() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("visible"), "").unwrap();
        fs::write(directory.path().join(".hidden"), "").unwrap();

        assert_eq!(
            super::file_completions("", directory.path(), None),
            vec!["visible".to_string()]
        );
        assert_eq!(
            super::file_completions(".h", directory.path(), None),
            vec![".hidden".to_string()]
        );
    }

    #[test]
    fn stale_history_paths_do_not_override_cwd_candidates() {
        let preview = CompletionPreview {
            line: "cat same-new".to_string(),
            cursor: 12,
            span_start: 4,
            kind: CompletionKind::File,
            values: vec!["same-new".to_string(), "same-next".to_string()],
        };
        assert!(super::history_hint_matches_preview(
            "cat same-",
            "new --flag",
            &preview
        ));
        assert!(!super::history_hint_matches_preview(
            "cat same-",
            "old",
            &preview
        ));

        let directory = CompletionPreview {
            line: "cat ../work/".to_string(),
            cursor: 12,
            span_start: 4,
            kind: CompletionKind::File,
            values: vec!["../work/".to_string()],
        };
        assert!(super::history_hint_matches_preview(
            "cat ../",
            "work/file.txt",
            &directory
        ));
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
        highlight_in(line, std::env::temp_dir())
    }

    fn highlight_in(line: &str, cwd: std::path::PathBuf) -> Vec<(Style, String)> {
        let context = josh_runtime::ShellContext::new(cwd.clone(), []);
        let snapshot = Arc::new(RwLock::new(Arc::new(CompletionSnapshot {
            commands: Arc::new(BTreeSet::from([
                "echo".to_string(),
                "local-tool".to_string(),
                "source".to_string(),
            ])),
            variables: Arc::new(BTreeSet::new()),
            cwd: Arc::new(cwd),
            home: None,
            session: Arc::new(context.snapshot()),
        })));
        let highlighter = JoshHighlighter {
            snapshot,
            palette: Palette::from_path(None).0,
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
    fn multi_token_command_names_share_validity_style() {
        let valid = Style::new().fg(Color::Green).bold();
        let buffer = highlight("local-tool arg");
        for token in ["local", "-", "tool"] {
            assert_eq!(style_of(&buffer, token), Some(&valid), "{token}");
        }
        assert_eq!(style_of(&buffer, "arg"), Some(&Style::new()));
    }

    #[cfg(unix)]
    #[test]
    fn executable_command_paths_are_highlighted_as_valid() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("local-tool");
        fs::write(&executable, "").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let valid = Style::new().fg(Color::Green).bold();
        let buffer = highlight_in("./local-tool arg", directory.path().to_path_buf());
        for token in [".", "/", "local", "-", "tool"] {
            assert_eq!(style_of(&buffer, token), Some(&valid), "{token}");
        }
    }

    #[test]
    fn source_is_highlighted_as_a_valid_command() {
        let valid = Style::new().fg(Color::Green).bold();
        let buffer = highlight("source init.josh");
        assert_eq!(style_of(&buffer, "source"), Some(&valid));
    }

    #[test]
    fn shell_conditions_highlight_their_command_heads() {
        let valid = Style::new().fg(Color::Green).bold();
        for source in ["while echo ok { break }", "status echo ok"] {
            let buffer = highlight(source);
            assert_eq!(style_of(&buffer, "echo"), Some(&valid), "{source}");
        }
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
