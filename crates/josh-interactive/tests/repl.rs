use josh_interactive::{CompletionKind, JoshValidator, ReplAnalyzer};
use josh_syntax::Completeness;
use reedline::{ValidationResult, Validator};

#[test]
fn validator_continues_only_incomplete_input() {
    let validator = JoshValidator;
    assert!(matches!(
        validator.validate("echo |"),
        ValidationResult::Incomplete
    ));
    assert!(matches!(
        validator.validate(")"),
        ValidationResult::Complete
    ));
    // Regression contract: new nested language forms continue only for EOF-caused parser
    // diagnostics; hard errors still submit immediately for interactive recovery.
    for source in [
        "fn f(x) {",
        "try { throw 1 } catch (",
        "value = true ? 1 :",
        "printf ok &&",
    ] {
        assert!(
            matches!(validator.validate(source), ValidationResult::Incomplete),
            "{source}"
        );
    }
    assert!(matches!(
        validator.validate("sleep 1 &"),
        ValidationResult::Complete
    ));
}

#[test]
fn completion_context_uses_utf8_safe_byte_spans() {
    let analyzer = ReplAnalyzer;
    let variable = analyzer.completion_context("printf $café", "printf $café".len());
    assert_eq!(variable.kind, CompletionKind::Variable);
    assert_eq!(variable.prefix, "café");
    assert!("printf $café".is_char_boundary(variable.span.start));
    assert!("printf $café".is_char_boundary(variable.span.end));

    let command = analyzer.completion_context("pri", 3);
    assert_eq!(command.kind, CompletionKind::Command);
    let midline = analyzer.completion_context("pritf", 3);
    assert_eq!(midline.kind, CompletionKind::Command);
    assert_eq!(midline.prefix, "pri");
    assert_eq!(&"pritf"[midline.span.range()], "pritf");
    for source in [
        "printf x | pr",
        "printf x; pr",
        "printf x\npr",
        "printf x |pr",
        "printf x && pr",
        "printf x || pr",
        "if pr",
        "while pr",
        "status pr",
        "command pr",
        "printf $(pr",
    ] {
        assert_eq!(
            analyzer.completion_context(source, source.len()).kind,
            CompletionKind::Command,
            "{source:?}"
        );
    }
    let file = analyzer.completion_context("cat Car", 7);
    assert_eq!(file.kind, CompletionKind::File);
    let command_path = analyzer.completion_context("./local-t", 9);
    assert_eq!(command_path.kind, CompletionKind::File);
    assert_eq!(command_path.prefix, "./local-t");
    let quoted = analyzer.completion_context("cat \"foo b", 10);
    assert_eq!(quoted.kind, CompletionKind::File);
    assert_eq!(quoted.prefix, "foo b");
    assert_eq!(&"cat \"foo b"[quoted.span.range()], "foo b");
    let escaped = analyzer.completion_context("cat foo\\ b", 10);
    assert_eq!(escaped.kind, CompletionKind::File);
    assert_eq!(escaped.prefix, "foo b");
    assert_eq!(&"cat foo\\ b"[escaped.span.range()], "foo\\ b");

    let bare_variable = analyzer.completion_context("printf $", 8);
    assert_eq!(bare_variable.kind, CompletionKind::Variable);
    assert_eq!(bare_variable.prefix, "");
    let embedded_variable = analyzer.completion_context("printf pre$HO", 13);
    assert_eq!(embedded_variable.kind, CompletionKind::Variable);
    assert_eq!(embedded_variable.prefix, "HO");
    assert_eq!(&"printf pre$HO"[embedded_variable.span.range()], "HO");
    assert_eq!(
        analyzer.completion_context("printf command pr", 17).kind,
        CompletionKind::File
    );

    let comment = analyzer.completion_context("printf ok # no completion", 25);
    assert_eq!(comment.kind, CompletionKind::None);
    assert_eq!(
        analyzer.parse("if (true) {").completeness,
        Completeness::Incomplete
    );
}
