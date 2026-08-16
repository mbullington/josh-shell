use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use josh_syntax::{
    ArrayElement, AssignOp, BinaryOp, BindingPattern, CallArg, ChainOp, CommandWord, Diagnostic,
    EnvironmentTarget, Expr, ExternalCommand, FunctionBody, IfCondition, ObjectEntry, Pipeline,
    Program, QuotedPart, RedirectionKind, Statement, UnaryOp, WordPart, parse,
};
use thiserror::Error;

use crate::{
    context::{ShellContext, ShellContextError, ShellSnapshot},
    host::{
        CancellationToken, Captured, CommandSpec, ExecutionError, ExecutionHost, ExecutionResult,
        RedirectionSpec, StreamStage,
    },
    value::{
        BuiltinFunction, ErrorValue, Frame, FunctionKind, FunctionValue, ObjectValue, StatusValue,
        Value,
    },
};

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("parse failed")]
    Parse(Vec<Diagnostic>),
    #[error("undefined identifier `{name}`; use $({name}) to capture a command")]
    Undefined { name: String },
    #[error("{0}")]
    Type(String),
    #[error("{0} is planned but not implemented")]
    Unsupported(String),
    #[error(transparent)]
    Process(#[from] ExecutionError),
    #[error(transparent)]
    ShellContext(#[from] ShellContextError),
    #[error("uncaught value: {0}")]
    Uncaught(Value),
    #[error("{0} is only valid inside its enclosing construct")]
    InvalidControl(&'static str),
}

impl EngineError {
    fn into_value(self) -> Value {
        let kind = match &self {
            Self::Undefined { .. } => "undefined",
            Self::Type(_) => "type",
            Self::Unsupported(_) => "unsupported",
            Self::Process(_) => "process",
            Self::ShellContext(ShellContextError::ChangeDirectory { .. }) => "filesystem",
            Self::ShellContext(_) => "environment",
            Self::Parse(_) => "parse",
            Self::Uncaught(_) => "uncaught",
            Self::InvalidControl(_) => "control-flow",
        };
        Value::Error(Arc::new(ErrorValue::new(kind, self.to_string())))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunResult {
    Value(Value),
    Exit(i32),
}

enum Unwind {
    Throw(Value),
    Return(Value),
    Break,
    Continue,
    Exit(i32),
    Error(EngineError),
}

type EvalResult<T> = Result<T, Unwind>;

impl From<EngineError> for Unwind {
    fn from(error: EngineError) -> Self {
        Self::Error(error)
    }
}

impl From<ShellContextError> for Unwind {
    fn from(error: ShellContextError) -> Self {
        Self::Error(error.into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StreamPort {
    Bytes,
    Values,
}

struct Completion {
    value: Value,
    status: StatusValue,
    failure: Option<Value>,
}

impl Completion {
    fn success(value: Value) -> Self {
        Self {
            value,
            status: StatusValue::new(Vec::new()),
            failure: None,
        }
    }
}

pub const MAX_CHUNK_SIZE: usize = 64 * 1024;

pub struct Engine {
    host: Box<dyn ExecutionHost>,
    context: ShellContext,
    frames: Vec<Frame>,
    execution_cancellation: CancellationToken,
}

impl Engine {
    pub fn new(host: impl ExecutionHost + 'static) -> Self {
        Self::with_shell_context(host, ShellContext::from_process())
    }

    pub fn with_shell_context(host: impl ExecutionHost + 'static, context: ShellContext) -> Self {
        Self::with_shell_context_and_cancellation_token(host, context, CancellationToken::default())
    }

    pub fn with_execution_cancellation(
        host: impl ExecutionHost + 'static,
        execution_cancellation: Arc<AtomicBool>,
    ) -> Self {
        Self::with_shell_context_and_cancellation_token(
            host,
            ShellContext::from_process(),
            CancellationToken::from_flag(execution_cancellation),
        )
    }

    pub fn with_cancellation_token(
        host: impl ExecutionHost + 'static,
        execution_cancellation: CancellationToken,
    ) -> Self {
        Self::with_shell_context_and_cancellation_token(
            host,
            ShellContext::from_process(),
            execution_cancellation,
        )
    }

    pub fn with_shell_context_and_cancellation_token(
        host: impl ExecutionHost + 'static,
        context: ShellContext,
        execution_cancellation: CancellationToken,
    ) -> Self {
        Self {
            host: Box::new(host),
            context,
            frames: vec![Frame::new()],
            execution_cancellation,
        }
    }

    #[must_use]
    pub fn execution_cancellation(&self) -> Arc<AtomicBool> {
        self.execution_cancellation.local_flag()
    }

    #[must_use]
    pub fn shell_context(&self) -> ShellContext {
        self.context.clone()
    }

    #[must_use]
    pub fn shell_snapshot(&self) -> ShellSnapshot {
        self.context.snapshot()
    }

    #[must_use]
    pub fn environment_variable_os(&self, name: &str) -> Option<OsString> {
        self.context.environment_variable(OsStr::new(name))
    }

    pub fn run_source(&mut self, source: impl Into<Arc<str>>) -> Result<RunResult, EngineError> {
        let parsed = parse(source);
        let program = parsed
            .strict_program()
            .map_err(|diagnostics| EngineError::Parse(diagnostics.to_vec()))?;
        self.run_program(program)
    }

    pub fn run_program(&mut self, program: &Program) -> Result<RunResult, EngineError> {
        match self.eval_program(program) {
            Ok(value) => Ok(RunResult::Value(value)),
            Err(Unwind::Exit(code)) => Ok(RunResult::Exit(code)),
            Err(Unwind::Throw(value)) => Err(EngineError::Uncaught(value)),
            Err(Unwind::Return(_)) => Err(EngineError::InvalidControl("return")),
            Err(Unwind::Break) => Err(EngineError::InvalidControl("break")),
            Err(Unwind::Continue) => Err(EngineError::InvalidControl("continue")),
            Err(Unwind::Error(error)) => Err(error),
        }
    }

    #[must_use]
    pub fn variable_names(&self) -> Vec<String> {
        let mut names = self
            .frames
            .iter()
            .flat_map(BTreeMap::keys)
            .cloned()
            .collect::<Vec<_>>();
        names.extend(self.context.environment_names());
        names.push("env".into());
        names.sort();
        names.dedup();
        names
    }

    pub fn prompt(&mut self) -> Result<Option<String>, EngineError> {
        let Some(value) = self.resolve_lexical("prompt") else {
            return Ok(None);
        };
        let Value::Function(function) = value else {
            return Err(EngineError::Type(
                "lexical `prompt` must be a zero-argument function".into(),
            ));
        };
        if !matches!(&function.kind, FunctionKind::User { params, .. } if params.is_empty()) {
            return Err(EngineError::Type(
                "lexical `prompt` must be a zero-argument user function".into(),
            ));
        }
        let value = match self.call_function(function, Vec::new()) {
            Ok(value) => value,
            Err(Unwind::Throw(value)) => return Err(EngineError::Uncaught(value)),
            Err(Unwind::Error(error)) => return Err(error),
            Err(Unwind::Exit(_)) => {
                return Err(EngineError::Unsupported("exit from prompt()".into()));
            }
            Err(Unwind::Return(_)) => return Err(EngineError::InvalidControl("return")),
            Err(Unwind::Break) => return Err(EngineError::InvalidControl("break")),
            Err(Unwind::Continue) => return Err(EngineError::InvalidControl("continue")),
        };
        let Value::String(prompt) = value else {
            return Err(EngineError::Type(format!(
                "prompt() must return a string, got {}",
                value.type_name()
            )));
        };
        Ok(Some(prompt.to_string()))
    }

    pub fn call_stream_function(
        &mut self,
        function: Arc<FunctionValue>,
        value: Value,
    ) -> Result<Value, EngineError> {
        match self.call_function(function, vec![value]) {
            Ok(value) => Ok(value),
            Err(Unwind::Throw(value)) => Err(EngineError::Uncaught(value)),
            Err(Unwind::Error(error)) => Err(error),
            Err(Unwind::Exit(_)) => Err(EngineError::Unsupported(
                "exit from a pipeline function".into(),
            )),
            Err(Unwind::Return(_)) => Err(EngineError::InvalidControl("return")),
            Err(Unwind::Break) => Err(EngineError::InvalidControl("break")),
            Err(Unwind::Continue) => Err(EngineError::InvalidControl("continue")),
        }
    }

    fn eval_program(&mut self, program: &Program) -> EvalResult<Value> {
        let mut last = Value::Null;
        for statement in &program.statements {
            last = self.eval_statement(statement)?;
        }
        Ok(last)
    }

    #[allow(clippy::too_many_lines)]
    fn eval_statement(&mut self, statement: &Statement) -> EvalResult<Value> {
        match statement {
            Statement::Command(pipeline) => {
                let completion = self.run_pipeline(pipeline, false)?;
                if let Some(failure) = completion.failure {
                    Err(Unwind::Throw(failure))
                } else {
                    Ok(completion.value)
                }
            }
            Statement::CommandChain { head, tail, .. } => {
                let mut completion = self.run_pipeline(head, false)?;
                for (op, pipeline) in tail {
                    let should_run = match op {
                        ChainOp::And => completion.status.success(),
                        ChainOp::Or => !completion.status.success(),
                    };
                    if should_run {
                        completion = self.run_pipeline(pipeline, false)?;
                    }
                }
                Ok(completion.value)
            }
            Statement::Status { pipeline, .. } => {
                let completion = self.run_pipeline(pipeline, false)?;
                Ok(Value::Status(Arc::new(completion.status)))
            }
            Statement::Assignment {
                name, op, value, ..
            } => {
                reject_reserved_name(name)?;
                let rhs = self.eval_expr(value)?;
                let value = match op {
                    AssignOp::Assign => rhs,
                    AssignOp::Add => self.apply_binary(
                        self.resolve_lexical(name).ok_or_else(|| undefined(name))?,
                        BinaryOp::Add,
                        rhs,
                    )?,
                    AssignOp::Subtract => self.apply_binary(
                        self.resolve_lexical(name).ok_or_else(|| undefined(name))?,
                        BinaryOp::Subtract,
                        rhs,
                    )?,
                };
                self.assign(name, value.clone());
                Ok(value)
            }
            Statement::EnvironmentAssignment { target, value, .. } => {
                let name = match target {
                    EnvironmentTarget::Member { name, .. } => name.clone(),
                    EnvironmentTarget::Index { key, .. } => {
                        let key = self.eval_expr(key)?;
                        let Value::String(name) = key else {
                            return Err(type_error(format!(
                                "environment variable name must be a string, got {}",
                                key.type_name()
                            )));
                        };
                        name.to_string()
                    }
                };
                let value = self.eval_expr(value)?;
                let exported = environment_assignment_value(&name, &value)?;
                self.context.set_environment_variable(&name, exported)?;
                Ok(value)
            }
            Statement::Let { pattern, value, .. } => {
                let value = self.eval_expr(value)?;
                self.bind_pattern(pattern, value.clone())?;
                Ok(value)
            }
            Statement::Function {
                name, params, body, ..
            } => {
                reject_reserved_name(name)?;
                for param in params {
                    reject_reserved_pattern(param)?;
                }
                let function = Value::Function(Arc::new(FunctionValue::user(
                    Some(Arc::from(name.as_str())),
                    params.clone(),
                    FunctionBody::Block(body.clone()),
                    self.snapshot(),
                )));
                self.current_frame().insert(name.clone(), function.clone());
                Ok(function)
            }
            Statement::Expr(expr) => self.eval_expr(expr),
            Statement::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                if self.eval_condition(condition)? {
                    self.eval_block(then_block)
                } else if let Some(block) = else_block {
                    self.eval_block(block)
                } else {
                    Ok(Value::Null)
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                let mut last = Value::Null;
                while self.eval_condition(condition)? {
                    match self.eval_block(body) {
                        Ok(value) => last = value,
                        Err(Unwind::Continue) => {}
                        Err(Unwind::Break) => break,
                        Err(unwind) => return Err(unwind),
                    }
                }
                Ok(last)
            }
            Statement::Loop { body, .. } => {
                let mut last = Value::Null;
                loop {
                    match self.eval_block(body) {
                        Ok(value) => last = value,
                        Err(Unwind::Continue) => {}
                        Err(Unwind::Break) => break,
                        Err(unwind) => return Err(unwind),
                    }
                }
                Ok(last)
            }
            Statement::Try {
                body,
                catch_pattern,
                catch_body,
                ..
            } => match self.eval_block(body) {
                Ok(value) => Ok(value),
                Err(Unwind::Throw(value)) => self.eval_catch(catch_pattern, catch_body, value),
                Err(Unwind::Error(error)) => {
                    let value = error.into_value();
                    self.eval_catch(catch_pattern, catch_body, value)
                }
                Err(unwind) => Err(unwind),
            },
            Statement::Throw { value, .. } => Err(Unwind::Throw(self.eval_expr(value)?)),
            Statement::Return { value, .. } => Err(Unwind::Return(
                value
                    .as_ref()
                    .map_or(Ok(Value::Null), |expr| self.eval_expr(expr))?,
            )),
            Statement::Break { .. } => Err(Unwind::Break),
            Statement::Continue { .. } => Err(Unwind::Continue),
            Statement::Missing { .. } | Statement::Error { .. } => {
                Err(type_error("cannot evaluate a recovered syntax node"))
            }
        }
    }

    fn eval_block(&mut self, program: &Program) -> EvalResult<Value> {
        self.frames.push(Frame::new());
        let result = self.eval_program(program);
        self.frames.pop();
        result
    }

    fn eval_catch(
        &mut self,
        pattern: &BindingPattern,
        body: &Program,
        value: Value,
    ) -> EvalResult<Value> {
        self.frames.push(Frame::new());
        let result = self
            .bind_pattern(pattern, value)
            .and_then(|()| self.eval_program(body));
        self.frames.pop();
        result
    }

    fn eval_condition(&mut self, condition: &IfCondition) -> EvalResult<bool> {
        match condition {
            IfCondition::Expr(expr) => Ok(self.eval_expr(expr)?.truthy()),
            IfCondition::Command(pipeline) => {
                let completion = self.run_pipeline(pipeline, false)?;
                Ok(completion.status.success())
            }
        }
    }

    fn run_pipeline(&mut self, pipeline: &Pipeline, capture: bool) -> EvalResult<Completion> {
        if pipeline.stages.len() > 1 {
            validate_stream_shapes(
                pipeline
                    .stages
                    .iter()
                    .map(|stage| self.structural_stream_shape(stage)),
            )?;
        }
        if let [stage] = pipeline.stages.as_slice() {
            let command = self.eval_command(stage)?;
            return self.run_standalone(command, capture);
        }

        let stages = pipeline
            .stages
            .iter()
            .enumerate()
            .map(|(index, stage)| self.eval_stream_stage(index, stage))
            .collect::<EvalResult<Vec<_>>>()?;
        validate_stream_shapes(stages.iter().map(stream_shape))?;

        if stages
            .iter()
            .all(|stage| matches!(stage, StreamStage::External(_)))
        {
            let argvs = stages
                .into_iter()
                .map(|stage| match stage {
                    StreamStage::External(command) => command,
                    _ => unreachable!("all stages were checked as external"),
                })
                .collect();
            return self.run_external(argvs, capture);
        }
        let result = self.host.execute_stream(
            stages,
            capture,
            self.execution_cancellation.clone(),
            self.context.clone(),
        );
        Self::complete_execution(result)
    }

    fn structural_stream_shape(&self, stage: &ExternalCommand) -> StreamShape {
        if matches!(
            stage.words.as_slice(),
            [CommandWord {
                parts,
                ..
            }] if matches!(parts.as_slice(), [WordPart::Evaluated { .. }])
        ) {
            return StreamShape::Function;
        }
        let name = stage.words.first().and_then(plain_command_word);
        match name.as_deref() {
            Some("text") => StreamShape::Text,
            Some("json" | "lines" | "jsonl" | "chunks") => StreamShape::BytesToValues,
            Some(name) if parse_chunks_name(name).is_some() => StreamShape::BytesToValues,
            Some("map" | "filter") => StreamShape::Function,
            Some("take" | "first" | "collect") => StreamShape::Values,
            Some(name) if matches!(self.resolve_lexical(name), Some(Value::Function(_))) => {
                StreamShape::Function
            }
            _ => StreamShape::External,
        }
    }

    fn eval_stream_stage(
        &mut self,
        index: usize,
        stage: &ExternalCommand,
    ) -> EvalResult<StreamStage> {
        if stage.words.len() == 1
            && let Some(value) = self.eval_standalone_word(&stage.words[0])?
        {
            if !stage.redirections.is_empty() {
                return Err(type_error(format!(
                    "pipeline stage {index} redirections require an external command"
                )));
            }
            let Value::Function(function) = value else {
                return Err(type_error(format!(
                    "pipeline stage {index} expression must evaluate to a function"
                )));
            };
            return Ok(StreamStage::Function(function));
        }

        let name = stage.words.first().and_then(plain_command_word);
        if !stage.redirections.is_empty()
            && name.as_deref().is_some_and(|name| {
                matches!(
                    name,
                    "text"
                        | "json"
                        | "lines"
                        | "jsonl"
                        | "first"
                        | "collect"
                        | "map"
                        | "filter"
                        | "take"
                        | "chunks"
                ) || parse_chunks_name(name).is_some()
                    || matches!(self.resolve_lexical(name), Some(Value::Function(_)))
            })
        {
            return Err(type_error(format!(
                "pipeline stage {index} redirections require an external command"
            )));
        }
        match name.as_deref() {
            Some("text") => {
                expect_stream_arity(index, "text", &stage.words, 1)?;
                Ok(StreamStage::Text)
            }
            Some("json") => {
                expect_stream_arity(index, "json", &stage.words, 1)?;
                Ok(StreamStage::Json)
            }
            Some("lines") => {
                expect_stream_arity(index, "lines", &stage.words, 1)?;
                Ok(StreamStage::Lines)
            }
            Some("jsonl") => {
                expect_stream_arity(index, "jsonl", &stage.words, 1)?;
                Ok(StreamStage::JsonLines)
            }
            Some("first") => {
                expect_stream_arity(index, "first", &stage.words, 1)?;
                Ok(StreamStage::First)
            }
            Some("collect") => {
                expect_stream_arity(index, "collect", &stage.words, 1)?;
                Ok(StreamStage::Collect)
            }
            Some("map") | Some("filter") => {
                expect_stream_arity(index, name.as_deref().unwrap(), &stage.words, 2)?;
                let value = self
                    .eval_standalone_word(&stage.words[1])?
                    .or_else(|| {
                        plain_command_word(&stage.words[1])
                            .and_then(|name| self.resolve_lexical(&name))
                    })
                    .ok_or_else(|| {
                        type_error(format!(
                            "pipeline stage {index} {} expects a function value",
                            name.as_deref().unwrap()
                        ))
                    })?;
                let Value::Function(function) = value else {
                    return Err(type_error(format!(
                        "pipeline stage {index} {} expects a function value",
                        name.as_deref().unwrap()
                    )));
                };
                if name.as_deref() == Some("map") {
                    Ok(StreamStage::Map(function))
                } else {
                    Ok(StreamStage::Filter(function))
                }
            }
            Some("take") => {
                expect_stream_arity(index, "take", &stage.words, 2)?;
                Ok(StreamStage::Take(self.eval_nonnegative_size(
                    index,
                    "take",
                    &stage.words[1],
                    true,
                )?))
            }
            Some(name) if parse_chunks_name(name).is_some() => {
                expect_stream_arity(index, "chunks(n)", &stage.words, 1)?;
                Ok(StreamStage::Chunks(validate_chunk_size(
                    index,
                    parse_chunks_name(name).unwrap(),
                )?))
            }
            Some("chunks") => {
                expect_stream_arity(index, "chunks", &stage.words, 2)?;
                let size = self.eval_nonnegative_size(index, "chunks", &stage.words[1], false)?;
                Ok(StreamStage::Chunks(validate_chunk_size(index, size)?))
            }
            _ => {
                let command = self.eval_command(stage)?;
                if command
                    .argv
                    .first()
                    .is_some_and(|name| matches!(name.as_slice(), b"cd" | b"exit"))
                {
                    return Err(EngineError::Unsupported(
                        "builtins inside pipelines or captures".into(),
                    )
                    .into());
                }
                if stage.words.len() == 1
                    && let Some(name) = name
                    && let Some(Value::Function(function)) = self.resolve_lexical(&name)
                {
                    return Ok(StreamStage::Function(function));
                }
                Ok(StreamStage::External(command))
            }
        }
    }

    fn eval_standalone_word(&mut self, word: &CommandWord) -> EvalResult<Option<Value>> {
        let [WordPart::Evaluated { expr, .. }] = word.parts.as_slice() else {
            return Ok(None);
        };
        self.eval_expr(expr).map(Some)
    }

    fn eval_nonnegative_size(
        &mut self,
        stage: usize,
        name: &str,
        word: &CommandWord,
        allow_zero: bool,
    ) -> EvalResult<usize> {
        let value = if let Some(value) = self.eval_standalone_word(word)? {
            value
        } else if let Some(text) = plain_command_word(word) {
            Value::Int(text.parse().map_err(|_| {
                type_error(format!("pipeline stage {stage} {name} expects an integer"))
            })?)
        } else {
            return Err(type_error(format!(
                "pipeline stage {stage} {name} expects an integer"
            )));
        };
        let Value::Int(value) = value else {
            return Err(type_error(format!(
                "pipeline stage {stage} {name} expects an integer"
            )));
        };
        if value < 0 || (!allow_zero && value == 0) {
            return Err(type_error(format!(
                "pipeline stage {stage} {name} expects {} integer",
                if allow_zero {
                    "a nonnegative"
                } else {
                    "a positive"
                }
            )));
        }
        usize::try_from(value).map_err(|_| {
            type_error(format!(
                "pipeline stage {stage} {name} is too large for this platform"
            ))
        })
    }

    fn run_standalone(&mut self, command: CommandSpec, capture: bool) -> EvalResult<Completion> {
        let CommandSpec { argv, redirections } = command;
        let Some(name) = argv.first() else {
            return Err(type_error("empty command"));
        };
        if name.as_slice() == b"exit" {
            if !redirections.is_empty() {
                return Err(EngineError::Unsupported("redirections on builtins".into()).into());
            }
            if capture {
                return Err(EngineError::Unsupported("builtins inside captures".into()).into());
            }
            if argv.len() > 2 {
                return Err(type_error("exit expects zero or one status operand"));
            }
            let code = argv
                .get(1)
                .map(|value| {
                    String::from_utf8_lossy(value)
                        .parse::<i32>()
                        .map_err(|_| type_error("exit status must be an integer"))
                })
                .transpose()?
                .unwrap_or(0);
            return Err(Unwind::Exit(code));
        }
        if name.as_slice() == b"cd" {
            if !redirections.is_empty() {
                return Err(EngineError::Unsupported("redirections on builtins".into()).into());
            }
            if capture {
                return Err(EngineError::Unsupported("builtins inside captures".into()).into());
            }
            if argv.len() > 2 {
                return Err(type_error("cd expects zero or one path operand"));
            }
            let target = argv
                .get(1)
                .cloned()
                .or_else(|| {
                    self.context
                        .environment_variable(OsStr::new("HOME"))
                        .and_then(os_string_bytes)
                })
                .unwrap_or_else(|| b".".to_vec());
            let target = os_string_from_bytes(target).map_err(type_error)?;
            self.context.change_directory(&target)?;
            return Ok(Completion::success(Value::Null));
        }
        if let Ok(name) = std::str::from_utf8(name)
            && let Some(Value::Function(function)) = self.resolve_lexical(name)
        {
            if !redirections.is_empty() {
                return Err(
                    EngineError::Unsupported("redirections on lexical functions".into()).into(),
                );
            }
            if capture {
                return Err(
                    EngineError::Unsupported("lexical functions inside captures".into()).into(),
                );
            }
            let args = argv[1..]
                .iter()
                .map(|value| match String::from_utf8(value.clone()) {
                    Ok(text) => Value::String(Arc::from(text)),
                    Err(error) => Value::Bytes(Arc::from(error.into_bytes())),
                })
                .collect();
            return self.call_function(function, args).map(Completion::success);
        }
        self.run_external(vec![CommandSpec { argv, redirections }], capture)
    }

    fn run_external(
        &mut self,
        commands: Vec<CommandSpec>,
        capture: bool,
    ) -> EvalResult<Completion> {
        let result = self.host.execute(
            commands,
            capture,
            self.execution_cancellation.clone(),
            self.context.clone(),
        );
        Self::complete_execution(result)
    }

    fn complete_execution(
        result: Result<ExecutionResult, ExecutionError>,
    ) -> EvalResult<Completion> {
        match result {
            Ok(result) => Ok(completion_from_result(result)),
            Err(
                error @ (ExecutionError::CommandFailed { .. }
                | ExecutionError::PipelineFailed { .. }),
            ) => {
                let status = status_from_completed_error(&error)
                    .expect("completed process errors always contain outcomes");
                let failure = Value::Error(Arc::new(ErrorValue::with_status(
                    "command",
                    error.to_string(),
                    status.clone(),
                )));
                Ok(Completion {
                    value: Value::Null,
                    status,
                    failure: Some(failure),
                })
            }
            Err(error) => Err(EngineError::Process(error).into()),
        }
    }

    fn eval_command(&mut self, command: &ExternalCommand) -> EvalResult<CommandSpec> {
        let argv = self.eval_command_words(&command.words)?;
        let redirections = command
            .redirections
            .iter()
            .map(|redirection| {
                let path = redirection
                    .target
                    .as_ref()
                    .map(|target| self.eval_redirection_target(target))
                    .transpose()?;
                Ok(match redirection.kind {
                    RedirectionKind::Input => RedirectionSpec::Input(path.expect("target parsed")),
                    RedirectionKind::Output => {
                        RedirectionSpec::Output(path.expect("target parsed"))
                    }
                    RedirectionKind::Append => {
                        RedirectionSpec::Append(path.expect("target parsed"))
                    }
                    RedirectionKind::Error => RedirectionSpec::Error(path.expect("target parsed")),
                    RedirectionKind::ErrorAppend => {
                        RedirectionSpec::ErrorAppend(path.expect("target parsed"))
                    }
                    RedirectionKind::ErrorToOutput => RedirectionSpec::ErrorToOutput,
                    RedirectionKind::OutputAndError => {
                        RedirectionSpec::OutputAndError(path.expect("target parsed"))
                    }
                })
            })
            .collect::<EvalResult<Vec<_>>>()?;
        Ok(CommandSpec { argv, redirections })
    }

    fn eval_redirection_target(&mut self, word: &CommandWord) -> EvalResult<Vec<u8>> {
        let values = self.eval_command_word(word)?;
        if values.len() != 1 {
            return Err(type_error(format!(
                "redirection target must evaluate to exactly one path, got {}",
                values.len()
            )));
        }
        Ok(values.into_iter().next().expect("one target"))
    }

    fn eval_command_words(&mut self, words: &[CommandWord]) -> EvalResult<Vec<Vec<u8>>> {
        let mut argv = Vec::new();
        for word in words {
            if let [WordPart::Variable { name, .. }] = word.parts.as_slice()
                && let Value::Array(values) = self.resolve_variable(name)?
            {
                for value in values.iter() {
                    argv.extend(self.expand_glob(value_to_bytes(value)?, true)?);
                }
                continue;
            }
            argv.extend(self.eval_command_word(word)?);
        }
        Ok(argv)
    }

    fn eval_command_word(&mut self, word: &CommandWord) -> EvalResult<Vec<Vec<u8>>> {
        let mut bytes = Vec::new();
        let mut pattern = Vec::new();
        let mut glob_active = false;
        for part in &word.parts {
            let value = self.eval_word_part(part)?;
            let unquoted = match part {
                WordPart::Literal { glob_unquoted, .. } => *glob_unquoted,
                WordPart::Variable { .. }
                | WordPart::Capture { .. }
                | WordPart::Evaluated { .. } => true,
                WordPart::SingleQuoted { .. } | WordPart::DoubleQuoted { .. } => false,
                WordPart::Missing { .. } | WordPart::Error { .. } => false,
            };
            glob_active |= unquoted && contains_glob_meta(&value);
            bytes.extend(&value);
            if unquoted {
                pattern.extend(value);
            } else {
                append_glob_literal(&mut pattern, &value);
            }
        }
        if glob_active {
            self.host
                .glob(&pattern, &self.context)
                .map_err(EngineError::from)
                .map_err(Into::into)
        } else {
            Ok(vec![bytes])
        }
    }

    fn expand_glob(&self, bytes: Vec<u8>, eligible: bool) -> EvalResult<Vec<Vec<u8>>> {
        if eligible && contains_glob_meta(&bytes) {
            self.host
                .glob(&bytes, &self.context)
                .map_err(EngineError::from)
                .map_err(Into::into)
        } else {
            Ok(vec![bytes])
        }
    }

    fn eval_word_part(&mut self, part: &WordPart) -> EvalResult<Vec<u8>> {
        match part {
            WordPart::Literal { value, .. } | WordPart::SingleQuoted { value, .. } => {
                Ok(value.as_bytes().to_vec())
            }
            WordPart::Variable { name, .. } => value_to_bytes(&self.resolve_variable(name)?),
            WordPart::Capture { pipeline, .. } => {
                let completion = self.run_pipeline(pipeline, true)?;
                if let Some(failure) = completion.failure {
                    Err(Unwind::Throw(failure))
                } else {
                    value_to_bytes(&completion.value)
                }
            }
            WordPart::Evaluated { expr, .. } => value_to_bytes(&self.eval_expr(expr)?),
            WordPart::DoubleQuoted { parts, .. } => {
                let mut output = Vec::new();
                for part in parts {
                    match part {
                        QuotedPart::Literal(text) => output.extend(text.as_bytes()),
                        QuotedPart::Variable(name) => {
                            let value = self.resolve_variable(name)?;
                            if let Value::Array(values) = value {
                                for (index, value) in values.iter().enumerate() {
                                    if index > 0 {
                                        output.push(b' ');
                                    }
                                    output.extend(value_to_bytes(value)?);
                                }
                            } else {
                                output.extend(value_to_bytes(&value)?);
                            }
                        }
                        QuotedPart::Capture(pipeline) => {
                            let completion = self.run_pipeline(pipeline, true)?;
                            if let Some(failure) = completion.failure {
                                return Err(Unwind::Throw(failure));
                            }
                            output.extend(value_to_bytes(&completion.value)?);
                        }
                        QuotedPart::Expression(expr) => {
                            output.extend(value_to_bytes(&self.eval_expr(expr)?)?);
                        }
                    }
                }
                Ok(output)
            }
            WordPart::Missing { .. } | WordPart::Error { .. } => {
                Err(type_error("cannot evaluate a recovered command word"))
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn eval_expr(&mut self, expr: &Expr) -> EvalResult<Value> {
        match expr {
            Expr::Null(_) => Ok(Value::Null),
            Expr::Bool(value, _) => Ok(Value::Bool(*value)),
            Expr::Int(value, _) => Ok(Value::Int(*value)),
            Expr::Float(value, _) => Ok(Value::Float(*value)),
            Expr::String(value, _) => Ok(Value::String(Arc::from(value.as_str()))),
            Expr::Identifier(name, _) => {
                self.resolve_expression(name).ok_or_else(|| undefined(name))
            }
            Expr::Array(elements, _) => {
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        ArrayElement::Value(expr) => values.push(self.eval_expr(expr)?),
                        ArrayElement::Spread(expr, _) => {
                            let Value::Array(spread) = self.eval_expr(expr)? else {
                                return Err(type_error("array spread requires an array"));
                            };
                            values.extend(spread.iter().cloned());
                        }
                    }
                }
                Ok(Value::Array(Arc::new(values)))
            }
            Expr::Object(entries, _) => {
                let mut object = ObjectValue::new();
                for entry in entries {
                    match entry {
                        ObjectEntry::Property { key, value, .. } => {
                            object.insert(Arc::from(key.as_str()), self.eval_expr(value)?);
                        }
                        ObjectEntry::Spread { value, .. } => {
                            let Value::Object(spread) = self.eval_expr(value)? else {
                                return Err(type_error("object spread requires an object"));
                            };
                            for (key, value) in spread.iter() {
                                object.insert(Arc::clone(key), value.clone());
                            }
                        }
                    }
                }
                Ok(Value::Object(Arc::new(object)))
            }
            Expr::Unary { op, expr, .. } => {
                let value = self.eval_expr(expr)?;
                match (op, value) {
                    (UnaryOp::Not, value) => Ok(Value::Bool(!value.truthy())),
                    (UnaryOp::Negate, Value::Int(value)) => value
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| type_error("integer negation overflow")),
                    (UnaryOp::Negate, Value::Float(value)) => Ok(Value::Float(-value)),
                    (UnaryOp::Typeof, value) => Ok(Value::String(Arc::from(value.type_name()))),
                    (UnaryOp::Negate, _) => Err(type_error("unary `-` requires a number")),
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let left = self.eval_expr(left)?;
                if *op == BinaryOp::And && !left.truthy() {
                    return Ok(left);
                }
                if *op == BinaryOp::Or && left.truthy() {
                    return Ok(left);
                }
                let right = self.eval_expr(right)?;
                self.apply_binary(left, *op, right)
            }
            Expr::Call { callee, args, .. } => self.eval_call(callee, args),
            Expr::Member { object, name, .. } => {
                let object = self.eval_expr(object)?;
                self.member_value(&object, name)
            }
            Expr::Index { object, index, .. } => {
                let object = self.eval_expr(object)?;
                let index = self.eval_expr(index)?;
                self.index_value(&object, &index)
            }
            Expr::Arrow { params, body, .. } => {
                for param in params {
                    reject_reserved_pattern(param)?;
                }
                Ok(Value::Function(Arc::new(FunctionValue::user(
                    None,
                    params.clone(),
                    body.clone(),
                    self.snapshot(),
                ))))
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                if self.eval_expr(condition)?.truthy() {
                    self.eval_expr(then_expr)
                } else {
                    self.eval_expr(else_expr)
                }
            }
            Expr::Capture { pipeline, .. } => {
                let completion = self.run_pipeline(pipeline, true)?;
                if let Some(failure) = completion.failure {
                    Err(Unwind::Throw(failure))
                } else {
                    Ok(completion.value)
                }
            }
            Expr::Missing(_) | Expr::Error(_) => {
                Err(type_error("cannot evaluate a recovered expression"))
            }
        }
    }

    fn eval_call(&mut self, callee: &Expr, args: &[CallArg]) -> EvalResult<Value> {
        if let Expr::Member { object, name, .. } = callee {
            let receiver = self.eval_expr(object)?;
            let args = self.eval_call_args(args)?;
            if let Some(result) = self.call_builtin_method(receiver.clone(), name, args.clone()) {
                return result;
            }
            if let Some(Value::Function(function)) = self.resolve_lexical(name) {
                let mut ufcs_args = Vec::with_capacity(args.len() + 1);
                ufcs_args.push(receiver);
                ufcs_args.extend(args);
                return self.call_function(function, ufcs_args);
            }
            let member = self.member_value(&receiver, name)?;
            return self.call_value(member, args);
        }
        let function = self.eval_expr(callee)?;
        let args = self.eval_call_args(args)?;
        self.call_value(function, args)
    }

    fn eval_call_args(&mut self, args: &[CallArg]) -> EvalResult<Vec<Value>> {
        let mut values = Vec::new();
        for arg in args {
            match arg {
                CallArg::Value(expr) => values.push(self.eval_expr(expr)?),
                CallArg::Spread(expr, _) => {
                    let Value::Array(spread) = self.eval_expr(expr)? else {
                        return Err(type_error("call spread requires an array"));
                    };
                    values.extend(spread.iter().cloned());
                }
            }
        }
        Ok(values)
    }

    fn call_value(&mut self, value: Value, args: Vec<Value>) -> EvalResult<Value> {
        let Value::Function(function) = value else {
            return Err(type_error(format!(
                "{} value is not callable",
                value.type_name()
            )));
        };
        self.call_function(function, args)
    }

    fn call_function(
        &mut self,
        function: Arc<FunctionValue>,
        args: Vec<Value>,
    ) -> EvalResult<Value> {
        match &function.kind {
            FunctionKind::Builtin(builtin) => self.call_builtin_function(*builtin, args),
            FunctionKind::User {
                name,
                params,
                body,
                captures,
            } => {
                let mut frame = (**captures).clone();
                if let Some(name) = name {
                    frame.insert(name.to_string(), Value::Function(Arc::clone(&function)));
                }
                self.frames.push(frame);
                let binding_result = params.iter().enumerate().try_for_each(|(index, pattern)| {
                    self.bind_pattern(pattern, args.get(index).cloned().unwrap_or(Value::Null))
                });
                let result = binding_result.and_then(|()| match &**body {
                    FunctionBody::Expression(expr) => self.eval_expr(expr),
                    FunctionBody::Block(program) => self.eval_program(program),
                });
                self.frames.pop();
                match result {
                    Err(Unwind::Return(value)) => Ok(value),
                    Err(Unwind::Break) => Err(type_error("break cannot cross a function boundary")),
                    Err(Unwind::Continue) => {
                        Err(type_error("continue cannot cross a function boundary"))
                    }
                    result => result,
                }
            }
        }
    }

    fn call_builtin_function(
        &mut self,
        builtin: BuiltinFunction,
        args: Vec<Value>,
    ) -> EvalResult<Value> {
        expect_arity(builtin.name(), &args, 1, 1)?;
        let value = &args[0];
        match builtin {
            BuiltinFunction::String => {
                scalar_to_string(value).map(|value| Value::String(value.into()))
            }
            BuiltinFunction::Int => convert_int(value).map(Value::Int),
            BuiltinFunction::Float => convert_float(value).map(Value::Float),
            BuiltinFunction::Bool => Ok(Value::Bool(value.truthy())),
            BuiltinFunction::Error => scalar_to_string(value)
                .map(|message| Value::Error(Arc::new(ErrorValue::new("user", message)))),
            BuiltinFunction::Glob => {
                let pattern = expect_string(value)?;
                let matches = self
                    .host
                    .glob(pattern.as_bytes(), &self.context)
                    .map_err(EngineError::from)?;
                Ok(Value::Array(Arc::new(
                    matches.into_iter().map(bytes_to_value).collect(),
                )))
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn call_builtin_method(
        &mut self,
        receiver: Value,
        name: &str,
        args: Vec<Value>,
    ) -> Option<EvalResult<Value>> {
        let result = match receiver {
            Value::String(value)
                if matches!(
                    name,
                    "length"
                        | "contains"
                        | "includes"
                        | "startsWith"
                        | "endsWith"
                        | "split"
                        | "replace"
                        | "replaceAll"
                        | "trim"
                        | "toUpperCase"
                        | "toLowerCase"
                        | "at"
                ) =>
            {
                self.string_method(&value, name, args)
            }
            Value::Array(value)
                if matches!(
                    name,
                    "length"
                        | "at"
                        | "contains"
                        | "includes"
                        | "map"
                        | "filter"
                        | "reduce"
                        | "flat"
                        | "join"
                        | "slice"
                ) =>
            {
                self.array_method(&value, name, args)
            }
            Value::Object(value) if matches!(name, "keys" | "entries") => {
                self.object_method(&value, name, args)
            }
            _ => return None,
        };
        Some(result)
    }

    fn string_method(&mut self, value: &str, name: &str, args: Vec<Value>) -> EvalResult<Value> {
        match name {
            "length" => {
                expect_arity(name, &args, 0, 0)?;
                usize_value(value.chars().count())
            }
            "contains" | "includes" => {
                expect_arity(name, &args, 1, 1)?;
                Ok(Value::Bool(value.contains(expect_string(&args[0])?)))
            }
            "startsWith" => {
                expect_arity(name, &args, 1, 1)?;
                Ok(Value::Bool(value.starts_with(expect_string(&args[0])?)))
            }
            "endsWith" => {
                expect_arity(name, &args, 1, 1)?;
                Ok(Value::Bool(value.ends_with(expect_string(&args[0])?)))
            }
            "split" => {
                expect_arity(name, &args, 1, 1)?;
                let separator = expect_string(&args[0])?;
                let parts = if separator.is_empty() {
                    value.chars().map(|ch| ch.to_string()).collect::<Vec<_>>()
                } else {
                    value.split(separator).map(str::to_owned).collect()
                };
                Ok(Value::Array(Arc::new(
                    parts
                        .into_iter()
                        .map(|part| Value::String(Arc::from(part)))
                        .collect(),
                )))
            }
            "replace" | "replaceAll" => {
                expect_arity(name, &args, 2, 2)?;
                let from = expect_string(&args[0])?;
                let to = expect_string(&args[1])?;
                let output = if name == "replace" {
                    value.replacen(from, to, 1)
                } else {
                    value.replace(from, to)
                };
                Ok(Value::String(Arc::from(output)))
            }
            "trim" => {
                expect_arity(name, &args, 0, 0)?;
                Ok(Value::String(Arc::from(value.trim())))
            }
            "toUpperCase" => {
                expect_arity(name, &args, 0, 0)?;
                Ok(Value::String(Arc::from(value.to_uppercase())))
            }
            "toLowerCase" => {
                expect_arity(name, &args, 0, 0)?;
                Ok(Value::String(Arc::from(value.to_lowercase())))
            }
            "at" => {
                expect_arity(name, &args, 1, 1)?;
                let index = expect_int(&args[0])?;
                Ok(string_at(value, index))
            }
            _ => Err(type_error(format!("string has no method `{name}`"))),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn array_method(&mut self, value: &[Value], name: &str, args: Vec<Value>) -> EvalResult<Value> {
        match name {
            "length" => {
                expect_arity(name, &args, 0, 0)?;
                usize_value(value.len())
            }
            "at" => {
                expect_arity(name, &args, 1, 1)?;
                Ok(sequence_at(value, expect_int(&args[0])?)
                    .cloned()
                    .unwrap_or(Value::Null))
            }
            "contains" | "includes" => {
                expect_arity(name, &args, 1, 1)?;
                Ok(Value::Bool(value.contains(&args[0])))
            }
            "map" | "filter" => {
                expect_arity(name, &args, 1, 1)?;
                let function = args[0].clone();
                let whole = Value::Array(Arc::new(value.to_vec()));
                let mut output = Vec::new();
                for (index, item) in value.iter().enumerate() {
                    let mapped = self.call_value(
                        function.clone(),
                        vec![item.clone(), usize_value(index)?, whole.clone()],
                    )?;
                    if name == "map" {
                        output.push(mapped);
                    } else if mapped.truthy() {
                        output.push(item.clone());
                    }
                }
                Ok(Value::Array(Arc::new(output)))
            }
            "reduce" => {
                expect_arity(name, &args, 1, 2)?;
                let function = args[0].clone();
                let (mut accumulator, start) = if let Some(initial) = args.get(1) {
                    (initial.clone(), 0)
                } else {
                    let Some(first) = value.first() else {
                        return Err(type_error(
                            "reduce on an empty array requires an initial value",
                        ));
                    };
                    (first.clone(), 1)
                };
                let whole = Value::Array(Arc::new(value.to_vec()));
                for (index, item) in value.iter().enumerate().skip(start) {
                    accumulator = self.call_value(
                        function.clone(),
                        vec![
                            accumulator,
                            item.clone(),
                            usize_value(index)?,
                            whole.clone(),
                        ],
                    )?;
                }
                Ok(accumulator)
            }
            "flat" => {
                expect_arity(name, &args, 0, 1)?;
                let depth = args.first().map_or(Ok(1), expect_int)?;
                if depth < 0 {
                    return Err(type_error("flat depth must be nonnegative"));
                }
                let mut output = Vec::new();
                flatten(value, depth, &mut output);
                Ok(Value::Array(Arc::new(output)))
            }
            "join" => {
                expect_arity(name, &args, 0, 1)?;
                let separator = args.first().map_or(Ok(","), expect_string)?;
                let text = value
                    .iter()
                    .map(scalar_to_string)
                    .collect::<EvalResult<Vec<_>>>()?
                    .join(separator);
                Ok(Value::String(Arc::from(text)))
            }
            "slice" => {
                expect_arity(name, &args, 0, 2)?;
                let len = i64::try_from(value.len())
                    .map_err(|_| type_error("array is too large to index"))?;
                let start = args.first().map_or(Ok(0), expect_int)?;
                let end = args.get(1).map_or(Ok(len), expect_int)?;
                let start = normalize_slice_index(start, len);
                let end = normalize_slice_index(end, len).max(start);
                Ok(Value::Array(Arc::new(
                    value[start as usize..end as usize].to_vec(),
                )))
            }
            _ => Err(type_error(format!("array has no method `{name}`"))),
        }
    }

    fn object_method(
        &self,
        value: &ObjectValue,
        name: &str,
        args: Vec<Value>,
    ) -> EvalResult<Value> {
        expect_arity(name, &args, 0, 0)?;
        match name {
            "keys" => Ok(Value::Array(Arc::new(
                value
                    .iter()
                    .map(|(key, _)| Value::String(Arc::clone(key)))
                    .collect(),
            ))),
            "entries" => Ok(Value::Array(Arc::new(
                value
                    .iter()
                    .map(|(key, value)| {
                        Value::Array(Arc::new(vec![
                            Value::String(Arc::clone(key)),
                            value.clone(),
                        ]))
                    })
                    .collect(),
            ))),
            _ => Err(type_error(format!("object has no method `{name}`"))),
        }
    }

    fn member_value(&self, value: &Value, name: &str) -> EvalResult<Value> {
        match value {
            Value::Environment => Ok(self.environment_value(name)),
            Value::Object(object) => Ok(object.get(name).cloned().unwrap_or(Value::Null)),
            Value::Array(values) if name == "length" => usize_value(values.len()),
            Value::String(value) if name == "length" => usize_value(value.chars().count()),
            Value::Bytes(value) if name == "length" => usize_value(value.len()),
            Value::Status(status) => match name {
                "success" => Ok(Value::Bool(status.success())),
                "code" => Ok(Value::Int(i64::from(status.code()))),
                "outcomes" => Ok(status_outcomes_value(status)),
                _ => Ok(Value::Null),
            },
            Value::Error(error) => match name {
                "kind" => Ok(Value::String(Arc::from(error.kind()))),
                "message" => Ok(Value::String(Arc::from(error.message()))),
                "status" => Ok(error.status().map_or(Value::Null, |status| {
                    Value::Status(Arc::new(status.clone()))
                })),
                _ => Ok(Value::Null),
            },
            _ => Err(type_error(format!(
                "{} value has no member `{name}`",
                value.type_name()
            ))),
        }
    }

    fn index_value(&self, value: &Value, index: &Value) -> EvalResult<Value> {
        match (value, index) {
            (Value::Array(values), Value::Int(index)) => {
                Ok(sequence_at(values, *index).cloned().unwrap_or(Value::Null))
            }
            (Value::String(value), Value::Int(index)) => Ok(string_at(value, *index)),
            (Value::Bytes(value), Value::Int(index)) => {
                Ok(sequence_at(value, *index)
                    .map_or(Value::Null, |byte| Value::Int(i64::from(*byte))))
            }
            (Value::Environment, Value::String(key)) => Ok(self.environment_value(key)),
            (Value::Environment, key) => Err(type_error(format!(
                "environment variable name must be a string, got {}",
                key.type_name()
            ))),
            (Value::Object(object), Value::String(key)) => {
                Ok(object.get(key).cloned().unwrap_or(Value::Null))
            }
            (Value::Object(object), Value::Int(index)) => Ok(object
                .get(&index.to_string())
                .cloned()
                .unwrap_or(Value::Null)),
            _ => Err(type_error(format!(
                "cannot index {} with {}",
                value.type_name(),
                index.type_name()
            ))),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn apply_binary(&self, left: Value, op: BinaryOp, right: Value) -> EvalResult<Value> {
        match (left, op, right) {
            (Value::Int(left), BinaryOp::Add, Value::Int(right)) => left
                .checked_add(right)
                .map(Value::Int)
                .ok_or_else(|| type_error("integer addition overflow")),
            (Value::Int(left), BinaryOp::Subtract, Value::Int(right)) => left
                .checked_sub(right)
                .map(Value::Int)
                .ok_or_else(|| type_error("integer subtraction overflow")),
            (Value::Int(left), BinaryOp::Multiply, Value::Int(right)) => left
                .checked_mul(right)
                .map(Value::Int)
                .ok_or_else(|| type_error("integer multiplication overflow")),
            (Value::Int(left), BinaryOp::Divide, Value::Int(right)) => {
                Ok(Value::Float(left as f64 / right as f64))
            }
            (Value::Int(_), BinaryOp::IntegerDivide, Value::Int(0)) => {
                Err(type_error("integer division by zero"))
            }
            (Value::Int(left), BinaryOp::IntegerDivide, Value::Int(right)) => left
                .checked_div(right)
                .map(Value::Int)
                .ok_or_else(|| type_error("integer division overflow")),
            (Value::Int(_), BinaryOp::Remainder, Value::Int(0)) => {
                Err(type_error("integer remainder by zero"))
            }
            (Value::Int(left), BinaryOp::Remainder, Value::Int(right)) => left
                .checked_rem(right)
                .map(Value::Int)
                .ok_or_else(|| type_error("integer remainder overflow")),
            (Value::Float(left), BinaryOp::Add, Value::Float(right)) => {
                Ok(Value::Float(left + right))
            }
            (Value::Float(left), BinaryOp::Subtract, Value::Float(right)) => {
                Ok(Value::Float(left - right))
            }
            (Value::Float(left), BinaryOp::Multiply, Value::Float(right)) => {
                Ok(Value::Float(left * right))
            }
            (Value::Float(left), BinaryOp::Divide, Value::Float(right)) => {
                Ok(Value::Float(left / right))
            }
            (Value::String(left), BinaryOp::Add, Value::String(right)) => {
                Ok(Value::String(Arc::from(format!("{left}{right}"))))
            }
            (left, BinaryOp::Equal, right) => Ok(Value::Bool(left == right)),
            (left, BinaryOp::NotEqual, right) => Ok(Value::Bool(left != right)),
            (Value::Int(left), BinaryOp::Less, Value::Int(right)) => Ok(Value::Bool(left < right)),
            (Value::Int(left), BinaryOp::LessEq, Value::Int(right)) => {
                Ok(Value::Bool(left <= right))
            }
            (Value::Int(left), BinaryOp::Greater, Value::Int(right)) => {
                Ok(Value::Bool(left > right))
            }
            (Value::Int(left), BinaryOp::GreaterEq, Value::Int(right)) => {
                Ok(Value::Bool(left >= right))
            }
            (Value::Float(left), BinaryOp::Less, Value::Float(right)) => {
                Ok(Value::Bool(left < right))
            }
            (Value::Float(left), BinaryOp::LessEq, Value::Float(right)) => {
                Ok(Value::Bool(left <= right))
            }
            (Value::Float(left), BinaryOp::Greater, Value::Float(right)) => {
                Ok(Value::Bool(left > right))
            }
            (Value::Float(left), BinaryOp::GreaterEq, Value::Float(right)) => {
                Ok(Value::Bool(left >= right))
            }
            (Value::String(left), BinaryOp::Less, Value::String(right)) => {
                Ok(Value::Bool(left < right))
            }
            (Value::String(left), BinaryOp::LessEq, Value::String(right)) => {
                Ok(Value::Bool(left <= right))
            }
            (Value::String(left), BinaryOp::Greater, Value::String(right)) => {
                Ok(Value::Bool(left > right))
            }
            (Value::String(left), BinaryOp::GreaterEq, Value::String(right)) => {
                Ok(Value::Bool(left >= right))
            }
            (left, BinaryOp::And, right) => Ok(if left.truthy() { right } else { left }),
            (left, BinaryOp::Or, right) => Ok(if left.truthy() { left } else { right }),
            (_, op, _) => Err(type_error(format!(
                "operator {op:?} does not accept these values"
            ))),
        }
    }

    fn bind_pattern(&mut self, pattern: &BindingPattern, value: Value) -> EvalResult<()> {
        reject_reserved_pattern(pattern)?;
        let mut bindings = Vec::new();
        collect_bindings(pattern, value, &mut bindings)?;
        self.current_frame().extend(bindings);
        Ok(())
    }

    fn resolve_expression(&self, name: &str) -> Option<Value> {
        if name == "env" {
            Some(Value::Environment)
        } else {
            self.resolve_lexical(name).or_else(|| builtin_value(name))
        }
    }

    fn resolve_lexical(&self, name: &str) -> Option<Value> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| frame.get(name).cloned())
    }

    fn resolve_variable(&self, name: &str) -> EvalResult<Value> {
        if name == "env" {
            return Ok(Value::Environment);
        }
        self.resolve_lexical(name)
            .or_else(|| {
                self.context
                    .environment_variable(OsStr::new(name))
                    .map(environment_scalar_value)
            })
            .ok_or_else(|| undefined(name))
    }

    fn environment_value(&self, name: &str) -> Value {
        let Some(value) = self.context.environment_variable(OsStr::new(name)) else {
            return Value::Null;
        };
        if name == "PATH" {
            Value::Array(Arc::new(
                env::split_paths(&value)
                    .map(|path| environment_scalar_value(path.into_os_string()))
                    .collect(),
            ))
        } else {
            environment_scalar_value(value)
        }
    }

    fn assign(&mut self, name: &str, value: Value) {
        if let Some(frame) = self
            .frames
            .iter_mut()
            .rev()
            .find(|frame| frame.contains_key(name))
        {
            frame.insert(name.to_owned(), value);
        } else {
            self.current_frame().insert(name.to_owned(), value);
        }
    }

    fn snapshot(&self) -> Frame {
        let mut snapshot = Frame::new();
        for frame in &self.frames {
            snapshot.extend(frame.clone());
        }
        snapshot
    }

    fn current_frame(&mut self) -> &mut Frame {
        self.frames
            .last_mut()
            .expect("an engine always has a global frame")
    }
}

fn plain_command_word(word: &CommandWord) -> Option<String> {
    let mut value = String::new();
    for part in &word.parts {
        let WordPart::Literal { value: part, .. } = part else {
            return None;
        };
        value.push_str(part);
    }
    Some(value)
}

fn parse_chunks_name(name: &str) -> Option<usize> {
    let value = name.strip_prefix("chunks(")?.strip_suffix(')')?;
    let size = value.parse().ok()?;
    (size > 0).then_some(size)
}

fn validate_chunk_size(stage: usize, size: usize) -> EvalResult<usize> {
    if size <= MAX_CHUNK_SIZE {
        Ok(size)
    } else {
        Err(type_error(format!(
            "pipeline stage {stage} chunks size {size} exceeds the {MAX_CHUNK_SIZE}-byte limit"
        )))
    }
}

fn expect_stream_arity(
    stage: usize,
    name: &str,
    words: &[CommandWord],
    expected: usize,
) -> EvalResult<()> {
    if words.len() == expected {
        Ok(())
    } else {
        Err(type_error(format!(
            "pipeline stage {stage} {name} expects {} operand{}",
            expected.saturating_sub(1),
            if expected == 2 { "" } else { "s" }
        )))
    }
}

#[derive(Clone, Copy)]
enum StreamShape {
    External,
    Text,
    BytesToValues,
    Function,
    Values,
}

fn stream_shape(stage: &StreamStage) -> StreamShape {
    match stage {
        StreamStage::External(_) => StreamShape::External,
        StreamStage::Text => StreamShape::Text,
        StreamStage::Json
        | StreamStage::Lines
        | StreamStage::JsonLines
        | StreamStage::Chunks(_) => StreamShape::BytesToValues,
        StreamStage::Function(_) | StreamStage::Map(_) => StreamShape::Function,
        StreamStage::Filter(_)
        | StreamStage::Take(_)
        | StreamStage::First
        | StreamStage::Collect => StreamShape::Values,
    }
}

fn validate_stream_shapes(shapes: impl IntoIterator<Item = StreamShape>) -> EvalResult<()> {
    let mut port = None;
    for (index, shape) in shapes.into_iter().enumerate() {
        port = Some(match shape {
            StreamShape::External => StreamPort::Bytes,
            StreamShape::Text => match port {
                Some(StreamPort::Bytes) => StreamPort::Values,
                Some(StreamPort::Values) => StreamPort::Bytes,
                None => {
                    return Err(type_error(format!(
                        "pipeline stage {index} text requires a byte or value input"
                    )));
                }
            },
            StreamShape::BytesToValues => {
                if port != Some(StreamPort::Bytes) {
                    return Err(type_error(format!(
                        "pipeline stage {index} requires a byte stream"
                    )));
                }
                StreamPort::Values
            }
            StreamShape::Function => {
                if port == Some(StreamPort::Bytes) {
                    return Err(type_error(format!(
                        "pipeline stage {index} cannot apply a function to bytes; add `lines`, `jsonl`, `json`, `text`, or `chunks(n)` first"
                    )));
                }
                if port != Some(StreamPort::Values) {
                    return Err(type_error(format!(
                        "pipeline stage {index} function requires a value stream"
                    )));
                }
                StreamPort::Values
            }
            StreamShape::Values => {
                if port != Some(StreamPort::Values) {
                    return Err(type_error(format!(
                        "pipeline stage {index} requires a value stream"
                    )));
                }
                StreamPort::Values
            }
        });
    }
    Ok(())
}

fn completion_from_result(result: ExecutionResult) -> Completion {
    let value = match result.captured {
        Some(Captured::String(value)) => Value::String(value),
        Some(Captured::Bytes(value)) => Value::Bytes(value),
        Some(Captured::Value(value)) => value,
        None => Value::Null,
    };
    Completion {
        value,
        status: StatusValue::new(result.outcomes),
        failure: None,
    }
}

fn status_from_completed_error(error: &ExecutionError) -> Option<StatusValue> {
    match error {
        ExecutionError::CommandFailed { outcomes, .. }
        | ExecutionError::PipelineFailed { outcomes, .. } => {
            Some(StatusValue::new(outcomes.clone()))
        }
        _ => None,
    }
}

fn status_outcomes_value(status: &StatusValue) -> Value {
    Value::Array(Arc::new(
        status
            .outcomes()
            .iter()
            .map(|outcome| {
                Value::Object(Arc::new(ObjectValue::from_entries([
                    (Arc::from("stage"), Value::Int(outcome.stage as i64)),
                    (
                        Arc::from("command"),
                        Value::String(Arc::from(outcome.rendered.as_str())),
                    ),
                    (
                        Arc::from("code"),
                        outcome
                            .code
                            .map_or(Value::Null, |code| Value::Int(i64::from(code))),
                    ),
                    (
                        Arc::from("signal"),
                        outcome
                            .signal
                            .map_or(Value::Null, |signal| Value::Int(i64::from(signal))),
                    ),
                    (Arc::from("success"), Value::Bool(outcome.success)),
                ])))
            })
            .collect(),
    ))
}

fn builtin_value(name: &str) -> Option<Value> {
    let builtin = match name {
        "string" => BuiltinFunction::String,
        "int" => BuiltinFunction::Int,
        "float" => BuiltinFunction::Float,
        "bool" => BuiltinFunction::Bool,
        "error" => BuiltinFunction::Error,
        "glob" => BuiltinFunction::Glob,
        _ => return None,
    };
    Some(Value::Function(Arc::new(FunctionValue::builtin(builtin))))
}

fn reject_reserved_name(name: &str) -> EvalResult<()> {
    if name == "env" {
        Err(type_error("`env` is a reserved runtime namespace"))
    } else {
        Ok(())
    }
}

fn reject_reserved_pattern(pattern: &BindingPattern) -> EvalResult<()> {
    match pattern {
        BindingPattern::Name { name, .. } => reject_reserved_name(name),
        BindingPattern::Array { items, rest, .. } => {
            for item in items {
                reject_reserved_pattern(item)?;
            }
            if let Some(rest) = rest {
                reject_reserved_pattern(rest)?;
            }
            Ok(())
        }
        BindingPattern::Object { entries, rest, .. } => {
            for (_, pattern) in entries {
                reject_reserved_pattern(pattern)?;
            }
            if let Some(rest) = rest {
                reject_reserved_pattern(rest)?;
            }
            Ok(())
        }
        BindingPattern::Missing { .. } => Ok(()),
    }
}

fn environment_assignment_value(name: &str, value: &Value) -> EvalResult<Option<OsString>> {
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    if name == "PATH"
        && let Value::Array(components) = value
    {
        let paths = components
            .iter()
            .map(|component| match component {
                Value::String(_) | Value::Bytes(_) => {
                    environment_scalar_os_string(component).map(PathBuf::from)
                }
                _ => Err(ShellContextError::InvalidEnvironmentValue {
                    name: name.into(),
                    message: format!(
                        "PATH components must be String or Bytes, got {}",
                        component.type_name()
                    ),
                }
                .into()),
            })
            .collect::<EvalResult<Vec<_>>>()?;
        return env::join_paths(paths).map(Some).map_err(|error| {
            ShellContextError::InvalidEnvironmentValue {
                name: name.into(),
                message: format!("PATH component cannot be joined: {error}"),
            }
            .into()
        });
    }
    if matches!(value, Value::Array(_)) {
        return Err(type_error(
            "arrays can only be assigned to env.PATH; use a scalar environment value",
        ));
    }
    environment_scalar_os_string(value).map(Some)
}

fn environment_scalar_os_string(value: &Value) -> EvalResult<OsString> {
    match value {
        Value::Bool(value) => Ok(value.to_string().into()),
        Value::Int(value) => Ok(value.to_string().into()),
        Value::Float(value) => Ok(value.to_string().into()),
        Value::String(value) => Ok(OsString::from(value.as_ref())),
        Value::Bytes(value) => os_string_from_bytes(value.to_vec()).map_err(type_error),
        _ => Err(type_error(format!(
            "{} cannot be exported; expected String, Bytes, Int, Float, Bool, or null",
            value.type_name()
        ))),
    }
}

fn environment_scalar_value(value: OsString) -> Value {
    match value.into_string() {
        Ok(value) => Value::String(Arc::from(value)),
        Err(value) => {
            os_string_bytes(value).map_or(Value::Null, |value| Value::Bytes(Arc::from(value)))
        }
    }
}

#[cfg(unix)]
fn os_string_from_bytes(value: Vec<u8>) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(value))
}

#[cfg(not(unix))]
fn os_string_from_bytes(value: Vec<u8>) -> Result<OsString, String> {
    String::from_utf8(value)
        .map(OsString::from)
        .map_err(|error| error.to_string())
}

#[cfg(unix)]
fn os_string_bytes(value: OsString) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStringExt;
    Some(value.into_vec())
}

#[cfg(not(unix))]
fn os_string_bytes(value: OsString) -> Option<Vec<u8>> {
    value.into_string().ok().map(String::into_bytes)
}

fn collect_bindings(
    pattern: &BindingPattern,
    value: Value,
    output: &mut Vec<(String, Value)>,
) -> EvalResult<()> {
    match pattern {
        BindingPattern::Name { name, .. } => output.push((name.clone(), value)),
        BindingPattern::Array { items, rest, .. } => {
            let Value::Array(values) = value else {
                return Err(type_error("array pattern requires an array"));
            };
            for (index, pattern) in items.iter().enumerate() {
                collect_bindings(
                    pattern,
                    values.get(index).cloned().unwrap_or(Value::Null),
                    output,
                )?;
            }
            if let Some(pattern) = rest {
                collect_bindings(
                    pattern,
                    Value::Array(Arc::new(values[items.len().min(values.len())..].to_vec())),
                    output,
                )?;
            }
        }
        BindingPattern::Object { entries, rest, .. } => {
            let Value::Object(object) = value else {
                return Err(type_error("object pattern requires an object"));
            };
            for (key, pattern) in entries {
                collect_bindings(
                    pattern,
                    object.get(key).cloned().unwrap_or(Value::Null),
                    output,
                )?;
            }
            if let Some(pattern) = rest {
                let remainder = ObjectValue::from_entries(
                    object
                        .iter()
                        .filter(|(key, _)| {
                            !entries
                                .iter()
                                .any(|(used, _)| used.as_str() == key.as_ref())
                        })
                        .map(|(key, value)| (Arc::clone(key), value.clone())),
                );
                collect_bindings(pattern, Value::Object(Arc::new(remainder)), output)?;
            }
        }
        BindingPattern::Missing { .. } => {
            return Err(type_error("cannot bind a recovered pattern"));
        }
    }
    Ok(())
}

fn contains_glob_meta(value: &[u8]) -> bool {
    value.iter().any(|byte| matches!(byte, b'*' | b'?' | b'['))
}

fn append_glob_literal(output: &mut Vec<u8>, value: &[u8]) {
    for byte in value {
        match byte {
            b'*' => output.extend_from_slice(b"[*]"),
            b'?' => output.extend_from_slice(b"[?]"),
            b'[' => output.extend_from_slice(b"[[]"),
            b']' => output.extend_from_slice(b"[]]"),
            byte => output.push(*byte),
        }
    }
}

fn bytes_to_value(bytes: Vec<u8>) -> Value {
    match String::from_utf8(bytes) {
        Ok(text) => Value::String(Arc::from(text)),
        Err(error) => Value::Bytes(Arc::from(error.into_bytes())),
    }
}

fn value_to_bytes(value: &Value) -> EvalResult<Vec<u8>> {
    match value {
        Value::Null => Ok(b"null".to_vec()),
        Value::Bool(value) => Ok(value.to_string().into_bytes()),
        Value::Int(value) => Ok(value.to_string().into_bytes()),
        Value::Float(value) => Ok(value.to_string().into_bytes()),
        Value::String(value) => Ok(value.as_bytes().to_vec()),
        Value::Bytes(value) => Ok(value.to_vec()),
        Value::Array(_) => Err(type_error(
            "an array expands only as a sole unquoted `$variable` command word",
        )),
        Value::Object(_) => Err(type_error("objects require explicit string conversion")),
        Value::Environment => Err(type_error("the env namespace is not a command argument")),
        Value::Function(_) => Err(type_error("functions cannot be command arguments")),
        Value::Error(_) => Err(type_error("errors require explicit string conversion")),
        Value::Status(_) => Err(type_error("statuses require explicit member access")),
    }
}

fn scalar_to_string(value: &Value) -> EvalResult<String> {
    match value {
        Value::Null => Ok("null".into()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(value.to_string()),
        Value::String(value) => Ok(value.to_string()),
        Value::Bytes(value) => {
            String::from_utf8(value.to_vec()).map_err(|_| type_error("bytes are not valid UTF-8"))
        }
        _ => Err(type_error(format!(
            "{} is not a scalar string conversion input",
            value.type_name()
        ))),
    }
}

fn convert_int(value: &Value) -> EvalResult<i64> {
    match value {
        Value::Int(value) => Ok(*value),
        Value::Bool(value) => Ok(i64::from(*value)),
        Value::Float(value)
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= i64::MIN as f64
                && *value < -(i64::MIN as f64) =>
        {
            Ok(*value as i64)
        }
        Value::String(value) => value
            .parse()
            .map_err(|_| type_error("string is not a base-10 integer")),
        _ => Err(type_error(format!(
            "cannot convert {} to int",
            value.type_name()
        ))),
    }
}

fn convert_float(value: &Value) -> EvalResult<f64> {
    match value {
        Value::Float(value) => Ok(*value),
        Value::Int(value) => Ok(*value as f64),
        Value::Bool(value) => Ok(if *value { 1.0 } else { 0.0 }),
        Value::String(value) => value
            .parse()
            .map_err(|_| type_error("string is not a float")),
        _ => Err(type_error(format!(
            "cannot convert {} to float",
            value.type_name()
        ))),
    }
}

fn expect_arity(name: &str, args: &[Value], minimum: usize, maximum: usize) -> EvalResult<()> {
    if (minimum..=maximum).contains(&args.len()) {
        Ok(())
    } else if minimum == maximum {
        Err(type_error(format!(
            "{name} expects {minimum} argument(s), got {}",
            args.len()
        )))
    } else {
        Err(type_error(format!(
            "{name} expects {minimum}..={maximum} arguments, got {}",
            args.len()
        )))
    }
}

fn expect_string(value: &Value) -> EvalResult<&str> {
    if let Value::String(value) = value {
        Ok(value)
    } else {
        Err(type_error(format!(
            "expected string, got {}",
            value.type_name()
        )))
    }
}

fn expect_int(value: &Value) -> EvalResult<i64> {
    if let Value::Int(value) = value {
        Ok(*value)
    } else {
        Err(type_error(format!(
            "expected int, got {}",
            value.type_name()
        )))
    }
}

fn usize_value(value: usize) -> EvalResult<Value> {
    i64::try_from(value)
        .map(Value::Int)
        .map_err(|_| type_error("value is too large for an integer"))
}

fn sequence_at<T>(values: &[T], index: i64) -> Option<&T> {
    let len = i64::try_from(values.len()).ok()?;
    let index = if index < 0 {
        len.checked_add(index)?
    } else {
        index
    };
    usize::try_from(index)
        .ok()
        .and_then(|index| values.get(index))
}

fn string_at(value: &str, index: i64) -> Value {
    let characters = value.chars().collect::<Vec<_>>();
    sequence_at(&characters, index).map_or(Value::Null, |character| {
        Value::String(Arc::from(character.to_string()))
    })
}

fn normalize_slice_index(index: i64, len: i64) -> i64 {
    if index < 0 {
        len.saturating_add(index).max(0)
    } else {
        index.min(len)
    }
}

fn flatten(values: &[Value], depth: i64, output: &mut Vec<Value>) {
    for value in values {
        if depth > 0
            && let Value::Array(nested) = value
        {
            flatten(nested, depth - 1, output);
        } else {
            output.push(value.clone());
        }
    }
}

fn undefined(name: &str) -> Unwind {
    EngineError::Undefined {
        name: name.to_owned(),
    }
    .into()
}

fn type_error(message: impl Into<String>) -> Unwind {
    EngineError::Type(message.into()).into()
}
