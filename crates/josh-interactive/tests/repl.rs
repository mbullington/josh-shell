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
    for source in [
        "printf x | pr",
        "printf x; pr",
        "printf x\npr",
        "printf x |pr",
        "if pr",
    ] {
        assert_eq!(
            analyzer.completion_context(source, source.len()).kind,
            CompletionKind::Command,
            "{source:?}"
        );
    }
    let file = analyzer.completion_context("cat Car", 7);
    assert_eq!(file.kind, CompletionKind::File);
    assert_eq!(
        analyzer.parse("if (true) {").completeness,
        Completeness::Incomplete
    );
}
