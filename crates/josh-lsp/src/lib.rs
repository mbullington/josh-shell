//! Errors-only language server for Josh.
//!
//! A pure mapping layer over [`josh_syntax::parse`]: each `didOpen`/`didChange`
//! carries the full document (the server keeps no state), the parse emits zero
//! or more diagnostics, and those are published back with UTF-8 byte spans
//! converted to LSP UTF-16 positions.

#![forbid(unsafe_code)]

use std::{io, sync::Arc};

use josh_syntax::{Diagnostic as JoshDiagnostic, Severity, Span};
use tower_lsp::{
    Client, LanguageServer, LspService, Server,
    jsonrpc::Result,
    lsp_types::{
        Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, DidChangeTextDocumentParams,
        DidCloseTextDocumentParams, DidOpenTextDocumentParams, InitializeParams, InitializeResult,
        Location, NumberOrString, Position, Range, ServerCapabilities, ServerInfo,
        TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    },
};

/// Serve the Josh language server over stdin/stdout until stdin closes. The
/// client signals shutdown via the `exit` notification, after which the
/// server rejects further requests; the process itself terminates when the
/// client closes stdin (editor clients do this immediately).
///
/// Stdout must carry nothing but protocol frames, so this runs on its own
/// runtime and never touches the shell engine.
pub fn run() -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let (service, socket) = LspService::new(|client| Backend { client });
        Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
            .serve(service)
            .await;
    });
    Ok(())
}

struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "josh-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        self.check(document.uri, document.text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full-document sync: each change carries the whole text; last wins.
        if let Some(change) = params.content_changes.into_iter().last() {
            self.check(params.text_document.uri, change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // Clear squiggles for the closed document.
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }
}

impl Backend {
    async fn check(&self, uri: Url, text: String) {
        let source: Arc<str> = Arc::from(text);
        let index = LineIndex::new(&source);
        let parsed = josh_syntax::parse(Arc::clone(&source));
        let diagnostics = parsed
            .diagnostics
            .iter()
            .map(|diagnostic| map_diagnostic(&source, &index, &uri, diagnostic))
            .collect();
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

/// Precomputed line-start byte offsets for UTF-8 → UTF-16 position mapping.
struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self { line_starts }
    }

    fn position(&self, text: &str, offset: usize) -> Position {
        // EOF-caused diagnostics point at `text.len()`; clamp anything past
        // the end, and a non-boundary offset back to the char start.
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        };
        let column = text[self.line_starts[line]..offset]
            .chars()
            .map(char::len_utf16)
            .sum::<usize>();
        Position::new(saturate_u32(line), saturate_u32(column))
    }

    fn range(&self, text: &str, span: Span) -> Range {
        Range::new(
            self.position(text, span.start),
            self.position(text, span.end),
        )
    }
}

const fn saturate_u32(value: usize) -> u32 {
    if value > u32::MAX as usize {
        u32::MAX
    } else {
        value as u32
    }
}

fn map_diagnostic(
    text: &str,
    index: &LineIndex,
    uri: &Url,
    diagnostic: &JoshDiagnostic,
) -> Diagnostic {
    let severity = match diagnostic.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
    };
    let message = if diagnostic.expected.is_empty() {
        diagnostic.message.clone()
    } else {
        format!(
            "{}; expected {}",
            diagnostic.message,
            diagnostic.expected.join(", ")
        )
    };
    let related: Vec<DiagnosticRelatedInformation> = diagnostic
        .secondary
        .iter()
        .map(|label| DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: index.range(text, label.span),
            },
            message: label.message.clone(),
        })
        .collect();
    Diagnostic {
        range: index.range(text, diagnostic.primary.span),
        severity: Some(severity),
        code: Some(NumberOrString::String(diagnostic.code.to_owned())),
        source: Some("josh".to_owned()),
        message,
        related_information: if related.is_empty() {
            None
        } else {
            Some(related)
        },
        ..Diagnostic::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri() -> Url {
        Url::parse("file:///tmp/test.josh").expect("valid test URI")
    }

    fn josh_diagnostic(
        severity: Severity,
        code: &'static str,
        message: &str,
        primary: Span,
    ) -> JoshDiagnostic {
        JoshDiagnostic {
            severity,
            code,
            message: message.to_owned(),
            expected: Vec::new(),
            primary: josh_syntax::Label {
                span: primary,
                message: String::new(),
            },
            secondary: Vec::new(),
            eof_caused: false,
        }
    }

    #[test]
    fn ascii_offsets_map_line_and_column() {
        let text = "let x = 1\necho hi\n";
        let index = LineIndex::new(text);
        assert_eq!(index.position(text, 0), Position::new(0, 0));
        assert_eq!(index.position(text, 7), Position::new(0, 7));
        assert_eq!(index.position(text, 10), Position::new(1, 0));
        assert_eq!(index.position(text, 15), Position::new(1, 5));
        assert_eq!(index.position(text, 18), Position::new(2, 0));
    }

    #[test]
    fn multibyte_chars_count_code_units_not_bytes() {
        let text = "let π = 1"; // π: 2 UTF-8 bytes, 1 UTF-16 unit
        let index = LineIndex::new(text);
        assert_eq!(index.position(text, 6), Position::new(0, 5));
        assert_eq!(index.position(text, 11), Position::new(0, 9));
    }

    #[test]
    fn surrogate_pairs_count_two_units() {
        let text = "a🦀b"; // 🦀: 4 UTF-8 bytes, 2 UTF-16 units
        let index = LineIndex::new(text);
        assert_eq!(index.position(text, 1), Position::new(0, 1));
        assert_eq!(index.position(text, 5), Position::new(0, 3));
        assert_eq!(index.position(text, 6), Position::new(0, 4));
    }

    #[test]
    fn offsets_clamp_to_document_end() {
        let text = "ab\ncd";
        let index = LineIndex::new(text);
        assert_eq!(index.position(text, 5), Position::new(1, 2));
        assert_eq!(index.position(text, 999), Position::new(1, 2));
    }

    #[test]
    fn mid_char_offsets_snap_back_to_char_start() {
        let text = "a🦀b";
        let index = LineIndex::new(text);
        assert_eq!(index.position(text, 3), Position::new(0, 1));
    }

    #[test]
    fn eof_diagnostic_maps_to_end_position() {
        let source = "let x = 'abc";
        let parsed = josh_syntax::parse(source);
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L001")
            .expect("unclosed quote diagnostic");
        assert!(diagnostic.eof_caused);
        let index = LineIndex::new(source);
        let mapped = map_diagnostic(source, &index, &uri(), diagnostic);
        assert_eq!(
            mapped.range,
            Range::new(Position::new(0, 12), Position::new(0, 12))
        );
    }

    #[test]
    fn real_parse_range_maps_across_multibyte_prefix() {
        let source = "let π = 1 == 2";
        let parsed = josh_syntax::parse(source);
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "P162")
            .expect("`==` diagnostic");
        assert_eq!(diagnostic.primary.span, Span::new(11, 13));
        let index = LineIndex::new(source);
        let mapped = map_diagnostic(source, &index, &uri(), diagnostic);
        assert_eq!(
            mapped.range,
            Range::new(Position::new(0, 10), Position::new(0, 12))
        );
        assert_eq!(mapped.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(mapped.source.as_deref(), Some("josh"));
        assert_eq!(mapped.code, Some(NumberOrString::String("P162".to_owned())));
    }

    #[test]
    fn message_appends_expected_list_like_display() {
        let source = "let x = \"abc";
        let parsed = josh_syntax::parse(source);
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L001")
            .expect("unclosed quote diagnostic");
        let index = LineIndex::new(source);
        let mapped = map_diagnostic(source, &index, &uri(), diagnostic);
        assert_eq!(mapped.message, "unclosed \" quote; expected \"");
    }

    #[test]
    fn secondary_labels_become_related_information() {
        let text = "🦀 rest";
        let index = LineIndex::new(text);
        let mut diagnostic = josh_diagnostic(Severity::Warning, "P999", "problem", Span::new(0, 4));
        diagnostic.secondary.push(josh_syntax::Label {
            span: Span::new(5, 9),
            message: "see here".to_owned(),
        });
        let mapped = map_diagnostic(text, &index, &uri(), &diagnostic);
        assert_eq!(mapped.severity, Some(DiagnosticSeverity::WARNING));
        let related = mapped.related_information.expect("secondary label mapped");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].message, "see here");
        assert_eq!(
            related[0].location.range,
            Range::new(Position::new(0, 3), Position::new(0, 7))
        );
        assert!(
            mapped.range.start == Position::new(0, 0) && mapped.range.end == Position::new(0, 2)
        );
    }

    #[test]
    fn empty_secondary_labels_stay_absent() {
        let text = "x";
        let index = LineIndex::new(text);
        let diagnostic = josh_diagnostic(Severity::Error, "P998", "bad", Span::new(0, 1));
        let mapped = map_diagnostic(text, &index, &uri(), &diagnostic);
        assert!(mapped.related_information.is_none());
    }
}
