//! Width-aware value rendering for interactive display: containers print flat
//! when they fit in the remaining columns and break across indented lines when
//! they do not, with optional ANSI styling on atoms.

use nu_ansi_term::{Color, Style};
use unicode_ident::{is_xid_continue, is_xid_start};

use crate::value::Value;

const INDENT: usize = 2;
const DEFAULT_WIDTH: usize = 100;

#[derive(Debug, Clone, Copy)]
pub struct PrettyOptions {
    pub width: usize,
    pub colors: bool,
}

impl Default for PrettyOptions {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            colors: false,
        }
    }
}

#[must_use]
pub fn render_value(value: &Value, options: &PrettyOptions) -> String {
    let mut output = String::new();
    Printer { options }.value(&mut output, value, 0, 0);
    output
}

struct Printer<'a> {
    options: &'a PrettyOptions,
}

impl Printer<'_> {
    /// `indent` is the nesting depth in columns used for any newlines this
    /// value emits; `column` is the horizontal offset of the value's first
    /// character on its current line.
    fn value(&self, output: &mut String, value: &Value, indent: usize, column: usize) {
        let breakable = matches!(value, Value::Array(_) | Value::Object(_));
        let remaining = self.options.width.saturating_sub(column);
        if !breakable || matches!(flat_width(value, remaining), Some(width) if width <= remaining) {
            self.flat(output, value);
        } else {
            self.broken(output, value, indent);
        }
    }

    fn flat(&self, output: &mut String, value: &Value) {
        match value {
            Value::Array(items) => {
                output.push('[');
                for (index, item) in items.snapshot().iter().enumerate() {
                    if index > 0 {
                        output.push_str(", ");
                    }
                    self.flat(output, item);
                }
                output.push(']');
            }
            Value::Object(object) => {
                output.push('{');
                for (index, (key, item)) in object.snapshot().iter().enumerate() {
                    if index > 0 {
                        output.push_str(", ");
                    }
                    output.push_str(&render_key(key));
                    output.push_str(": ");
                    self.flat(output, item);
                }
                output.push('}');
            }
            _ => self.atom(output, value),
        }
    }

    fn broken(&self, output: &mut String, value: &Value, indent: usize) {
        let child_indent = indent + INDENT;
        match value {
            Value::Array(items) => {
                output.push('[');
                for (index, item) in items.snapshot().iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    newline_indent(output, child_indent);
                    self.value(output, item, child_indent, child_indent);
                }
                newline_indent(output, indent);
                output.push(']');
            }
            Value::Object(object) => {
                output.push('{');
                for (index, (key, item)) in object.snapshot().iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    newline_indent(output, child_indent);
                    let key_width = render_key(key).chars().count();
                    output.push_str(&render_key(key));
                    output.push_str(": ");
                    self.value(output, item, child_indent, child_indent + key_width + 2);
                }
                newline_indent(output, indent);
                output.push('}');
            }
            _ => self.atom(output, value),
        }
    }

    fn atom(&self, output: &mut String, value: &Value) {
        let (text, class) = atom_text(value);
        let style = match (class, self.options.colors) {
            (AtomClass::String, true) => Style::new().fg(Color::Yellow),
            (AtomClass::Literal, true) => Style::new().fg(Color::Cyan),
            _ => Style::new(),
        };
        output.push_str(&style.paint(text).to_string());
    }
}

/// One-line width of `value`, or `None` once it exceeds `limit` (early exit
/// so large structures cost O(limit) rather than O(n) here).
fn flat_width(value: &Value, limit: usize) -> Option<usize> {
    let width = match value {
        Value::Array(items) => {
            let items = items.snapshot();
            let mut width = 2_usize;
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    width += 2;
                }
                width += flat_width(item, limit)?;
            }
            width
        }
        Value::Object(object) => {
            let entries = object.snapshot();
            let mut width = 2_usize;
            for (index, (key, item)) in entries.iter().enumerate() {
                if index > 0 {
                    width += 2;
                }
                width += render_key(key).chars().count() + 2 + flat_width(item, limit)?;
            }
            width
        }
        _ => atom_text(value).0.chars().count(),
    };
    (width <= limit).then_some(width)
}

enum AtomClass {
    String,
    Literal,
    Plain,
}

fn atom_text(value: &Value) -> (String, AtomClass) {
    match value {
        Value::String(text) => (quoted(text), AtomClass::String),
        Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::Null => {
            (value.to_string(), AtomClass::Literal)
        }
        _ => (value.to_string(), AtomClass::Plain),
    }
}

fn newline_indent(output: &mut String, columns: usize) {
    output.push('\n');
    for _ in 0..columns {
        output.push(' ');
    }
}

/// A member key is printed bare when it is a valid Josh identifier (matching
/// object-literal syntax) and JSON-quoted otherwise.
fn render_key(key: &str) -> String {
    if is_identifier(key) {
        key.into()
    } else {
        quoted(key)
    }
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || is_xid_start(first))
        && chars.all(|ch| ch == '$' || is_xid_continue(ch))
}

/// JSON escapes, without the `serde_json` boundary semantics: the shell only
/// displays this, so control characters below U+0020 become short escapes and
/// everything printable stays literal UTF-8.
fn quoted(text: &str) -> String {
    let mut output = String::with_capacity(text.len() + 2);
    output.push('"');
    for character in text.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let code = character as u8;
                output.push_str("\\u00");
                output.push(HEX[(code >> 4) as usize] as char);
                output.push(HEX[(code & 0x0f) as usize] as char);
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::{PrettyOptions, render_value};
    use crate::value::{ArrayValue, JoshStr, ObjectValue, Value};
    use std::sync::Arc;

    fn string(text: &str) -> Value {
        Value::String(JoshStr::from(text))
    }

    fn render(value: &Value, width: usize) -> String {
        render_value(
            value,
            &PrettyOptions {
                width,
                colors: false,
            },
        )
    }

    fn object(entries: &[(&str, Value)]) -> Value {
        Value::Object(Arc::new(ObjectValue::from_entries(
            entries
                .iter()
                .map(|(key, value)| (Arc::from(*key), value.clone())),
        )))
    }

    fn array(items: &[Value]) -> Value {
        Value::Array(Arc::new(ArrayValue::from_vec(items.to_vec())))
    }

    #[test]
    fn scalars_render_on_one_line() {
        assert_eq!(render(&Value::Int(42), 20), "42");
        assert_eq!(render(&Value::Null, 20), "null");
        assert_eq!(render(&Value::Bool(true), 20), "true");
        assert_eq!(render(&string("hi"), 20), "\"hi\"");
    }

    #[test]
    fn containers_stay_flat_when_they_fit() {
        let value = object(&[
            ("name", string("josh")),
            ("tags", array(&[string("a"), string("b")])),
        ]);
        assert_eq!(render(&value, 35), "{name: \"josh\", tags: [\"a\", \"b\"]}");
    }

    #[test]
    fn containers_break_when_they_do_not_fit() {
        let value = object(&[
            ("name", string("josh")),
            ("tags", array(&[string("a"), string("b")])),
        ]);
        assert_eq!(
            render(&value, 20),
            "{\n  name: \"josh\",\n  tags: [\"a\", \"b\"]\n}"
        );
    }

    #[test]
    fn breaking_repeats_recursively() {
        let value = object(&[
            ("name", string("josh")),
            ("tags", array(&[string("a"), string("b")])),
        ]);
        assert_eq!(
            render(&value, 12),
            "{\n  name: \"josh\",\n  tags: [\n    \"a\",\n    \"b\"\n  ]\n}"
        );
    }

    #[test]
    fn non_identifier_keys_are_quoted() {
        let value = object(&[("a-b", Value::Int(1)), ("x", Value::Int(2))]);
        assert_eq!(render(&value, 10), "{\n  \"a-b\": 1,\n  x: 2\n}");
        assert_eq!(render(&value, 30), "{\"a-b\": 1, x: 2}");
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(render(&string("a\nb\u{7}\""), 30), "\"a\\nb\\u0007\\\"\"");
    }

    #[test]
    fn empty_containers_print_compact() {
        assert_eq!(render(&array(&[]), 10), "[]");
        assert_eq!(render(&object(&[]), 10), "{}");
    }

    #[test]
    fn atoms_never_break_their_own_line() {
        let value = object(&[("message", string("a rather long string indeed"))]);
        assert_eq!(
            render(&value, 10),
            "{\n  message: \"a rather long string indeed\"\n}"
        );
    }

    #[test]
    fn colors_wrap_atoms_in_ansi_when_enabled() {
        let colored = render_value(
            &string("hi"),
            &PrettyOptions {
                width: 20,
                colors: true,
            },
        );
        assert_eq!(colored, "\u{1b}[33m\"hi\"\u{1b}[0m");
    }
}
