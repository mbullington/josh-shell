//! REPL syntax colors resolved from a TextMate theme (`.tmTheme`).
//!
//! `JOSH_THEME` points at a `.tmTheme` file. Each color slot probes the
//! theme with the scope selectors used by the editor grammar in
//! `editors/vscode` (generic prefixes like `string` still match, so any
//! standard theme applies), falling back to a fixed ANSI palette when no
//! selector has a foreground or no theme is configured. Only foregrounds
//! and font styles transfer; the terminal owns the background.

use std::env;

use nu_ansi_term::{Color, Style};
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{MatchPower, Scope, ScopeStack};

use josh_syntax::{LexMode, TokenKind};

#[derive(Debug, Clone)]
pub struct Palette {
    pub command_valid: Style,
    pub command_invalid: Style,
    pub string: Style,
    pub constant: Style,
    pub keyword: Style,
    pub comment: Style,
    pub variable: Style,
    pub capture: Style,
    pub expression: Style,
    pub error: Style,
    pub hint: Style,
}

impl Palette {
    /// Resolve the palette from `JOSH_THEME`. The second tuple element is a
    /// warning to print when a configured theme failed to load.
    #[must_use]
    pub fn from_environment() -> (Self, Option<String>) {
        match env::var_os("JOSH_THEME") {
            Some(path) if !path.is_empty() => match ThemeSet::get_theme(&path) {
                Ok(theme) => (Self::from_theme(&theme), None),
                Err(error) => (
                    Self::fallback(),
                    Some(format!(
                        "warning: could not load JOSH_THEME {}: {error}",
                        std::path::Path::new(&path).display()
                    )),
                ),
            },
            _ => (Self::fallback(), None),
        }
    }

    fn from_theme(theme: &Theme) -> Self {
        let plain = theme
            .settings
            .foreground
            .map_or_else(Style::new, |color| Style::new().fg(nu_color(color)));
        Self {
            command_valid: theme_style(
                theme,
                &[
                    "entity.name.function",
                    "support.function",
                    "variable.function",
                ],
            )
            .unwrap_or_else(|| Style::new().fg(Color::Green).bold()),
            command_invalid: theme_style(theme, &["invalid.illegal", "invalid"])
                .unwrap_or_else(|| Style::new().fg(Color::Red).bold()),
            string: theme_style(theme, &["string.quoted", "string"])
                .unwrap_or_else(|| Style::new().fg(Color::Yellow)),
            constant: theme_style(
                theme,
                &["constant.language", "constant.numeric", "constant"],
            )
            .unwrap_or_else(|| Style::new().fg(Color::Cyan)),
            keyword: theme_style(theme, &["keyword.control", "keyword"])
                .unwrap_or_else(|| Style::new().fg(Color::Blue).bold()),
            comment: theme_style(theme, &["comment"])
                .unwrap_or_else(|| Style::new().fg(Color::DarkGray)),
            variable: theme_style(theme, &["variable.other", "variable"])
                .unwrap_or_else(|| Style::new().fg(Color::Purple)),
            capture: theme_style(
                theme,
                &["punctuation.section.embedded", "meta.embedded", "variable"],
            )
            .unwrap_or_else(|| Style::new().fg(Color::Purple)),
            expression: plain,
            error: theme_style(theme, &["invalid.illegal", "invalid"])
                .unwrap_or_else(|| Style::new().fg(Color::Red).underline()),
            hint: theme_style(theme, &["comment"])
                .unwrap_or_else(|| Style::new().fg(Color::LightGray)),
        }
    }

    fn fallback() -> Self {
        Self {
            command_valid: Style::new().fg(Color::Green).bold(),
            command_invalid: Style::new().fg(Color::Red).bold(),
            string: Style::new().fg(Color::Yellow),
            constant: Style::new().fg(Color::Cyan),
            keyword: Style::new().fg(Color::Blue).bold(),
            comment: Style::new().fg(Color::DarkGray),
            variable: Style::new().fg(Color::Purple),
            capture: Style::new().fg(Color::Purple),
            expression: Style::new().fg(Color::LightBlue),
            error: Style::new().fg(Color::Red).underline(),
            hint: Style::new().fg(Color::LightGray),
        }
    }

    pub fn token_style(&self, kind: &TokenKind, mode: LexMode) -> Style {
        match kind {
            TokenKind::Whitespace | TokenKind::Newline => Style::new(),
            TokenKind::Comment => self.comment,
            TokenKind::SingleQuoted
            | TokenKind::DoubleQuoted
            | TokenKind::RawString
            | TokenKind::String => self.string,
            TokenKind::Number | TokenKind::True | TokenKind::False | TokenKind::Null => {
                self.constant
            }
            TokenKind::DollarVariable => self.variable,
            TokenKind::CaptureStart => self.capture,
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
            | TokenKind::Typeof => self.keyword,
            TokenKind::Unsupported(_) | TokenKind::Unknown => self.error,
            _ if mode == LexMode::Expression => self.expression,
            _ => Style::new(),
        }
    }
}

/// Best-matching style for one scope, probed under `source.josh`: the
/// highest selector match power wins, and for equal power the later theme
/// item overrides, mirroring syntect's application order. Theme items
/// without a foreground do not count so slots keep readable fallback
/// colors.
fn theme_style(theme: &Theme, candidates: &[&str]) -> Option<Style> {
    for candidate in candidates {
        let stack = ScopeStack::from_vec(vec![
            Scope::new("source.josh").ok()?,
            Scope::new(candidate).ok()?,
        ]);
        let mut best: Option<(MatchPower, Style)> = None;
        for item in &theme.scopes {
            if let Some(power) = item.scope.does_match(stack.as_slice())
                && let Some(foreground) = item.style.foreground
            {
                let mut style = Style::new().fg(nu_color(foreground));
                if let Some(font_style) = item.style.font_style {
                    if font_style.contains(FontStyle::BOLD) {
                        style = style.bold();
                    }
                    if font_style.contains(FontStyle::UNDERLINE) {
                        style = style.underline();
                    }
                    if font_style.contains(FontStyle::ITALIC) {
                        style = style.italic();
                    }
                }
                if best
                    .as_ref()
                    .is_none_or(|(best_power, _)| power >= *best_power)
                {
                    best = Some((power, style));
                }
            }
        }
        if best.is_some() {
            return best.map(|(_, style)| style);
        }
    }
    None
}

fn nu_color(color: syntect::highlighting::Color) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}
