#![forbid(unsafe_code)]

use std::{fmt, ops::Range, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    #[must_use]
    pub const fn empty(at: usize) -> Self {
        Self::new(at, at)
    }
    #[must_use]
    pub const fn range(self) -> Range<usize> {
        self.start..self.end
    }
    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        Self::new(self.start, other.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexMode {
    Command,
    Expression,
    SingleQuote,
    DoubleQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Whitespace,
    Comment,
    Newline,
    Word,
    Identifier,
    Number,
    String,
    SingleQuoted,
    DoubleQuoted,
    DollarVariable,
    CaptureStart,
    InterpolationStart,
    Let,
    Fn,
    If,
    Else,
    While,
    Loop,
    Try,
    Catch,
    Throw,
    Return,
    Break,
    Continue,
    Status,
    Typeof,
    True,
    False,
    Null,
    Assign,
    PlusAssign,
    MinusAssign,
    Pipe,
    Semicolon,
    Comma,
    Dot,
    Ellipsis,
    Colon,
    Question,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Plus,
    Minus,
    Star,
    Slash,
    DoubleSlash,
    Percent,
    Bang,
    AndAnd,
    OrOr,
    StrictEq,
    StrictNotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    RedirectAppend,
    RedirectError,
    RedirectErrorAppend,
    RedirectErrorToOutput,
    RedirectOutputAndError,
    Arrow,
    Unsupported(&'static str),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub mode: LexMode,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completeness {
    Complete,
    Incomplete,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub expected: Vec<String>,
    pub primary: Label,
    pub secondary: Vec<Label>,
    pub eof_caused: bool,
}

impl Diagnostic {
    fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            expected: vec![],
            primary: Label {
                span,
                message: String::new(),
            },
            secondary: vec![],
            eof_caused: false,
        }
    }
    fn eof(code: &'static str, message: impl Into<String>, at: usize, expected: &[&str]) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            expected: expected.iter().map(ToString::to_string).collect(),
            primary: Label {
                span: Span::empty(at),
                message: String::new(),
            },
            secondary: vec![],
            eof_caused: true,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}] at {}..{}",
            self.message, self.code, self.primary.span.start, self.primary.span.end
        )?;
        if !self.expected.is_empty() {
            write!(f, "; expected {}", self.expected.join(", "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Command(Pipeline),
    CommandChain {
        head: Pipeline,
        tail: Vec<(ChainOp, Pipeline)>,
        span: Span,
    },
    Status {
        pipeline: Pipeline,
        span: Span,
    },
    Assignment {
        name: String,
        op: AssignOp,
        value: Expr,
        span: Span,
    },
    EnvironmentAssignment {
        target: EnvironmentTarget,
        value: Expr,
        span: Span,
    },
    Let {
        pattern: BindingPattern,
        value: Expr,
        span: Span,
    },
    Function {
        name: String,
        params: Vec<BindingPattern>,
        body: Program,
        span: Span,
    },
    Expr(Expr),
    While {
        condition: IfCondition,
        body: Program,
        span: Span,
    },
    Loop {
        body: Program,
        span: Span,
    },
    Throw {
        value: Expr,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Missing {
        span: Span,
    },
    Error {
        span: Span,
    },
}

impl Statement {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Command(x) => x.span,
            Self::CommandChain { span, .. }
            | Self::Status { span, .. }
            | Self::Assignment { span, .. }
            | Self::EnvironmentAssignment { span, .. }
            | Self::Let { span, .. }
            | Self::Function { span, .. }
            | Self::While { span, .. }
            | Self::Loop { span, .. }
            | Self::Throw { span, .. }
            | Self::Return { span, .. }
            | Self::Break { span }
            | Self::Continue { span }
            | Self::Missing { span }
            | Self::Error { span } => *span,
            Self::Expr(x) => x.span(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Add,
    Subtract,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentTarget {
    Member { name: String, span: Span },
    Index { key: Expr, span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingPattern {
    Name {
        name: String,
        span: Span,
    },
    Array {
        items: Vec<BindingPattern>,
        rest: Option<Box<BindingPattern>>,
        span: Span,
    },
    Object {
        entries: Vec<(String, BindingPattern)>,
        rest: Option<Box<BindingPattern>>,
        span: Span,
    },
    Missing {
        span: Span,
    },
}

impl BindingPattern {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Name { span, .. }
            | Self::Array { span, .. }
            | Self::Object { span, .. }
            | Self::Missing { span } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IfCondition {
    Expr(Box<Expr>),
    Command(Pipeline),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub stages: Vec<ExternalCommand>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalCommand {
    pub words: Vec<CommandWord>,
    pub redirections: Vec<Redirection>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Redirection {
    pub kind: RedirectionKind,
    pub target: Option<CommandWord>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectionKind {
    Input,
    Output,
    Append,
    Error,
    ErrorAppend,
    ErrorToOutput,
    OutputAndError,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandWord {
    pub parts: Vec<WordPart>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WordPart {
    Literal {
        value: String,
        glob_unquoted: bool,
        span: Span,
    },
    SingleQuoted {
        value: String,
        span: Span,
    },
    DoubleQuoted {
        parts: Vec<QuotedPart>,
        span: Span,
    },
    Variable {
        name: String,
        span: Span,
    },
    Capture {
        pipeline: Box<Pipeline>,
        span: Span,
    },
    Evaluated {
        expr: Box<Expr>,
        span: Span,
    },
    Missing {
        span: Span,
    },
    Error {
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuotedPart {
    Literal(String),
    Variable(String),
    Capture(Box<Pipeline>),
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Null(Span),
    Bool(bool, Span),
    Int(i64, Span),
    Float(f64, Span),
    String(String, Span),
    Identifier(String, Span),
    Array(Vec<ArrayElement>, Span),
    Object(Vec<ObjectEntry>, Span),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
        span: Span,
    },
    Member {
        object: Box<Expr>,
        name: String,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Arrow {
        params: Vec<BindingPattern>,
        body: FunctionBody,
        span: Span,
    },
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },
    If {
        condition: IfCondition,
        then_block: Program,
        else_block: Option<Program>,
        span: Span,
    },
    Try {
        body: Program,
        catch_pattern: BindingPattern,
        catch_body: Program,
        span: Span,
    },
    Capture {
        pipeline: Box<Pipeline>,
        span: Span,
    },
    Missing(Span),
    Error(Span),
}

impl Expr {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Null(x)
            | Self::Bool(_, x)
            | Self::Int(_, x)
            | Self::Float(_, x)
            | Self::String(_, x)
            | Self::Identifier(_, x)
            | Self::Array(_, x)
            | Self::Object(_, x)
            | Self::Missing(x)
            | Self::Error(x) => *x,
            Self::If { span, .. } | Self::Try { span, .. } => *span,
            Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Call { span, .. }
            | Self::Member { span, .. }
            | Self::Index { span, .. }
            | Self::Arrow { span, .. }
            | Self::Ternary { span, .. }
            | Self::Capture { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayElement {
    Value(Expr),
    Spread(Expr, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectEntry {
    Property {
        key: String,
        value: Expr,
        span: Span,
    },
    Spread {
        value: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Value(Expr),
    Spread(Expr, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionBody {
    Expression(Box<Expr>),
    Block(Program),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
    Typeof,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    IntegerDivide,
    Remainder,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Equal,
    NotEqual,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub struct Parse {
    pub source: Arc<str>,
    pub tokens: Vec<Token>,
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
    pub completeness: Completeness,
}

impl Parse {
    pub fn strict_program(&self) -> Result<&Program, &[Diagnostic]> {
        if self.completeness == Completeness::Complete
            && !self
                .diagnostics
                .iter()
                .any(|x| x.severity == Severity::Error)
        {
            Ok(&self.program)
        } else {
            Err(&self.diagnostics)
        }
    }
}

pub fn parse(source: impl Into<Arc<str>>) -> Parse {
    let source = source.into();
    Parser::new(Arc::clone(&source)).finish()
}

struct Parser {
    source: Arc<str>,
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    fn new(source: Arc<str>) -> Self {
        let (tokens, diagnostics) = lex(&source);
        Self {
            source,
            tokens,
            pos: 0,
            diagnostics,
        }
    }

    fn finish(mut self) -> Parse {
        let program = self.parse_program(false);
        let has_error = self
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error);
        let completeness = if !has_error {
            Completeness::Complete
        } else if self.diagnostics.iter().all(|d| d.eof_caused) {
            Completeness::Incomplete
        } else {
            Completeness::Invalid
        };
        Parse {
            source: self.source,
            tokens: self.tokens,
            program,
            diagnostics: self.diagnostics,
            completeness,
        }
    }

    fn parse_program(&mut self, in_block: bool) -> Program {
        let start = self.current_start();
        let mut statements = Vec::new();
        loop {
            self.skip_separators();
            if self.at_end() || (in_block && self.at(TokenTag::RightBrace)) {
                break;
            }
            if self.at(TokenTag::RightBrace) {
                let span = self.bump_span(LexMode::Command);
                self.diagnostics
                    .push(Diagnostic::error("P101", "unexpected `}`", span));
                statements.push(Statement::Error { span });
                continue;
            }
            let before = self.pos;
            let statement = self.parse_statement();
            let statement_end = statement.span().end;
            let consumed_separator = self.tokens[before..self.pos].iter().any(|token| {
                matches!(token.kind, TokenKind::Newline | TokenKind::Semicolon)
                    && token.span.start >= statement_end
            });
            statements.push(statement);
            self.skip_trivia_mode(LexMode::Command);
            if self.pos == before {
                let span = self.bump_span(LexMode::Command);
                self.diagnostics
                    .push(Diagnostic::error("P102", "could not parse token", span));
            } else if !consumed_separator && !self.at_statement_boundary(in_block) {
                let at = self.current_start();
                self.diagnostics.push(self.expected(
                    "P103",
                    "statements must be separated by a newline or `;`",
                    at,
                    &["newline", ";"],
                ));
                while !self.at_statement_boundary(in_block) {
                    self.bump_mode(LexMode::Command);
                }
            }
        }
        let end = statements.last().map_or(start, |s| s.span().end);
        Program {
            statements,
            span: Span::new(start, end),
        }
    }

    fn parse_statement(&mut self) -> Statement {
        match self.peek_tag() {
            Some(TokenTag::Let) => return self.parse_let(),
            Some(TokenTag::Fn) => return self.parse_function(),
            Some(TokenTag::If) => return Statement::Expr(self.parse_if_expression()),
            Some(TokenTag::While) => return self.parse_while(),
            Some(TokenTag::Loop) => return self.parse_loop(),
            Some(TokenTag::Try) => return Statement::Expr(self.parse_try_expression()),
            Some(TokenTag::Throw) => return self.parse_throw(),
            Some(TokenTag::Return) => return self.parse_return(),
            Some(TokenTag::Break) => {
                let span = self.bump_span(LexMode::Expression);
                return Statement::Break { span };
            }
            Some(TokenTag::Continue) => {
                let span = self.bump_span(LexMode::Expression);
                return Statement::Continue { span };
            }
            Some(TokenTag::Status) => return self.parse_status(),
            _ => {}
        }
        if self.is_assignment_head() {
            return self.parse_assignment();
        }
        if self.is_environment_assignment_head() {
            return self.parse_environment_assignment();
        }
        if self.is_expression_head() {
            return Statement::Expr(self.parse_expr(0));
        }
        self.parse_command_chain()
    }

    fn is_assignment_head(&self) -> bool {
        if self.peek_tag() != Some(TokenTag::Identifier) {
            return false;
        }
        matches!(
            self.nth_significant_tag(1),
            Some(TokenTag::Assign | TokenTag::PlusAssign | TokenTag::MinusAssign)
        )
    }

    fn is_environment_assignment_head(&self) -> bool {
        let Some(first) = self.peek_significant_index(0) else {
            return false;
        };
        if tag(&self.tokens[first].kind) != TokenTag::Identifier
            || &self.source[self.tokens[first].span.range()] != "env"
        {
            return false;
        }
        let Some(access) = self.peek_significant_index(1) else {
            return false;
        };
        if self.tokens[first].span.end != self.tokens[access].span.start {
            return false;
        }
        match tag(&self.tokens[access].kind) {
            TokenTag::Dot => matches!(
                (self.nth_significant_tag(2), self.nth_significant_tag(3)),
                (Some(TokenTag::Identifier), Some(TokenTag::Assign))
            ),
            TokenTag::LeftBracket => {
                let mut square_depth = 0_usize;
                let mut paren_depth = 0_usize;
                let mut brace_depth = 0_usize;
                for (index, token) in self.tokens.iter().enumerate().skip(access) {
                    match tag(&token.kind) {
                        TokenTag::LeftBracket => square_depth += 1,
                        TokenTag::RightBracket => {
                            square_depth = square_depth.saturating_sub(1);
                            if square_depth == 0 && paren_depth == 0 && brace_depth == 0 {
                                return self.tokens[index + 1..]
                                    .iter()
                                    .find(|token| {
                                        !matches!(
                                            token.kind,
                                            TokenKind::Whitespace | TokenKind::Comment
                                        )
                                    })
                                    .is_some_and(|token| tag(&token.kind) == TokenTag::Assign);
                            }
                        }
                        TokenTag::LeftParen => paren_depth += 1,
                        TokenTag::RightParen => paren_depth = paren_depth.saturating_sub(1),
                        TokenTag::LeftBrace => brace_depth += 1,
                        TokenTag::RightBrace => brace_depth = brace_depth.saturating_sub(1),
                        TokenTag::Newline | TokenTag::Semicolon if square_depth == 0 => {
                            return false;
                        }
                        _ => {}
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn is_expression_head(&self) -> bool {
        let Some(first) = self.peek_significant_index(0) else {
            return false;
        };
        match tag(&self.tokens[first].kind) {
            TokenTag::Identifier => {
                let Some(next) = self.peek_significant_index(1) else {
                    return false;
                };
                let next_tag = tag(&self.tokens[next].kind);
                next_tag == TokenTag::Arrow
                    || (self.tokens[first].span.end == self.tokens[next].span.start
                        && matches!(
                            next_tag,
                            TokenTag::LeftParen | TokenTag::Dot | TokenTag::LeftBracket
                        ))
            }
            TokenTag::Number
            | TokenTag::String
            | TokenTag::True
            | TokenTag::False
            | TokenTag::Null
            | TokenTag::LeftParen
            | TokenTag::LeftBracket
            | TokenTag::LeftBrace
            | TokenTag::Bang
            | TokenTag::Typeof => true,
            _ => false,
        }
    }

    fn parse_command_chain(&mut self) -> Statement {
        let stops = [
            TokenTag::AndAnd,
            TokenTag::OrOr,
            TokenTag::Newline,
            TokenTag::Semicolon,
            TokenTag::RightBrace,
        ];
        let head = self.parse_pipeline(&stops);
        let mut tail = Vec::new();
        loop {
            self.skip_trivia_mode(LexMode::Command);
            let op = if self.at(TokenTag::AndAnd) {
                ChainOp::And
            } else if self.at(TokenTag::OrOr) {
                ChainOp::Or
            } else {
                break;
            };
            self.bump_mode(LexMode::Command);
            self.skip_trivia_mode(LexMode::Command);
            while self.at(TokenTag::Newline) {
                self.bump_mode(LexMode::Command);
                self.skip_trivia_mode(LexMode::Command);
            }
            if self.at_end() {
                self.diagnostics.push(Diagnostic::eof(
                    "P132",
                    "command chain cannot end with a logical operator",
                    self.source.len(),
                    &["command"],
                ));
                break;
            }
            tail.push((op, self.parse_pipeline(&stops)));
        }
        if tail.is_empty() {
            Statement::Command(head)
        } else {
            let span = Span::new(head.span.start, tail.last().unwrap().1.span.end);
            Statement::CommandChain { head, tail, span }
        }
    }

    fn parse_status(&mut self) -> Statement {
        let start = self.bump_span(LexMode::Command).start;
        let pipeline =
            self.parse_pipeline(&[TokenTag::Newline, TokenTag::Semicolon, TokenTag::RightBrace]);
        Statement::Status {
            span: Span::new(start, pipeline.span.end),
            pipeline,
        }
    }

    fn parse_let(&mut self) -> Statement {
        let start = self.bump_span(LexMode::Expression).start;
        let pattern = self.parse_pattern();
        self.skip_trivia_mode(LexMode::Expression);
        if self.at(TokenTag::Assign) {
            self.bump_mode(LexMode::Expression);
        } else {
            let at = self.current_start();
            self.diagnostics
                .push(self.expected("P111", "missing `=` in let binding", at, &["="]));
        }
        let value = self.parse_expr(0);
        let end = value.span().end.max(start);
        Statement::Let {
            pattern,
            value,
            span: Span::new(start, end),
        }
    }

    fn parse_pattern(&mut self) -> BindingPattern {
        self.skip_trivia_mode(LexMode::Expression);
        if self.at(TokenTag::Identifier) {
            let span = self.tokens[self.pos].span;
            return BindingPattern::Name {
                name: self.bump_text(LexMode::Expression),
                span,
            };
        }
        if self.at(TokenTag::LeftBracket) {
            let start = self.bump_span(LexMode::Expression).start;
            let mut items = Vec::new();
            let mut rest = None;
            loop {
                self.skip_trivia_mode(LexMode::Expression);
                if self.at_end() || self.at(TokenTag::RightBracket) {
                    break;
                }
                if self.at(TokenTag::Ellipsis) {
                    self.bump_mode(LexMode::Expression);
                    rest = Some(Box::new(self.parse_pattern()));
                    self.skip_trivia_mode(LexMode::Expression);
                    break;
                }
                items.push(self.parse_pattern());
                self.skip_trivia_mode(LexMode::Expression);
                if self.at(TokenTag::Comma) {
                    self.bump_mode(LexMode::Expression);
                } else {
                    break;
                }
            }
            let end = self.expect_closer(TokenTag::RightBracket, "]", LexMode::Expression);
            return BindingPattern::Array {
                items,
                rest,
                span: Span::new(start, end),
            };
        }
        if self.at(TokenTag::LeftBrace) {
            let start = self.bump_span(LexMode::Expression).start;
            let mut entries = Vec::new();
            let mut rest = None;
            loop {
                self.skip_trivia_mode(LexMode::Expression);
                if self.at_end() || self.at(TokenTag::RightBrace) {
                    break;
                }
                if self.at(TokenTag::Ellipsis) {
                    self.bump_mode(LexMode::Expression);
                    rest = Some(Box::new(self.parse_pattern()));
                    self.skip_trivia_mode(LexMode::Expression);
                    break;
                }
                let key_span = self
                    .tokens
                    .get(self.pos)
                    .map_or(Span::empty(self.current_start()), |x| x.span);
                let key = if self.at_property_name() {
                    self.parse_property_name()
                } else {
                    self.diagnostics.push(self.expected(
                        "P112",
                        "missing object binding key",
                        self.current_start(),
                        &["identifier", "string"],
                    ));
                    String::new()
                };
                self.skip_trivia_mode(LexMode::Expression);
                let pattern = if self.at(TokenTag::Colon) {
                    self.bump_mode(LexMode::Expression);
                    self.parse_pattern()
                } else {
                    BindingPattern::Name {
                        name: key.clone(),
                        span: key_span,
                    }
                };
                entries.push((key, pattern));
                self.skip_trivia_mode(LexMode::Expression);
                if self.at(TokenTag::Comma) {
                    self.bump_mode(LexMode::Expression);
                } else {
                    break;
                }
            }
            let end = self.expect_closer(TokenTag::RightBrace, "}", LexMode::Expression);
            return BindingPattern::Object {
                entries,
                rest,
                span: Span::new(start, end),
            };
        }
        let at = self.current_start();
        self.diagnostics.push(self.expected(
            "P110",
            "missing binding pattern",
            at,
            &["identifier", "[", "{"],
        ));
        BindingPattern::Missing {
            span: Span::empty(at),
        }
    }

    fn parse_assignment(&mut self) -> Statement {
        let start = self.current_start();
        let name = self.bump_text(LexMode::Expression);
        self.skip_trivia_mode(LexMode::Expression);
        let op = match self.peek_tag() {
            Some(TokenTag::PlusAssign) => {
                self.bump_mode(LexMode::Expression);
                AssignOp::Add
            }
            Some(TokenTag::MinusAssign) => {
                self.bump_mode(LexMode::Expression);
                AssignOp::Subtract
            }
            _ => {
                self.bump_mode(LexMode::Expression);
                AssignOp::Assign
            }
        };
        let value = self.parse_expr(0);
        Statement::Assignment {
            name,
            op,
            span: Span::new(start, value.span().end),
            value,
        }
    }

    fn parse_environment_assignment(&mut self) -> Statement {
        let start = self.current_start();
        self.bump_mode(LexMode::Expression);
        let target = if self.at(TokenTag::Dot) {
            self.bump_mode(LexMode::Expression);
            self.skip_trivia_mode(LexMode::Expression);
            let member_span = self.tokens[self.pos].span;
            let name = self.bump_text(LexMode::Expression);
            EnvironmentTarget::Member {
                name,
                span: Span::new(start, member_span.end),
            }
        } else {
            self.bump_mode(LexMode::Expression);
            let key = self.parse_expr(0);
            self.skip_trivia_mode(LexMode::Expression);
            let end = self.expect_closer(TokenTag::RightBracket, "]", LexMode::Expression);
            EnvironmentTarget::Index {
                key,
                span: Span::new(start, end),
            }
        };
        self.skip_trivia_mode(LexMode::Expression);
        self.bump_mode(LexMode::Expression);
        let value = self.parse_expr(0);
        Statement::EnvironmentAssignment {
            target,
            span: Span::new(start, value.span().end),
            value,
        }
    }

    fn parse_function(&mut self) -> Statement {
        let start = self.bump_span(LexMode::Expression).start;
        self.skip_trivia_mode(LexMode::Expression);
        let name = if self.at(TokenTag::Identifier) {
            self.bump_text(LexMode::Expression)
        } else {
            let at = self.current_start();
            self.diagnostics.push(self.expected(
                "P113",
                "missing function name",
                at,
                &["identifier"],
            ));
            String::new()
        };
        let params = self.parse_parameters();
        let (body, end) = self.parse_required_block("function");
        Statement::Function {
            name,
            params,
            body,
            span: Span::new(start, end),
        }
    }

    fn parse_parameters(&mut self) -> Vec<BindingPattern> {
        self.skip_trivia_mode(LexMode::Expression);
        if !self.at(TokenTag::LeftParen) {
            self.diagnostics.push(self.expected(
                "P114",
                "missing function parameter list",
                self.current_start(),
                &["("],
            ));
            return Vec::new();
        }
        self.bump_mode(LexMode::Expression);
        let mut params = Vec::new();
        loop {
            self.skip_trivia_mode(LexMode::Expression);
            if self.at_end() || self.at(TokenTag::RightParen) {
                break;
            }
            params.push(self.parse_pattern());
            self.skip_trivia_mode(LexMode::Expression);
            if self.at(TokenTag::Comma) {
                self.bump_mode(LexMode::Expression);
            } else {
                break;
            }
        }
        self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
        params
    }

    fn parse_if_expression(&mut self) -> Expr {
        let start = self.bump_span(LexMode::Expression).start;
        let condition = self.parse_condition();
        let (then_block, mut end) = self.parse_required_block("if");
        self.skip_separators();
        let else_block = if self.at(TokenTag::Else) {
            self.bump_mode(LexMode::Expression);
            self.skip_trivia_mode(LexMode::Expression);
            if self.at(TokenTag::If) {
                let nested = self.parse_if_expression();
                end = nested.span().end;
                Some(Program {
                    span: nested.span(),
                    statements: vec![Statement::Expr(nested)],
                })
            } else {
                let (block, block_end) = self.parse_required_block("else");
                end = block_end;
                Some(block)
            }
        } else {
            None
        };
        Expr::If {
            condition,
            then_block,
            else_block,
            span: Span::new(start, end),
        }
    }

    fn parse_while(&mut self) -> Statement {
        let start = self.bump_span(LexMode::Expression).start;
        let condition = self.parse_condition();
        let (body, end) = self.parse_required_block("while");
        Statement::While {
            condition,
            body,
            span: Span::new(start, end),
        }
    }

    fn parse_condition(&mut self) -> IfCondition {
        self.skip_trivia_mode(LexMode::Expression);
        if self.at(TokenTag::LeftParen) {
            self.bump_mode(LexMode::Expression);
            let expr = self.parse_expr(0);
            self.skip_trivia_mode(LexMode::Expression);
            self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
            IfCondition::Expr(Box::new(expr))
        } else {
            IfCondition::Command(self.parse_pipeline(&[TokenTag::LeftBrace]))
        }
    }

    fn parse_loop(&mut self) -> Statement {
        let start = self.bump_span(LexMode::Expression).start;
        let (body, end) = self.parse_required_block("loop");
        Statement::Loop {
            body,
            span: Span::new(start, end),
        }
    }

    fn parse_try_expression(&mut self) -> Expr {
        let start = self.bump_span(LexMode::Expression).start;
        let (body, _) = self.parse_required_block("try");
        self.skip_separators();
        if !self.at(TokenTag::Catch) {
            self.diagnostics.push(self.expected(
                "P123",
                "missing `catch` after try block",
                self.current_start(),
                &["catch"],
            ));
        } else {
            self.bump_mode(LexMode::Expression);
        }
        self.skip_trivia_mode(LexMode::Expression);
        let parenthesized = self.at(TokenTag::LeftParen);
        if parenthesized {
            self.bump_mode(LexMode::Expression);
        }
        let catch_pattern = self.parse_pattern();
        if parenthesized {
            self.skip_trivia_mode(LexMode::Expression);
            self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
        }
        let (catch_body, end) = self.parse_required_block("catch");
        Expr::Try {
            body,
            catch_pattern,
            catch_body,
            span: Span::new(start, end),
        }
    }

    fn parse_throw(&mut self) -> Statement {
        let start = self.bump_span(LexMode::Expression).start;
        let value = self.parse_expr(0);
        Statement::Throw {
            span: Span::new(start, value.span().end),
            value,
        }
    }

    fn parse_return(&mut self) -> Statement {
        let start_span = self.bump_span(LexMode::Expression);
        while self.is_trivia() {
            self.bump_mode(LexMode::Expression);
        }
        let value = if self.at_end()
            || matches!(
                self.peek_tag(),
                Some(TokenTag::Newline | TokenTag::Semicolon | TokenTag::RightBrace)
            ) {
            None
        } else {
            Some(self.parse_expr(0))
        };
        let end = value
            .as_ref()
            .map_or(start_span.end, |expr| expr.span().end);
        Statement::Return {
            value,
            span: Span::new(start_span.start, end),
        }
    }

    fn parse_required_block(&mut self, owner: &str) -> (Program, usize) {
        self.skip_separators();
        if !self.at(TokenTag::LeftBrace) {
            let at = self.current_start();
            self.diagnostics.push(self.expected(
                "P120",
                format!("missing `{{` after {owner}"),
                at,
                &["{"],
            ));
            return (
                Program {
                    statements: vec![Statement::Missing {
                        span: Span::empty(at),
                    }],
                    span: Span::empty(at),
                },
                at,
            );
        }
        self.bump_mode(LexMode::Expression);
        let body = self.parse_program(true);
        self.skip_trivia_mode(LexMode::Command);
        let end = if self.at(TokenTag::RightBrace) {
            self.bump_span(LexMode::Expression).end
        } else {
            let at = self.source.len();
            self.diagnostics.push(Diagnostic::eof(
                "P121",
                format!("unclosed {owner} block"),
                at,
                &["}"],
            ));
            at
        };
        (body, end)
    }

    fn parse_pipeline(&mut self, stops: &[TokenTag]) -> Pipeline {
        let start = self.current_start();
        let mut stages = Vec::new();
        let mut diagnosed_empty_stage = false;
        loop {
            self.skip_trivia_mode(LexMode::Command);
            if self.at_end() || self.at_any(stops) || self.at(TokenTag::RightParen) {
                break;
            }
            if self.at(TokenTag::Pipe) {
                let span = self.bump_span(LexMode::Command);
                let mut diagnostic =
                    Diagnostic::error("P131", "expected a command before `|`", span);
                diagnostic.expected.push("command".into());
                self.diagnostics.push(diagnostic);
                diagnosed_empty_stage = true;
                continue;
            }
            stages.push(self.parse_command(stops));
            self.skip_trivia_mode(LexMode::Command);
            if !self.at(TokenTag::Pipe) {
                break;
            }
            self.bump_mode(LexMode::Command);
            self.skip_trivia_mode(LexMode::Command);
            while self.at(TokenTag::Newline) {
                self.bump_mode(LexMode::Command);
                self.skip_trivia_mode(LexMode::Command);
            }
            if self.at_end() {
                self.diagnostics.push(Diagnostic::eof(
                    "P130",
                    "pipeline cannot end with `|`",
                    self.source.len(),
                    &["command"],
                ));
                break;
            }
            if self.at_any(stops) || self.at(TokenTag::RightParen) || self.at(TokenTag::Semicolon) {
                let at = self.current_start();
                self.diagnostics.push(self.expected(
                    "P131",
                    "expected a command after `|`",
                    at,
                    &["command"],
                ));
                diagnosed_empty_stage = true;
                break;
            }
        }
        if stages.is_empty() && !diagnosed_empty_stage {
            let at = self.current_start();
            self.diagnostics
                .push(self.expected("P131", "expected a command", at, &["command"]));
        }
        let end = stages.last().map_or(start, |s| s.span.end);
        Pipeline {
            stages,
            span: Span::new(start, end),
        }
    }

    fn parse_command(&mut self, stops: &[TokenTag]) -> ExternalCommand {
        let start = self.current_start();
        let mut words: Vec<CommandWord> = Vec::new();
        let mut redirections = Vec::new();
        loop {
            let had_space = self.skip_trivia_mode(LexMode::Command);
            if words.is_empty() && self.excluded_command_here().is_some() {
                let feature = self.excluded_command_here().expect("excluded command");
                let span = self.bump_span(LexMode::Command);
                self.diagnostics.push(Diagnostic::error(
                    "P143",
                    format!("{feature} is deliberately unavailable"),
                    span,
                ));
                while !self.at_end() && !self.at_any(stops) && !self.at(TokenTag::Newline) {
                    self.bump_mode(LexMode::Command);
                }
                break;
            }
            if self.at(TokenTag::RightParen)
                && words.len() == 1
                && self.source[words[0].span.range()].starts_with("chunks(")
                && !self.source[words[0].span.range()].ends_with(')')
            {
                let close = self.bump_span(LexMode::Command);
                words[0].parts.push(WordPart::Literal {
                    value: ")".into(),
                    glob_unquoted: true,
                    span: close,
                });
                words[0].span.end = close.end;
                continue;
            }
            if self.at_end()
                || self.at_any(stops)
                || self.at(TokenTag::Pipe)
                || self.at(TokenTag::Semicolon)
                || self.at(TokenTag::Newline)
                || self.at(TokenTag::LeftBrace)
                || self.at(TokenTag::RightBrace)
                || self.at(TokenTag::RightParen)
            {
                break;
            }
            if self.at_redirection() {
                redirections.push(self.parse_redirection());
                continue;
            }
            if let Some(feature) = self.unsupported_here() {
                let span = self.bump_span(LexMode::Command);
                self.diagnostics.push(Diagnostic::error(
                    "P140",
                    format!("{feature} is reserved but not implemented"),
                    span,
                ));
                while !self.at_end() && !self.at_any(stops) && !self.at(TokenTag::Newline) {
                    self.bump_mode(LexMode::Command);
                }
                break;
            }
            if (had_space || words.is_empty()) && self.at(TokenTag::LeftParen) {
                let open = self.bump_span(LexMode::Expression);
                let expr = self.parse_expr(0);
                self.skip_trivia_mode(LexMode::Expression);
                let end = self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
                words.push(CommandWord {
                    span: Span::new(open.start, end),
                    parts: vec![WordPart::Evaluated {
                        expr: Box::new(expr),
                        span: Span::new(open.start, end),
                    }],
                });
                continue;
            }
            let before = self.pos;
            words.push(self.parse_word());
            if self.pos == before {
                let span = self.bump_span(LexMode::Command);
                self.diagnostics.push(Diagnostic::error(
                    "P141",
                    "command parser made no progress",
                    span,
                ));
            }
        }
        let end = words
            .last()
            .map(|word| word.span.end)
            .into_iter()
            .chain(redirections.last().map(|redirection| redirection.span.end))
            .max()
            .unwrap_or(start);
        ExternalCommand {
            words,
            redirections,
            span: Span::new(start, end),
        }
    }

    fn at_redirection(&self) -> bool {
        matches!(
            self.peek_tag(),
            Some(
                TokenTag::Less
                    | TokenTag::Greater
                    | TokenTag::RedirectAppend
                    | TokenTag::RedirectError
                    | TokenTag::RedirectErrorAppend
                    | TokenTag::RedirectErrorToOutput
                    | TokenTag::RedirectOutputAndError
            )
        )
    }

    fn parse_redirection(&mut self) -> Redirection {
        let operator = self.peek_tag().expect("redirection token");
        let operator_span = self.bump_span(LexMode::Command);
        let kind = match operator {
            TokenTag::Less => RedirectionKind::Input,
            TokenTag::Greater => RedirectionKind::Output,
            TokenTag::RedirectAppend => RedirectionKind::Append,
            TokenTag::RedirectError => RedirectionKind::Error,
            TokenTag::RedirectErrorAppend => RedirectionKind::ErrorAppend,
            TokenTag::RedirectErrorToOutput => RedirectionKind::ErrorToOutput,
            TokenTag::RedirectOutputAndError => RedirectionKind::OutputAndError,
            _ => unreachable!("redirection token was checked"),
        };
        if kind == RedirectionKind::ErrorToOutput {
            return Redirection {
                kind,
                target: None,
                span: operator_span,
            };
        }
        self.skip_trivia_mode(LexMode::Command);
        if self.at_end()
            || matches!(
                self.peek_tag(),
                Some(
                    TokenTag::Pipe
                        | TokenTag::Semicolon
                        | TokenTag::Newline
                        | TokenTag::RightBrace
                        | TokenTag::RightParen
                )
            )
        {
            self.diagnostics.push(self.expected(
                "P142",
                "redirection requires a target path",
                self.current_start(),
                &["path"],
            ));
            return Redirection {
                kind,
                target: None,
                span: operator_span,
            };
        }
        let target = if self.at(TokenTag::LeftParen) {
            let open = self.bump_span(LexMode::Expression);
            let expr = self.parse_expr(0);
            self.skip_trivia_mode(LexMode::Expression);
            let end = self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
            CommandWord {
                span: Span::new(open.start, end),
                parts: vec![WordPart::Evaluated {
                    expr: Box::new(expr),
                    span: Span::new(open.start, end),
                }],
            }
        } else {
            self.parse_word()
        };
        Redirection {
            kind,
            span: Span::new(operator_span.start, target.span.end),
            target: Some(target),
        }
    }

    fn parse_word(&mut self) -> CommandWord {
        let start = self.current_start();
        let mut end = start;
        let mut parts = Vec::new();
        while !self.at_end()
            && !self.is_trivia()
            && !matches!(
                self.peek_tag(),
                Some(
                    TokenTag::Pipe
                        | TokenTag::Semicolon
                        | TokenTag::Less
                        | TokenTag::Greater
                        | TokenTag::RedirectAppend
                        | TokenTag::RedirectError
                        | TokenTag::RedirectErrorAppend
                        | TokenTag::RedirectErrorToOutput
                        | TokenTag::RedirectOutputAndError
                        | TokenTag::Newline
                        | TokenTag::RightParen
                        | TokenTag::LeftBrace
                        | TokenTag::RightBrace
                )
            )
        {
            if self.unsupported_here().is_some() {
                break;
            }
            let token = self.tokens[self.pos].clone();
            match token.kind {
                TokenKind::SingleQuoted => {
                    self.bump_mode(LexMode::SingleQuote);
                    let raw = &self.source[token.span.range()];
                    let value = raw
                        .strip_prefix('\'')
                        .and_then(|x| x.strip_suffix('\''))
                        .unwrap_or_else(|| raw.strip_prefix('\'').unwrap_or(raw))
                        .to_owned();
                    parts.push(WordPart::SingleQuoted {
                        value,
                        span: token.span,
                    });
                }
                TokenKind::DoubleQuoted => {
                    self.bump_mode(LexMode::DoubleQuote);
                    parts.push(self.parse_double_quoted(token.span));
                }
                TokenKind::DollarVariable => {
                    self.bump_mode(LexMode::Command);
                    parts.push(WordPart::Variable {
                        name: self.source[token.span.range()]
                            .trim_start_matches('$')
                            .to_owned(),
                        span: token.span,
                    });
                }
                TokenKind::CaptureStart => {
                    self.bump_mode(LexMode::Command);
                    let pipeline = self.parse_pipeline(&[TokenTag::RightParen]);
                    self.skip_separators();
                    let end_span = self.expect_closer(TokenTag::RightParen, ")", LexMode::Command);
                    let span = Span::new(token.span.start, end_span);
                    parts.push(WordPart::Capture {
                        pipeline: Box::new(pipeline),
                        span,
                    });
                    end = end_span;
                    continue;
                }
                _ => {
                    self.bump_mode(LexMode::Command);
                    let raw = &self.source[token.span.range()];
                    parts.push(WordPart::Literal {
                        value: unescape_command(raw),
                        glob_unquoted: !raw.starts_with('\\'),
                        span: token.span,
                    });
                }
            }
            end = token.span.end;
        }
        CommandWord {
            parts,
            span: Span::new(start, end),
        }
    }

    fn parse_double_quoted(&mut self, span: Span) -> WordPart {
        let raw = &self.source[span.range()];
        let closed = raw.ends_with('"') && raw.len() > 1;
        let inner = raw
            .strip_prefix('"')
            .and_then(|x| x.strip_suffix('"'))
            .unwrap_or_else(|| raw.strip_prefix('"').unwrap_or(raw))
            .to_owned();
        let base = span.start + usize::from(raw.starts_with('"'));
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut at = 0;
        while at < inner.len() {
            let ch = inner[at..]
                .chars()
                .next()
                .expect("quote cursor is in bounds");
            if ch == '\\' {
                at += ch.len_utf8();
                if at < inner.len() {
                    let escaped = inner[at..].chars().next().unwrap();
                    literal.push(escaped);
                    at += escaped.len_utf8();
                }
                continue;
            }
            if inner[at..].starts_with("$(") {
                if let Some(close) = find_matching(&inner, at + 2, '(', ')') {
                    if !literal.is_empty() {
                        parts.push(QuotedPart::Literal(std::mem::take(&mut literal)));
                    }
                    let fragment = &inner[at + 2..close];
                    let offset = base + at + 2;
                    let mut nested = Parser::new(Arc::from(fragment));
                    let mut pipeline = nested.parse_pipeline(&[]);
                    nested.skip_separators();
                    if !nested.at_end() {
                        let span = nested.bump_span(LexMode::Command);
                        nested.diagnostics.push(Diagnostic::error(
                            "P182",
                            "unexpected token after interpolated pipeline",
                            span,
                        ));
                    }
                    offset_pipeline(&mut pipeline, offset);
                    self.diagnostics.extend(nested.diagnostics.into_iter().map(
                        |mut diagnostic| {
                            offset_diagnostic(&mut diagnostic, offset);
                            diagnostic.eof_caused = false;
                            diagnostic
                        },
                    ));
                    parts.push(QuotedPart::Capture(Box::new(pipeline)));
                    at = close + 1;
                    continue;
                }
                self.diagnostics.push(if closed {
                    Diagnostic::error(
                        "P180",
                        "unclosed command interpolation",
                        Span::new(base + at, span.end),
                    )
                } else {
                    Diagnostic::eof(
                        "P180",
                        "unclosed command interpolation",
                        self.source.len(),
                        &[")"],
                    )
                });
                literal.push_str(&inner[at..]);
                break;
            }
            if inner[at..].starts_with("${") {
                if let Some(close) = find_matching(&inner, at + 2, '{', '}') {
                    if !literal.is_empty() {
                        parts.push(QuotedPart::Literal(std::mem::take(&mut literal)));
                    }
                    let fragment = &inner[at + 2..close];
                    let offset = base + at + 2;
                    let mut nested = Parser::new(Arc::from(fragment));
                    let mut expr = nested.parse_expr(0);
                    nested.skip_separators();
                    if !nested.at_end() {
                        let span = nested.bump_span(LexMode::Expression);
                        nested.diagnostics.push(Diagnostic::error(
                            "P183",
                            "unexpected token after interpolated expression",
                            span,
                        ));
                    }
                    offset_expr(&mut expr, offset);
                    self.diagnostics.extend(nested.diagnostics.into_iter().map(
                        |mut diagnostic| {
                            offset_diagnostic(&mut diagnostic, offset);
                            diagnostic.eof_caused = false;
                            diagnostic
                        },
                    ));
                    parts.push(QuotedPart::Expression(expr));
                    at = close + 1;
                    continue;
                }
                self.diagnostics.push(if closed {
                    Diagnostic::error(
                        "P181",
                        "unclosed expression interpolation",
                        Span::new(base + at, span.end),
                    )
                } else {
                    Diagnostic::eof(
                        "P181",
                        "unclosed expression interpolation",
                        self.source.len(),
                        &["}"],
                    )
                });
                literal.push_str(&inner[at..]);
                break;
            }
            if ch == '$' {
                let name_start = at + 1;
                if name_start < inner.len() {
                    let next = inner[name_start..].chars().next().unwrap();
                    if is_ident_start(next) {
                        if !literal.is_empty() {
                            parts.push(QuotedPart::Literal(std::mem::take(&mut literal)));
                        }
                        let mut end = name_start + next.len_utf8();
                        while end < inner.len() {
                            let c = inner[end..].chars().next().unwrap();
                            if !is_ident_continue(c) {
                                break;
                            }
                            end += c.len_utf8();
                        }
                        parts.push(QuotedPart::Variable(inner[name_start..end].to_owned()));
                        at = end;
                        continue;
                    }
                }
            }
            literal.push(ch);
            at += ch.len_utf8();
        }
        if !literal.is_empty() {
            parts.push(QuotedPart::Literal(literal));
        }
        WordPart::DoubleQuoted { parts, span }
    }

    fn parse_expr(&mut self, min_bp: u8) -> Expr {
        self.skip_trivia_mode(LexMode::Expression);
        let mut lhs = self.parse_prefix();
        loop {
            let had_trivia = self.skip_trivia_mode(LexMode::Expression);
            if self.at(TokenTag::LeftParen) && !had_trivia {
                let start = lhs.span().start;
                self.bump_mode(LexMode::Expression);
                let mut args = Vec::new();
                self.skip_trivia_mode(LexMode::Expression);
                while !self.at_end() && !self.at(TokenTag::RightParen) {
                    if self.at(TokenTag::Ellipsis) {
                        let spread_start = self.bump_span(LexMode::Expression).start;
                        let value = self.parse_expr(0);
                        let span = Span::new(spread_start, value.span().end);
                        args.push(CallArg::Spread(value, span));
                    } else {
                        args.push(CallArg::Value(self.parse_expr(0)));
                    }
                    self.skip_trivia_mode(LexMode::Expression);
                    if self.at(TokenTag::Comma) {
                        self.bump_mode(LexMode::Expression);
                    } else {
                        break;
                    }
                }
                let end = self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
                lhs = Expr::Call {
                    callee: Box::new(lhs),
                    args,
                    span: Span::new(start, end),
                };
                continue;
            }
            if self.at(TokenTag::Dot) && !had_trivia {
                let start = lhs.span().start;
                self.bump_mode(LexMode::Expression);
                self.skip_trivia_mode(LexMode::Expression);
                if self.at(TokenTag::Identifier) || self.at(TokenTag::Status) {
                    let name_span = self.tokens[self.pos].span;
                    let name = self.bump_text(LexMode::Expression);
                    lhs = Expr::Member {
                        object: Box::new(lhs),
                        name,
                        span: Span::new(start, name_span.end),
                    };
                } else {
                    let at = self.current_start();
                    self.diagnostics.push(self.expected(
                        "P150",
                        "missing member name",
                        at,
                        &["identifier"],
                    ));
                }
                continue;
            }
            if self.at(TokenTag::LeftBracket) && !had_trivia {
                let start = lhs.span().start;
                self.bump_mode(LexMode::Expression);
                let index = self.parse_expr(0);
                self.skip_trivia_mode(LexMode::Expression);
                let end = self.expect_closer(TokenTag::RightBracket, "]", LexMode::Expression);
                lhs = Expr::Index {
                    object: Box::new(lhs),
                    index: Box::new(index),
                    span: Span::new(start, end),
                };
                continue;
            }
            if self.at(TokenTag::Arrow) && min_bp == 0 {
                let start = lhs.span().start;
                self.bump_mode(LexMode::Expression);
                let params = match lhs {
                    Expr::Identifier(name, span) => vec![BindingPattern::Name { name, span }],
                    _ => {
                        self.diagnostics.push(Diagnostic::error(
                            "P164",
                            "arrow parameters must be binding patterns",
                            lhs.span(),
                        ));
                        Vec::new()
                    }
                };
                let (body, end) = self.parse_arrow_body();
                lhs = Expr::Arrow {
                    params,
                    body,
                    span: Span::new(start, end),
                };
                continue;
            }
            if matches!(
                self.tokens.get(self.pos).map(|t| &t.kind),
                Some(TokenKind::Unsupported("equality operator `==`"))
            ) {
                let span = self.bump_span(LexMode::Expression);
                let mut diagnostic =
                    Diagnostic::error("P162", "`==` is not supported; use `===`", span);
                diagnostic.expected.push("===".into());
                self.diagnostics.push(diagnostic);
                let rhs = self.parse_expr(6);
                lhs = Expr::Error(Span::new(lhs.span().start, rhs.span().end));
                continue;
            }
            let Some((left_bp, right_bp, op)) = self.infix_binding_power() else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            self.bump_mode(LexMode::Expression);
            let rhs = self.parse_expr(right_bp);
            let span = Span::new(lhs.span().start, rhs.span().end);
            lhs = Expr::Binary {
                left: Box::new(lhs),
                op,
                right: Box::new(rhs),
                span,
            };
        }
        self.skip_trivia_mode(LexMode::Expression);
        if min_bp == 0 && self.at(TokenTag::Question) {
            let start = lhs.span().start;
            self.bump_mode(LexMode::Expression);
            let then_expr = self.parse_expr(0);
            self.skip_trivia_mode(LexMode::Expression);
            if self.at(TokenTag::Colon) {
                self.bump_mode(LexMode::Expression);
            } else {
                self.diagnostics.push(self.expected(
                    "P165",
                    "missing `:` in ternary expression",
                    self.current_start(),
                    &[":"],
                ));
            }
            let else_expr = self.parse_expr(0);
            let end = else_expr.span().end;
            lhs = Expr::Ternary {
                condition: Box::new(lhs),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span: Span::new(start, end),
            };
        }
        lhs
    }

    fn parse_prefix(&mut self) -> Expr {
        self.skip_trivia_mode(LexMode::Expression);
        if self.at_end() {
            let at = self.source.len();
            self.diagnostics.push(Diagnostic::eof(
                "P160",
                "missing expression",
                at,
                &["expression"],
            ));
            return Expr::Missing(Span::empty(at));
        }
        let token = self.tokens[self.pos].clone();
        match token.kind {
            TokenKind::Null => {
                self.bump_mode(LexMode::Expression);
                Expr::Null(token.span)
            }
            TokenKind::True | TokenKind::False => {
                self.bump_mode(LexMode::Expression);
                Expr::Bool(matches!(token.kind, TokenKind::True), token.span)
            }
            TokenKind::Number => {
                self.bump_mode(LexMode::Expression);
                let text = &self.source[token.span.range()];
                if let Ok(value) = text.parse::<i64>() {
                    Expr::Int(value, token.span)
                } else if let Ok(value) = text.parse::<f64>() {
                    Expr::Float(value, token.span)
                } else {
                    self.diagnostics
                        .push(Diagnostic::error("P161", "invalid number", token.span));
                    Expr::Error(token.span)
                }
            }
            TokenKind::Identifier | TokenKind::Word => {
                let name = self.bump_text(LexMode::Expression);
                Expr::Identifier(name, token.span)
            }
            TokenKind::String | TokenKind::SingleQuoted | TokenKind::DoubleQuoted => {
                self.bump_mode(if matches!(token.kind, TokenKind::SingleQuoted) {
                    LexMode::SingleQuote
                } else {
                    LexMode::DoubleQuote
                });
                let raw = &self.source[token.span.range()];
                Expr::String(decode_quoted_literal(raw, &token.kind), token.span)
            }
            TokenKind::LeftParen if self.is_parenthesized_arrow() => {
                let start = self.bump_span(LexMode::Expression).start;
                let mut params = Vec::new();
                loop {
                    self.skip_trivia_mode(LexMode::Expression);
                    if self.at_end() || self.at(TokenTag::RightParen) {
                        break;
                    }
                    params.push(self.parse_pattern());
                    self.skip_trivia_mode(LexMode::Expression);
                    if self.at(TokenTag::Comma) {
                        self.bump_mode(LexMode::Expression);
                    } else {
                        break;
                    }
                }
                self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
                self.skip_trivia_mode(LexMode::Expression);
                if self.at(TokenTag::Arrow) {
                    self.bump_mode(LexMode::Expression);
                }
                let (body, end) = self.parse_arrow_body();
                Expr::Arrow {
                    params,
                    body,
                    span: Span::new(start, end),
                }
            }
            TokenKind::LeftParen => {
                self.bump_mode(LexMode::Expression);
                let expr = self.parse_expr(0);
                self.skip_trivia_mode(LexMode::Expression);
                self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
                expr
            }
            TokenKind::LeftBracket => self.parse_array(),
            TokenKind::LeftBrace => self.parse_object(),
            TokenKind::Minus | TokenKind::Bang | TokenKind::Typeof => {
                self.bump_mode(LexMode::Expression);
                let expr = self.parse_expr(13);
                let op = match token.kind {
                    TokenKind::Minus => UnaryOp::Negate,
                    TokenKind::Bang => UnaryOp::Not,
                    TokenKind::Typeof => UnaryOp::Typeof,
                    _ => unreachable!(),
                };
                let end = expr.span().end;
                Expr::Unary {
                    op,
                    expr: Box::new(expr),
                    span: Span::new(token.span.start, end),
                }
            }
            TokenKind::CaptureStart => {
                self.bump_mode(LexMode::Expression);
                let pipeline = self.parse_pipeline(&[TokenTag::RightParen]);
                self.skip_separators();
                let end = self.expect_closer(TokenTag::RightParen, ")", LexMode::Expression);
                Expr::Capture {
                    pipeline: Box::new(pipeline),
                    span: Span::new(token.span.start, end),
                }
            }
            TokenKind::If => self.parse_if_expression(),
            TokenKind::Try => self.parse_try_expression(),
            TokenKind::Unsupported("equality operator `==`") => {
                self.bump_mode(LexMode::Expression);
                let mut d =
                    Diagnostic::error("P162", "`==` is not supported; use `===`", token.span);
                d.expected.push("===".into());
                self.diagnostics.push(d);
                Expr::Error(token.span)
            }
            _ => {
                self.bump_mode(LexMode::Expression);
                self.diagnostics.push(Diagnostic::error(
                    "P163",
                    "expected an expression",
                    token.span,
                ));
                Expr::Error(token.span)
            }
        }
    }

    fn parse_array(&mut self) -> Expr {
        let start = self.bump_span(LexMode::Expression).start;
        let mut values = Vec::new();
        loop {
            self.skip_trivia_mode(LexMode::Expression);
            if self.at_end() || self.at(TokenTag::RightBracket) {
                break;
            }
            if self.at(TokenTag::Ellipsis) {
                let spread_start = self.bump_span(LexMode::Expression).start;
                let value = self.parse_expr(0);
                let span = Span::new(spread_start, value.span().end);
                values.push(ArrayElement::Spread(value, span));
            } else {
                values.push(ArrayElement::Value(self.parse_expr(0)));
            }
            self.skip_trivia_mode(LexMode::Expression);
            if self.at(TokenTag::Comma) {
                self.bump_mode(LexMode::Expression);
            } else {
                break;
            }
        }
        let end = self.expect_closer(TokenTag::RightBracket, "]", LexMode::Expression);
        Expr::Array(values, Span::new(start, end))
    }

    fn parse_object(&mut self) -> Expr {
        let start = self.bump_span(LexMode::Expression).start;
        let mut entries = Vec::new();
        loop {
            self.skip_trivia_mode(LexMode::Expression);
            if self.at_end() || self.at(TokenTag::RightBrace) {
                break;
            }
            if self.at(TokenTag::Ellipsis) {
                let spread_start = self.bump_span(LexMode::Expression).start;
                let value = self.parse_expr(0);
                let span = Span::new(spread_start, value.span().end);
                entries.push(ObjectEntry::Spread { value, span });
            } else {
                let property_start = self.current_start();
                let key = if self.at_property_name() {
                    self.parse_property_name()
                } else {
                    self.diagnostics.push(self.expected(
                        "P166",
                        "missing object property name",
                        self.current_start(),
                        &["identifier", "string"],
                    ));
                    String::new()
                };
                self.skip_trivia_mode(LexMode::Expression);
                let value = if self.at(TokenTag::Colon) {
                    self.bump_mode(LexMode::Expression);
                    self.parse_expr(0)
                } else {
                    Expr::Identifier(key.clone(), Span::new(property_start, self.current_start()))
                };
                let span = Span::new(property_start, value.span().end);
                entries.push(ObjectEntry::Property { key, value, span });
            }
            self.skip_trivia_mode(LexMode::Expression);
            if self.at(TokenTag::Comma) {
                self.bump_mode(LexMode::Expression);
            } else {
                break;
            }
        }
        let end = self.expect_closer(TokenTag::RightBrace, "}", LexMode::Expression);
        Expr::Object(entries, Span::new(start, end))
    }

    fn parse_property_name(&mut self) -> String {
        let token = self.tokens[self.pos].clone();
        self.bump_mode(LexMode::Expression);
        let raw = &self.source[token.span.range()];
        if matches!(
            token.kind,
            TokenKind::String | TokenKind::SingleQuoted | TokenKind::DoubleQuoted
        ) {
            decode_quoted_literal(raw, &token.kind)
        } else {
            raw.to_owned()
        }
    }

    fn parse_arrow_body(&mut self) -> (FunctionBody, usize) {
        self.skip_trivia_mode(LexMode::Expression);
        if self.at(TokenTag::LeftBrace) {
            let (body, end) = self.parse_required_block("arrow function");
            (FunctionBody::Block(body), end)
        } else {
            let body = self.parse_expr(0);
            let end = body.span().end;
            (FunctionBody::Expression(Box::new(body)), end)
        }
    }

    fn is_parenthesized_arrow(&self) -> bool {
        if self.peek_tag() != Some(TokenTag::LeftParen) {
            return false;
        }
        let mut depth = 0_usize;
        for index in self.pos..self.tokens.len() {
            match tag(&self.tokens[index].kind) {
                TokenTag::LeftParen => depth += 1,
                TokenTag::RightParen => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return self.tokens[index + 1..]
                            .iter()
                            .find(|token| {
                                !matches!(
                                    token.kind,
                                    TokenKind::Whitespace | TokenKind::Comment | TokenKind::Newline
                                )
                            })
                            .is_some_and(|token| tag(&token.kind) == TokenTag::Arrow);
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn infix_binding_power(&self) -> Option<(u8, u8, BinaryOp)> {
        Some(match self.peek_tag()? {
            TokenTag::OrOr => (1, 2, BinaryOp::Or),
            TokenTag::AndAnd => (3, 4, BinaryOp::And),
            TokenTag::StrictEq => (5, 6, BinaryOp::Equal),
            TokenTag::StrictNotEq => (5, 6, BinaryOp::NotEqual),
            TokenTag::Less => (7, 8, BinaryOp::Less),
            TokenTag::LessEq => (7, 8, BinaryOp::LessEq),
            TokenTag::Greater => (7, 8, BinaryOp::Greater),
            TokenTag::GreaterEq => (7, 8, BinaryOp::GreaterEq),
            TokenTag::Plus => (9, 10, BinaryOp::Add),
            TokenTag::Minus => (9, 10, BinaryOp::Subtract),
            TokenTag::Star => (11, 12, BinaryOp::Multiply),
            TokenTag::Slash => (11, 12, BinaryOp::Divide),
            TokenTag::DoubleSlash => (11, 12, BinaryOp::IntegerDivide),
            TokenTag::Percent => (11, 12, BinaryOp::Remainder),
            _ => return None,
        })
    }

    fn expect_closer(&mut self, wanted: TokenTag, text: &str, mode: LexMode) -> usize {
        if self.at(wanted) {
            return self.bump_span(mode).end;
        }
        let at = self.current_start();
        if self.at_end() {
            self.diagnostics.push(Diagnostic::eof(
                "P170",
                format!("unclosed delimiter; missing `{text}`"),
                at,
                &[text],
            ));
        } else {
            self.diagnostics
                .push(self.expected("P171", format!("missing `{text}`"), at, &[text]));
        }
        at
    }

    fn expected(
        &self,
        code: &'static str,
        message: impl Into<String>,
        at: usize,
        expected: &[&str],
    ) -> Diagnostic {
        if at == self.source.len() {
            Diagnostic::eof(code, message, at, expected)
        } else {
            let mut d = Diagnostic::error(code, message, Span::empty(at));
            d.expected = expected.iter().map(ToString::to_string).collect();
            d
        }
    }

    fn excluded_command_here(&self) -> Option<&'static str> {
        let token = self.tokens.get(self.pos)?;
        let name = &self.source[token.span.range()];
        match name {
            "jobs" | "fg" | "bg" => Some("job control"),
            "source" | "import" | "export" => Some("modules and source loading"),
            _ => None,
        }
    }

    fn unsupported_here(&self) -> Option<&'static str> {
        match self.tokens.get(self.pos).map(|t| &t.kind) {
            Some(TokenKind::Unsupported(name)) => Some(name),
            _ => None,
        }
    }

    fn at_statement_boundary(&self, in_block: bool) -> bool {
        self.at_end()
            || self.at(TokenTag::Newline)
            || self.at(TokenTag::Semicolon)
            || (in_block && self.at(TokenTag::RightBrace))
    }

    fn skip_separators(&mut self) {
        loop {
            self.skip_trivia_mode(LexMode::Command);
            if matches!(
                self.peek_tag(),
                Some(TokenTag::Newline | TokenTag::Semicolon)
            ) {
                self.bump_mode(LexMode::Command);
            } else {
                break;
            }
        }
    }
    fn skip_trivia_mode(&mut self, mode: LexMode) -> bool {
        let mut any = false;
        while self.is_trivia() || (mode == LexMode::Expression && self.at(TokenTag::Newline)) {
            any = true;
            self.bump_mode(mode);
        }
        any
    }
    fn is_trivia(&self) -> bool {
        matches!(
            self.tokens.get(self.pos).map(|t| &t.kind),
            Some(TokenKind::Whitespace | TokenKind::Comment)
        )
    }
    fn bump_mode(&mut self, mode: LexMode) {
        if let Some(t) = self.tokens.get_mut(self.pos) {
            t.mode = mode;
            self.pos += 1;
        }
    }
    fn bump_span(&mut self, mode: LexMode) -> Span {
        let span = self
            .tokens
            .get(self.pos)
            .map_or(Span::empty(self.source.len()), |t| t.span);
        self.bump_mode(mode);
        span
    }
    fn bump_text(&mut self, mode: LexMode) -> String {
        let span = self.bump_span(mode);
        self.source[span.range()].to_owned()
    }
    fn current_start(&self) -> usize {
        self.tokens
            .get(self.pos)
            .map_or(self.source.len(), |t| t.span.start)
    }
    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }
    fn at(&self, wanted: TokenTag) -> bool {
        self.peek_tag() == Some(wanted)
    }
    fn at_any(&self, wanted: &[TokenTag]) -> bool {
        self.peek_tag().is_some_and(|t| wanted.contains(&t))
    }
    fn peek_tag(&self) -> Option<TokenTag> {
        self.tokens.get(self.pos).map(|t| tag(&t.kind))
    }
    fn peek_significant_index(&self, n: usize) -> Option<usize> {
        self.tokens
            .iter()
            .enumerate()
            .skip(self.pos)
            .filter(|(_, t)| !matches!(t.kind, TokenKind::Whitespace | TokenKind::Comment))
            .nth(n)
            .map(|(i, _)| i)
    }
    fn nth_significant_tag(&self, n: usize) -> Option<TokenTag> {
        self.peek_significant_index(n)
            .map(|i| tag(&self.tokens[i].kind))
    }
    fn at_property_name(&self) -> bool {
        matches!(
            self.tokens.get(self.pos).map(|token| &token.kind),
            Some(
                TokenKind::Identifier
                    | TokenKind::String
                    | TokenKind::SingleQuoted
                    | TokenKind::DoubleQuoted
            )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenTag {
    Whitespace,
    Comment,
    Newline,
    Word,
    Identifier,
    Number,
    String,
    Let,
    Fn,
    If,
    Else,
    While,
    Loop,
    Try,
    Catch,
    Throw,
    Return,
    Break,
    Continue,
    Status,
    Typeof,
    True,
    False,
    Null,
    Assign,
    PlusAssign,
    MinusAssign,
    Pipe,
    Semicolon,
    Comma,
    Dot,
    Ellipsis,
    Colon,
    Question,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Plus,
    Minus,
    Star,
    Slash,
    DoubleSlash,
    Percent,
    Bang,
    AndAnd,
    OrOr,
    StrictEq,
    StrictNotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    RedirectAppend,
    RedirectError,
    RedirectErrorAppend,
    RedirectErrorToOutput,
    RedirectOutputAndError,
    Arrow,
    CaptureStart,
    Unsupported,
    Unknown,
}

fn tag(kind: &TokenKind) -> TokenTag {
    match kind {
        TokenKind::Whitespace => TokenTag::Whitespace,
        TokenKind::Comment => TokenTag::Comment,
        TokenKind::Newline => TokenTag::Newline,
        TokenKind::Word
        | TokenKind::SingleQuoted
        | TokenKind::DoubleQuoted
        | TokenKind::DollarVariable => TokenTag::Word,
        TokenKind::Identifier => TokenTag::Identifier,
        TokenKind::Number => TokenTag::Number,
        TokenKind::String => TokenTag::String,
        TokenKind::Let => TokenTag::Let,
        TokenKind::Fn => TokenTag::Fn,
        TokenKind::If => TokenTag::If,
        TokenKind::Else => TokenTag::Else,
        TokenKind::While => TokenTag::While,
        TokenKind::Loop => TokenTag::Loop,
        TokenKind::Try => TokenTag::Try,
        TokenKind::Catch => TokenTag::Catch,
        TokenKind::Throw => TokenTag::Throw,
        TokenKind::Return => TokenTag::Return,
        TokenKind::Break => TokenTag::Break,
        TokenKind::Continue => TokenTag::Continue,
        TokenKind::Status => TokenTag::Status,
        TokenKind::Typeof => TokenTag::Typeof,
        TokenKind::True => TokenTag::True,
        TokenKind::False => TokenTag::False,
        TokenKind::Null => TokenTag::Null,
        TokenKind::Assign => TokenTag::Assign,
        TokenKind::PlusAssign => TokenTag::PlusAssign,
        TokenKind::MinusAssign => TokenTag::MinusAssign,
        TokenKind::Pipe => TokenTag::Pipe,
        TokenKind::Semicolon => TokenTag::Semicolon,
        TokenKind::Comma => TokenTag::Comma,
        TokenKind::Dot => TokenTag::Dot,
        TokenKind::Ellipsis => TokenTag::Ellipsis,
        TokenKind::Colon => TokenTag::Colon,
        TokenKind::Question => TokenTag::Question,
        TokenKind::LeftParen => TokenTag::LeftParen,
        TokenKind::RightParen => TokenTag::RightParen,
        TokenKind::LeftBracket => TokenTag::LeftBracket,
        TokenKind::RightBracket => TokenTag::RightBracket,
        TokenKind::LeftBrace => TokenTag::LeftBrace,
        TokenKind::RightBrace => TokenTag::RightBrace,
        TokenKind::Plus => TokenTag::Plus,
        TokenKind::Minus => TokenTag::Minus,
        TokenKind::Star => TokenTag::Star,
        TokenKind::Slash => TokenTag::Slash,
        TokenKind::DoubleSlash => TokenTag::DoubleSlash,
        TokenKind::Percent => TokenTag::Percent,
        TokenKind::Bang => TokenTag::Bang,
        TokenKind::AndAnd => TokenTag::AndAnd,
        TokenKind::OrOr => TokenTag::OrOr,
        TokenKind::StrictEq => TokenTag::StrictEq,
        TokenKind::StrictNotEq => TokenTag::StrictNotEq,
        TokenKind::Less => TokenTag::Less,
        TokenKind::LessEq => TokenTag::LessEq,
        TokenKind::Greater => TokenTag::Greater,
        TokenKind::GreaterEq => TokenTag::GreaterEq,
        TokenKind::RedirectAppend => TokenTag::RedirectAppend,
        TokenKind::RedirectError => TokenTag::RedirectError,
        TokenKind::RedirectErrorAppend => TokenTag::RedirectErrorAppend,
        TokenKind::RedirectErrorToOutput => TokenTag::RedirectErrorToOutput,
        TokenKind::RedirectOutputAndError => TokenTag::RedirectOutputAndError,
        TokenKind::Arrow => TokenTag::Arrow,
        TokenKind::CaptureStart => TokenTag::CaptureStart,
        TokenKind::Unsupported(_) | TokenKind::InterpolationStart => TokenTag::Unsupported,
        TokenKind::Unknown => TokenTag::Unknown,
    }
}

fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut at = 0;
    while at < source.len() {
        let start = at;
        let ch = source[at..].chars().next().expect("cursor is in bounds");
        if ch == '\n' {
            at += 1;
            push(&mut tokens, TokenKind::Newline, start, at);
            continue;
        }
        if ch.is_whitespace() {
            at += ch.len_utf8();
            while at < source.len() {
                let c = source[at..].chars().next().unwrap();
                if c == '\n' || !c.is_whitespace() {
                    break;
                }
                at += c.len_utf8();
            }
            push(&mut tokens, TokenKind::Whitespace, start, at);
            continue;
        }
        if ch == '#' {
            at += 1;
            while at < source.len() && source.as_bytes()[at] != b'\n' {
                at += source[at..].chars().next().unwrap().len_utf8();
            }
            push(&mut tokens, TokenKind::Comment, start, at);
            continue;
        }
        if ch == '\'' || ch == '"' {
            let quote = ch;
            at += 1;
            let mut closed = false;
            while at < source.len() {
                let c = source[at..].chars().next().unwrap();
                if c == '\\' && quote == '"' {
                    at += c.len_utf8();
                    if at < source.len() {
                        at += source[at..].chars().next().unwrap().len_utf8();
                    }
                    continue;
                }
                at += c.len_utf8();
                if c == quote {
                    closed = true;
                    break;
                }
            }
            push(
                &mut tokens,
                if quote == '\'' {
                    TokenKind::SingleQuoted
                } else {
                    TokenKind::DoubleQuoted
                },
                start,
                at,
            );
            if !closed {
                diagnostics.push(Diagnostic::eof(
                    "L001",
                    format!("unclosed {quote} quote"),
                    source.len(),
                    &[&quote.to_string()],
                ));
            }
            continue;
        }
        if ch == '\\' {
            at += 1;
            if at < source.len() {
                at += source[at..].chars().next().unwrap().len_utf8();
            }
            push(&mut tokens, TokenKind::Word, start, at);
            continue;
        }
        if source[start..].starts_with("$(") {
            at += 2;
            push(&mut tokens, TokenKind::CaptureStart, start, at);
            continue;
        }
        if ch == '$' {
            at += 1;
            while at < source.len() {
                let c = source[at..].chars().next().unwrap();
                if !is_ident_continue(c) {
                    break;
                }
                at += c.len_utf8();
            }
            push(
                &mut tokens,
                if at > start + 1 {
                    TokenKind::DollarVariable
                } else {
                    TokenKind::Word
                },
                start,
                at,
            );
            continue;
        }
        if source[start..].starts_with("2>&1") {
            at += 4;
            push(&mut tokens, TokenKind::RedirectErrorToOutput, start, at);
            continue;
        }
        if source[start..].starts_with("2>>") {
            at += 3;
            push(&mut tokens, TokenKind::RedirectErrorAppend, start, at);
            continue;
        }
        if source[start..].starts_with("2>") {
            at += 2;
            push(&mut tokens, TokenKind::RedirectError, start, at);
            continue;
        }
        if source[start..].starts_with("&>") {
            at += 2;
            push(&mut tokens, TokenKind::RedirectOutputAndError, start, at);
            continue;
        }
        if is_ident_start(ch) {
            at += ch.len_utf8();
            while at < source.len() {
                let c = source[at..].chars().next().unwrap();
                if !is_ident_continue(c) {
                    break;
                }
                at += c.len_utf8();
            }
            let kind = match &source[start..at] {
                "let" => TokenKind::Let,
                "fn" => TokenKind::Fn,
                "if" => TokenKind::If,
                "else" => TokenKind::Else,
                "while" => TokenKind::While,
                "loop" => TokenKind::Loop,
                "try" => TokenKind::Try,
                "catch" => TokenKind::Catch,
                "throw" => TokenKind::Throw,
                "return" => TokenKind::Return,
                "break" => TokenKind::Break,
                "continue" => TokenKind::Continue,
                "status" => TokenKind::Status,
                "typeof" => TokenKind::Typeof,
                "true" => TokenKind::True,
                "false" => TokenKind::False,
                "null" => TokenKind::Null,
                _ => TokenKind::Identifier,
            };
            push(&mut tokens, kind, start, at);
            continue;
        }
        if ch.is_ascii_digit() {
            at += 1;
            while at < source.len() {
                let c = source[at..].chars().next().unwrap();
                if !(c.is_ascii_digit() || c == '.') {
                    break;
                }
                at += c.len_utf8();
            }
            push(&mut tokens, TokenKind::Number, start, at);
            continue;
        }
        let (kind, width) = if source[start..].starts_with("...") {
            (TokenKind::Ellipsis, 3)
        } else if source[start..].starts_with("!==") {
            (TokenKind::StrictNotEq, 3)
        } else if source[start..].starts_with("===") {
            (TokenKind::StrictEq, 3)
        } else if source[start..].starts_with("==") {
            (TokenKind::Unsupported("equality operator `==`"), 2)
        } else if source[start..].starts_with("=>") {
            (TokenKind::Arrow, 2)
        } else if source[start..].starts_with("+=") {
            (TokenKind::PlusAssign, 2)
        } else if source[start..].starts_with("-=") {
            (TokenKind::MinusAssign, 2)
        } else if source[start..].starts_with("&&") {
            (TokenKind::AndAnd, 2)
        } else if source[start..].starts_with("||") {
            (TokenKind::OrOr, 2)
        } else if source[start..].starts_with("//") {
            (TokenKind::DoubleSlash, 2)
        } else if source[start..].starts_with("<=") {
            (TokenKind::LessEq, 2)
        } else if source[start..].starts_with(">=") {
            (TokenKind::GreaterEq, 2)
        } else if source[start..].starts_with(">>") {
            (TokenKind::RedirectAppend, 2)
        } else {
            (
                match ch {
                    '=' => TokenKind::Assign,
                    '|' => TokenKind::Pipe,
                    ';' => TokenKind::Semicolon,
                    ',' => TokenKind::Comma,
                    '.' => TokenKind::Dot,
                    ':' => TokenKind::Colon,
                    '?' => TokenKind::Question,
                    '(' => TokenKind::LeftParen,
                    ')' => TokenKind::RightParen,
                    '[' => TokenKind::LeftBracket,
                    ']' => TokenKind::RightBracket,
                    '{' => TokenKind::LeftBrace,
                    '}' => TokenKind::RightBrace,
                    '+' => TokenKind::Plus,
                    '-' => TokenKind::Minus,
                    '*' => TokenKind::Star,
                    '/' => TokenKind::Slash,
                    '%' => TokenKind::Percent,
                    '!' => TokenKind::Bang,
                    '<' => TokenKind::Less,
                    '>' => TokenKind::Greater,
                    '&' => TokenKind::Unsupported("background commands"),
                    _ => TokenKind::Word,
                },
                ch.len_utf8(),
            )
        };
        at += width;
        push(&mut tokens, kind, start, at);
    }
    (tokens, diagnostics)
}

fn push(tokens: &mut Vec<Token>, kind: TokenKind, start: usize, end: usize) {
    tokens.push(Token {
        kind,
        mode: LexMode::Command,
        span: Span::new(start, end),
    });
}
fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch == '$' || unicode_ident::is_xid_start(ch)
}
fn is_ident_continue(ch: char) -> bool {
    ch == '_' || unicode_ident::is_xid_continue(ch)
}
fn unescape_command(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn decode_quoted_literal(raw: &str, kind: &TokenKind) -> String {
    let inner = raw.get(1..raw.len().saturating_sub(1)).unwrap_or("");
    if matches!(kind, TokenKind::DoubleQuoted | TokenKind::String) {
        unescape_command(inner)
    } else {
        inner.to_owned()
    }
}

fn find_matching(text: &str, mut at: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1;
    let mut quote = None;
    while at < text.len() {
        let ch = text[at..].chars().next()?;
        if let Some(active) = quote {
            if ch == '\\' && active == '"' {
                at += ch.len_utf8();
                if at < text.len() {
                    at += text[at..].chars().next()?.len_utf8();
                }
                continue;
            }
            if ch == active {
                quote = None;
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
        }
        at += ch.len_utf8();
    }
    None
}

fn offset_span(span: &mut Span, offset: usize) {
    span.start += offset;
    span.end += offset;
}
fn offset_diagnostic(diagnostic: &mut Diagnostic, offset: usize) {
    offset_span(&mut diagnostic.primary.span, offset);
    for label in &mut diagnostic.secondary {
        offset_span(&mut label.span, offset);
    }
}
fn offset_pipeline(pipeline: &mut Pipeline, offset: usize) {
    offset_span(&mut pipeline.span, offset);
    for stage in &mut pipeline.stages {
        offset_span(&mut stage.span, offset);
        for word in &mut stage.words {
            offset_command_word(word, offset);
        }
        for redirection in &mut stage.redirections {
            offset_span(&mut redirection.span, offset);
            if let Some(target) = &mut redirection.target {
                offset_command_word(target, offset);
            }
        }
    }
}
fn offset_command_word(word: &mut CommandWord, offset: usize) {
    offset_span(&mut word.span, offset);
    for part in &mut word.parts {
        offset_word_part(part, offset);
    }
}
fn offset_word_part(part: &mut WordPart, offset: usize) {
    match part {
        WordPart::Literal { span, .. }
        | WordPart::SingleQuoted { span, .. }
        | WordPart::Variable { span, .. }
        | WordPart::Missing { span }
        | WordPart::Error { span } => offset_span(span, offset),
        WordPart::Capture { pipeline, span } => {
            offset_span(span, offset);
            offset_pipeline(pipeline, offset);
        }
        WordPart::Evaluated { expr, span } => {
            offset_span(span, offset);
            offset_expr(expr, offset);
        }
        WordPart::DoubleQuoted { parts, span } => {
            offset_span(span, offset);
            for quoted in parts {
                match quoted {
                    QuotedPart::Capture(pipeline) => offset_pipeline(pipeline, offset),
                    QuotedPart::Expression(expr) => offset_expr(expr, offset),
                    QuotedPart::Literal(_) | QuotedPart::Variable(_) => {}
                }
            }
        }
    }
}
fn offset_expr(expr: &mut Expr, offset: usize) {
    match expr {
        Expr::Null(span)
        | Expr::Bool(_, span)
        | Expr::Int(_, span)
        | Expr::Float(_, span)
        | Expr::String(_, span)
        | Expr::Identifier(_, span)
        | Expr::Missing(span)
        | Expr::Error(span) => offset_span(span, offset),
        Expr::Array(values, span) => {
            offset_span(span, offset);
            for value in values {
                match value {
                    ArrayElement::Value(value) => offset_expr(value, offset),
                    ArrayElement::Spread(value, span) => {
                        offset_expr(value, offset);
                        offset_span(span, offset);
                    }
                }
            }
        }
        Expr::Object(entries, span) => {
            offset_span(span, offset);
            for entry in entries {
                match entry {
                    ObjectEntry::Property { value, span, .. }
                    | ObjectEntry::Spread { value, span } => {
                        offset_expr(value, offset);
                        offset_span(span, offset);
                    }
                }
            }
        }
        Expr::Unary { expr, span, .. } => {
            offset_span(span, offset);
            offset_expr(expr, offset);
        }
        Expr::Binary {
            left, right, span, ..
        } => {
            offset_span(span, offset);
            offset_expr(left, offset);
            offset_expr(right, offset);
        }
        Expr::Call { callee, args, span } => {
            offset_span(span, offset);
            offset_expr(callee, offset);
            for arg in args {
                match arg {
                    CallArg::Value(value) => offset_expr(value, offset),
                    CallArg::Spread(value, span) => {
                        offset_expr(value, offset);
                        offset_span(span, offset);
                    }
                }
            }
        }
        Expr::Member { object, span, .. } => {
            offset_span(span, offset);
            offset_expr(object, offset);
        }
        Expr::Index {
            object,
            index,
            span,
        } => {
            offset_span(span, offset);
            offset_expr(object, offset);
            offset_expr(index, offset);
        }
        Expr::Arrow {
            params, body, span, ..
        } => {
            offset_span(span, offset);
            for pattern in params {
                offset_pattern(pattern, offset);
            }
            match body {
                FunctionBody::Expression(expr) => offset_expr(expr, offset),
                FunctionBody::Block(program) => offset_program(program, offset),
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
            span,
        } => {
            offset_span(span, offset);
            offset_expr(condition, offset);
            offset_expr(then_expr, offset);
            offset_expr(else_expr, offset);
        }
        Expr::Capture { pipeline, span } => {
            offset_span(span, offset);
            offset_pipeline(pipeline, offset);
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            span,
        } => {
            offset_span(span, offset);
            offset_condition(condition, offset);
            offset_program(then_block, offset);
            if let Some(block) = else_block {
                offset_program(block, offset);
            }
        }
        Expr::Try {
            body,
            catch_pattern,
            catch_body,
            span,
        } => {
            offset_span(span, offset);
            offset_program(body, offset);
            offset_pattern(catch_pattern, offset);
            offset_program(catch_body, offset);
        }
    }
}

fn offset_pattern(pattern: &mut BindingPattern, offset: usize) {
    match pattern {
        BindingPattern::Name { span, .. } | BindingPattern::Missing { span } => {
            offset_span(span, offset);
        }
        BindingPattern::Array {
            items, rest, span, ..
        } => {
            offset_span(span, offset);
            for item in items {
                offset_pattern(item, offset);
            }
            if let Some(rest) = rest {
                offset_pattern(rest, offset);
            }
        }
        BindingPattern::Object {
            entries,
            rest,
            span,
            ..
        } => {
            offset_span(span, offset);
            for (_, pattern) in entries {
                offset_pattern(pattern, offset);
            }
            if let Some(rest) = rest {
                offset_pattern(rest, offset);
            }
        }
    }
}

fn offset_program(program: &mut Program, offset: usize) {
    offset_span(&mut program.span, offset);
    for statement in &mut program.statements {
        match statement {
            Statement::Command(pipeline) => offset_pipeline(pipeline, offset),
            Statement::CommandChain { head, tail, span } => {
                offset_span(span, offset);
                offset_pipeline(head, offset);
                for (_, pipeline) in tail {
                    offset_pipeline(pipeline, offset);
                }
            }
            Statement::Status { pipeline, span } => {
                offset_span(span, offset);
                offset_pipeline(pipeline, offset);
            }
            Statement::Assignment { value, span, .. } => {
                offset_span(span, offset);
                offset_expr(value, offset);
            }
            Statement::EnvironmentAssignment {
                target,
                value,
                span,
            } => {
                offset_span(span, offset);
                match target {
                    EnvironmentTarget::Member { span, .. } => offset_span(span, offset),
                    EnvironmentTarget::Index { key, span } => {
                        offset_span(span, offset);
                        offset_expr(key, offset);
                    }
                }
                offset_expr(value, offset);
            }
            Statement::Let {
                pattern,
                value,
                span,
                ..
            } => {
                offset_span(span, offset);
                offset_pattern(pattern, offset);
                offset_expr(value, offset);
            }
            Statement::Function {
                params, body, span, ..
            } => {
                offset_span(span, offset);
                for pattern in params {
                    offset_pattern(pattern, offset);
                }
                offset_program(body, offset);
            }
            Statement::Expr(expr) => offset_expr(expr, offset),
            Statement::While {
                condition,
                body,
                span,
            } => {
                offset_span(span, offset);
                offset_condition(condition, offset);
                offset_program(body, offset);
            }
            Statement::Loop { body, span } => {
                offset_span(span, offset);
                offset_program(body, offset);
            }
            Statement::Throw { value, span } => {
                offset_span(span, offset);
                offset_expr(value, offset);
            }
            Statement::Return { value, span } => {
                offset_span(span, offset);
                if let Some(value) = value {
                    offset_expr(value, offset);
                }
            }
            Statement::Break { span }
            | Statement::Continue { span }
            | Statement::Missing { span }
            | Statement::Error { span } => offset_span(span, offset),
        }
    }
}

fn offset_condition(condition: &mut IfCondition, offset: usize) {
    match condition {
        IfCondition::Expr(expr) => offset_expr(expr, offset),
        IfCondition::Command(pipeline) => offset_pipeline(pipeline, offset),
    }
}
