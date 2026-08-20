use josh_syntax::{
    BinaryOp, BindingPattern, Completeness, Diagnostic, EnvironmentTarget, Expr, IfCondition,
    Label, ObjectEntry, QuotedPart, RedirectionKind, Severity, Span, Statement, WordPart, parse,
};

fn one(source: &str) -> Statement {
    let parsed = parse(source);
    assert_eq!(
        parsed.completeness,
        Completeness::Complete,
        "{source}: {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "{source}: {:?}",
        parsed.diagnostics
    );
    parsed
        .program
        .statements
        .into_iter()
        .next()
        .expect("one statement")
}

#[test]
fn environment_assignments_have_narrow_semantic_targets() {
    assert!(matches!(
        one("env.PATH = [\"/bin\"]"),
        Statement::EnvironmentAssignment {
            target: EnvironmentTarget::Member { ref name, .. },
            value: Expr::Array(_, _),
            ..
        } if name == "PATH"
    ));
    assert!(matches!(
        one("env[\"NAME\"] = null"),
        Statement::EnvironmentAssignment {
            target: EnvironmentTarget::Index {
                key: Expr::String(ref name, _),
                ..
            },
            value: Expr::Null(_),
            ..
        } if name == "NAME"
    ));
    assert!(matches!(
        one("object.key"),
        Statement::Expr(Expr::Member { .. })
    ));
}

#[test]
fn ambiguity_corpus_has_stable_statement_shapes() {
    assert!(matches!(one("ls -la"), Statement::Command(_)));
    assert!(matches!(
        one("x = 5"),
        Statement::Assignment {
            value: Expr::Int(5, _),
            ..
        }
    ));
    assert!(matches!(one("x(1, 2)"), Statement::Expr(Expr::Call { .. })));
    let Statement::Command(command) = one("x (1 + 2)") else {
        panic!("expected command")
    };
    assert!(matches!(
        command.stages[0].words[1].parts[0],
        WordPart::Evaluated { .. }
    ));
    assert!(
        matches!(one("items.filter(f)"), Statement::Expr(Expr::Call { callee, .. }) if matches!(*callee, Expr::Member { .. }))
    );
    let Statement::Command(pipeline) = one("items.filter(f) | take 1") else {
        panic!("expected value pipeline")
    };
    assert_eq!(pipeline.stages.len(), 2);
    let WordPart::Evaluated { expr, .. } = &pipeline.stages[0].words[0].parts[0] else {
        panic!("expected expression source")
    };
    assert!(matches!(expr.as_ref(), Expr::Call { .. }));
    assert!(matches!(one("x - 1"), Statement::Command(_)));
    assert!(
        matches!(one("let y = ls"), Statement::Let { value: Expr::Identifier(ref name, _), .. } if name == "ls")
    );
    assert!(matches!(one("x=>x"), Statement::Expr(Expr::Arrow { .. })));
    assert!(matches!(one("x => x"), Statement::Expr(Expr::Arrow { .. })));
    assert!(matches!(one("for foo"), Statement::Command(_)));
}

#[test]
fn if_condition_variants_are_explicit() {
    assert!(matches!(
        one("if (n > 3) { printf yes }"),
        Statement::Expr(Expr::If {
            condition: IfCondition::Expr(_),
            ..
        })
    ));
    assert!(matches!(
        one("if grep -q foo file { printf yes }"),
        Statement::Expr(Expr::If {
            condition: IfCondition::Command(_),
            ..
        })
    ));
    let Statement::Expr(Expr::If {
        condition: IfCondition::Command(command),
        ..
    }) = one("if printf '{' { printf yes }")
    else {
        panic!("expected command condition")
    };
    assert_eq!(command.stages[0].words.len(), 2);
    let Statement::Expr(Expr::If {
        condition: IfCondition::Command(command),
        ..
    }) = one(r"if printf \{ { printf yes }")
    else {
        panic!("expected command condition")
    };
    assert_eq!(command.stages[0].words.len(), 2);
}

#[test]
fn tokens_losslessly_partition_unicode_source() {
    let source = "let café = [1, 2]\nprintf 'λ' $café # fin\n";
    let parsed = parse(source);
    let mut cursor = 0;
    let mut rebuilt = String::new();
    for token in &parsed.tokens {
        assert_eq!(token.span.start, cursor);
        assert!(source.is_char_boundary(token.span.start));
        assert!(source.is_char_boundary(token.span.end));
        rebuilt.push_str(&source[token.span.range()]);
        cursor = token.span.end;
    }
    assert_eq!(cursor, source.len());
    assert_eq!(rebuilt, source);
}

#[test]
fn eof_only_errors_are_incomplete_but_hard_errors_are_invalid() {
    for source in ["echo '", "(1 +", "(1 +\n", "echo |", "x =", "if (true) {"] {
        assert_eq!(
            parse(source).completeness,
            Completeness::Incomplete,
            "{source}"
        );
    }
    assert_eq!(parse("(1 +\n2)").completeness, Completeness::Complete);
    assert_eq!(parse(")").completeness, Completeness::Invalid);
    let equality = parse("(x == 1)");
    assert_eq!(equality.completeness, Completeness::Invalid);
    assert!(
        equality
            .diagnostics
            .iter()
            .any(|d| d.code == "P162" && d.message.contains("==="))
    );
}

#[test]
fn strict_policy_reuses_tolerant_result() {
    let valid = parse("x = 5");
    let strict = valid.strict_program().expect("strict parse");
    assert!(std::ptr::eq(strict, &valid.program));
    let invalid = parse("echo |");
    let diagnostics = invalid.strict_program().expect_err("strict rejection");
    assert_eq!(diagnostics.as_ptr(), invalid.diagnostics.as_ptr());
}

#[test]
fn double_quote_interpolations_remain_semantic_parts() {
    let Statement::Command(command) = one("printf \"v=${1 + 2}:$(printf ok)\"") else {
        panic!("expected command")
    };
    let WordPart::DoubleQuoted { parts, .. } = &command.stages[0].words[1].parts[0] else {
        panic!("expected quote")
    };
    assert!(
        parts
            .iter()
            .any(|part| matches!(part, QuotedPart::Expression(_)))
    );
    assert!(
        parts
            .iter()
            .any(|part| matches!(part, QuotedPart::Capture(_)))
    );
}

#[test]
fn multiline_command_forms_make_progress_and_preserve_pipelines() {
    let Statement::Expr(Expr::If {
        condition: IfCondition::Command(condition),
        ..
    }) = one("if true\n{ printf yes }")
    else {
        panic!("expected command condition")
    };
    assert_eq!(condition.stages.len(), 1);

    for source in ["x = $(printf hi\n)", "x = $(printf hi;)"] {
        assert_eq!(
            parse(source).completeness,
            Completeness::Complete,
            "{source}"
        );
    }

    let Statement::Command(pipeline) = one("printf left |\nprintf right") else {
        panic!("expected pipeline")
    };
    assert_eq!(pipeline.stages.len(), 2);
}

#[test]
fn command_stages_cannot_be_empty() {
    for source in ["| printf right", "printf left | | cat", "printf left | ;"] {
        let parsed = parse(source);
        assert_eq!(parsed.completeness, Completeness::Invalid, "{source}");
        assert!(parsed.diagnostics.iter().any(|d| d.code == "P131"));
    }
}

#[test]
fn statements_require_separators() {
    assert_eq!(
        parse("x = 1 printf separator-was-ignored").completeness,
        Completeness::Invalid
    );
    assert_eq!(
        parse("x = 1\nprintf ok").completeness,
        Completeness::Complete
    );
    assert_eq!(
        parse("x = 1; printf ok").completeness,
        Completeness::Complete
    );
}

#[test]
fn nested_interpolations_must_consume_their_fragments() {
    for source in [
        "printf \"${1 2}\"",
        "printf \"${1 +}\"",
        "printf \"$(echo |)\"",
    ] {
        assert_eq!(
            parse(source).completeness,
            Completeness::Invalid,
            "{source}"
        );
    }
    assert_eq!(
        parse("printf \"${1 + 2}\"").completeness,
        Completeness::Complete
    );
}

#[test]
fn return_newlines_end_the_optional_operand() {
    for source in ["fn f() { return\n42 }", "fn f() { return # done\n42 }"] {
        let Statement::Function { body, .. } = one(source) else {
            panic!("expected function")
        };
        assert!(matches!(
            body.statements[0],
            Statement::Return { value: None, .. }
        ));
    }
    let Statement::Function { body, .. } = one("fn f() { return (\n42\n) }") else {
        panic!("expected function")
    };
    assert!(matches!(
        body.statements[0],
        Statement::Return {
            value: Some(Expr::Int(42, _)),
            ..
        }
    ));
}

#[test]
fn expression_double_quotes_decode_like_command_double_quotes() {
    let Statement::Assignment {
        value: Expr::String(value, _),
        ..
    } = one(r#"x = "a\"b\\c""#)
    else {
        panic!("expected string assignment")
    };
    assert_eq!(value, "a\"b\\c");

    let Statement::Assignment {
        value: Expr::Object(entries, _),
        ..
    } = one(r#"x = {"a\"b": "c\\d"}"#)
    else {
        panic!("expected object assignment")
    };
    let josh_syntax::ObjectEntry::Property {
        key,
        value: Expr::String(value, _),
        ..
    } = &entries[0]
    else {
        panic!("expected string property")
    };
    assert_eq!(key, "a\"b");
    assert_eq!(value, "c\\d");
}

#[test]
fn redirections_are_stage_nodes_and_comparisons_remain_expressions() {
    let Statement::Command(pipeline) = one("printf x > out 2>> errors 2>&1 < input &> all") else {
        panic!("expected command")
    };
    assert_eq!(pipeline.stages[0].words.len(), 2);
    assert_eq!(pipeline.stages[0].redirections.len(), 5);
    assert_eq!(
        pipeline.stages[0].redirections[0].kind,
        RedirectionKind::Output
    );
    assert_eq!(
        pipeline.stages[0].redirections[1].kind,
        RedirectionKind::ErrorAppend
    );
    assert_eq!(
        pipeline.stages[0].redirections[2].kind,
        RedirectionKind::ErrorToOutput
    );
    assert_eq!(
        pipeline.stages[0].redirections[3].kind,
        RedirectionKind::Input
    );
    assert_eq!(
        pipeline.stages[0].redirections[4].kind,
        RedirectionKind::OutputAndError
    );
    assert!(matches!(
        one("(left > right)"),
        Statement::Expr(Expr::Binary {
            op: BinaryOp::Greater,
            ..
        })
    ));
    assert!(matches!(
        one("(left >= right)"),
        Statement::Expr(Expr::Binary {
            op: BinaryOp::GreaterEq,
            ..
        })
    ));
}

#[test]
fn background_syntax_remains_reserved() {
    let source = "sleep 1 &";
    let parsed = parse(source);
    assert_eq!(parsed.completeness, Completeness::Invalid);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("not implemented")),
        "{:?}",
        parsed.diagnostics
    );
}

#[test]
fn functions_data_prerequisites_and_control_flow_have_semantic_nodes() {
    // Regression contract: implemented language forms must never degrade into command argv,
    // generic error nodes, or runtime-dependent parser guesses.
    for source in [
        "fn f({x}, [y, ...rest]) { return x + y }",
        "value = (x, y) => x ? y : null",
        "value = {a: 1, ...other}.a",
        "let {a: renamed, ...rest} = object",
        "while (ready) { if (stop) { break } else { continue } }",
        "loop { break }",
        "try { throw error(\"no\") } catch (problem) { return problem }",
        "missing-command || printf recovered && printf done",
        "status sh -c 'exit 7'",
    ] {
        let parsed = parse(source);
        assert_eq!(
            parsed.completeness,
            Completeness::Complete,
            "{source}: {:?}",
            parsed.diagnostics
        );
        assert!(parsed.diagnostics.is_empty(), "{source}");
    }

    assert!(matches!(
        one("fn f(x) { return x }"),
        Statement::Function { .. }
    ));
    assert!(matches!(one("loop { break }"), Statement::Loop { .. }));
    assert!(matches!(
        one("printf left && printf right"),
        Statement::CommandChain { .. }
    ));
    assert!(matches!(
        one("status sh -c 'exit 7'"),
        Statement::Status { .. }
    ));
}

#[test]
fn advanced_recovery_stays_lossless_and_classifies_eof_only_failures() {
    // Regression contract: new nested forms use the same tolerant parse and preserve every
    // source byte, so the REPL can recover without a second parser or delimiter counter.
    for source in [
        "fn f(x) {",
        "value = [1, ...",
        "try { throw 1 } catch (",
        "value = true ? 1 :",
        "printf ok &&",
    ] {
        let parsed = parse(source);
        assert_eq!(parsed.completeness, Completeness::Incomplete, "{source}");
        let rebuilt = parsed
            .tokens
            .iter()
            .map(|token| &source[token.span.range()])
            .collect::<String>();
        assert_eq!(rebuilt, source);
    }

    assert!(matches!(one("for item"), Statement::Command(_)));
    // Deliberately excluded surfaces; `source` graduated from this list
    // when it became the bash-style script-loading statement.
    for source in ["import thing", "export thing", "jobs", "fg", "bg"] {
        assert_eq!(
            parse(source).completeness,
            Completeness::Invalid,
            "{source}"
        );
    }
    assert_eq!(parse("sleep 1 &").completeness, Completeness::Invalid);
}

#[test]
fn range_slices_parse_with_optional_bounds() {
    let Statement::Expr(Expr::Slice { start, end, .. }) = one("a[0..2]") else {
        panic!("a[0..2] must parse as a slice")
    };
    assert!(matches!(*start.expect("start"), Expr::Int(0, _)));
    assert!(matches!(*end.expect("end"), Expr::Int(2, _)));

    let Statement::Expr(Expr::Slice { start, end, .. }) = one("a[..]") else {
        panic!("a[..] must parse as a slice")
    };
    assert!(start.is_none() && end.is_none());

    let Statement::Expr(Expr::Slice { start, end, .. }) = one("a[..2]") else {
        panic!("a[..2] must parse as a slice")
    };
    assert!(start.is_none() && end.is_some());

    let Statement::Expr(Expr::Slice { start, end, .. }) = one("a[1..]") else {
        panic!("a[1..] must parse as a slice")
    };
    assert!(start.is_some() && end.is_none());

    // Number literals split cleanly from the `..` token.
    assert!(parse("a[0..2]").diagnostics.is_empty());
    assert!(parse("a[1.5..3]").diagnostics.is_empty());
    // Inclusive `..=` is rejected with a dedicated diagnostic.
    let parsed = parse("a[0..=2]");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "P182"),
        "{:?}",
        parsed.diagnostics
    );
    // Bracket indexing without `..` stays an Index expression.
    let Statement::Expr(Expr::Index { .. }) = one("a[0]") else {
        panic!("a[0] must stay an index expression")
    };
}

#[test]
fn reserved_words_parse_as_explicit_property_keys() {
    // Regression contract: reserved words are legal IdentifierName keys (JS
    // semantics) in object literals, destructuring renames, and member
    // access, but never shorthand references or bindings.
    let Statement::Assignment {
        value: Expr::Object(entries, _),
        ..
    } = one("x = ({ status: 1, true: 2, typeof: 3 })")
    else {
        panic!("keyword-keyed object literal must parse")
    };
    for (entry, wanted) in entries.iter().zip(["status", "true", "typeof"]) {
        let ObjectEntry::Property { key, .. } = entry else {
            panic!("expected property entry")
        };
        assert_eq!(key, wanted);
    }

    let Statement::Let {
        pattern: BindingPattern::Object { entries, .. },
        ..
    } = one("let { status: s, catch: c } = o")
    else {
        panic!("keyword-keyed destructuring rename must parse")
    };
    assert!(matches!(
        &entries[..],
        [(first_key, BindingPattern::Name { name: first, .. }), (second_key, BindingPattern::Name { name: second, .. })]
            if first_key == "status" && first == "s" && second_key == "catch" && second == "c"
    ));

    assert!(matches!(
        one("o.true.status"),
        Statement::Expr(Expr::Member { .. })
    ));

    // Shorthand forms stay errors: keywords cannot reference or bind.
    for (source, code) in [("x = ({ status })", "P166"), ("let { status } = o", "P112")] {
        let parsed = parse(source);
        assert!(
            parsed.diagnostics.iter().any(|d| d.code == code),
            "{source}: {:?}",
            parsed.diagnostics
        );
    }
}

fn diagnostic(code: &'static str, span: Span, expected: &[&str]) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code,
        message: format!("message for {code}"),
        expected: expected.iter().map(|item| item.to_string()).collect(),
        primary: Label {
            span,
            message: String::new(),
        },
        secondary: vec![],
        eof_caused: span.start == span.end,
    }
}

#[test]
fn render_points_a_caret_at_the_offending_span() {
    // Regression contract: rendered diagnostics must keep pointing at the
    // exact line and char column of the span, across multi-line sources,
    // non-ASCII text, empty spans, and EOF anchors.
    let rendered =
        diagnostic("P999", Span::new(8, 8), &["expression"]).render("let x = ;", "<input>");
    assert_eq!(
        rendered,
        [
            "error: message for P999[P999]; expected expression",
            "  --> <input>:1:9",
            "  |",
            "1 | let x = ;",
            "  |         ^",
        ]
        .join("\n")
    );

    // Non-ASCII text before the span: columns count chars, not bytes.
    let rendered = diagnostic("P998", Span::new(13, 14), &[]).render("x = 🤖🤖 + ", "script.josh");
    assert_eq!(
        rendered,
        [
            "error: message for P998[P998]",
            "  --> script.josh:1:8",
            "  |",
            "1 | x = 🤖🤖 + ",
            "  |        ^",
        ]
        .join("\n")
    );

    // Multi-line sources select the containing line and underline the span.
    let rendered =
        diagnostic("P997", Span::new(8, 13), &[]).render("one\ntwo three\nfour", "<input>");
    assert_eq!(
        rendered,
        [
            "error: message for P997[P997]",
            "  --> <input>:2:5",
            "  |",
            "2 | two three",
            "  |     ^^^^^",
        ]
        .join("\n")
    );

    // EOF after a trailing newline anchors to the end of the last content line.
    let rendered = diagnostic("P996", Span::new(9, 9), &["`}`"]).render("fn f() {\n", "init.josh");
    assert_eq!(
        rendered,
        [
            "error: message for P996[P996]; expected `}`",
            "  --> init.josh:1:9",
            "  |",
            "1 | fn f() {",
            "  |         ^",
        ]
        .join("\n")
    );
}
