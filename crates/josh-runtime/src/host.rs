use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use thiserror::Error;

use crate::{
    context::ShellContext,
    materialize::MaterializationLimit,
    value::{FunctionValue, Value},
};

#[derive(Debug, Clone)]
pub struct CancellationToken {
    local: Arc<AtomicBool>,
    ancestors: Arc<Vec<Arc<AtomicBool>>>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::from_flag(Arc::new(AtomicBool::new(false)))
    }
}

impl CancellationToken {
    #[must_use]
    pub fn from_flag(flag: Arc<AtomicBool>) -> Self {
        Self {
            local: flag,
            ancestors: Arc::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn child(&self) -> Self {
        let mut ancestors = Vec::with_capacity(self.ancestors.len() + 1);
        ancestors.extend(self.ancestors.iter().cloned());
        ancestors.push(Arc::clone(&self.local));
        Self {
            local: Arc::new(AtomicBool::new(false)),
            ancestors: Arc::new(ancestors),
        }
    }

    pub fn cancel(&self) {
        self.local.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.local.load(Ordering::Acquire)
            || self
                .ancestors
                .iter()
                .any(|flag| flag.load(Ordering::Acquire))
    }

    #[must_use]
    pub fn local_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.local)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub argv: Vec<Vec<u8>>,
    pub redirections: Vec<RedirectionSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectionSpec {
    Input(Vec<u8>),
    Output(Vec<u8>),
    Append(Vec<u8>),
    Error(Vec<u8>),
    ErrorAppend(Vec<u8>),
    ErrorToOutput,
    OutputAndError(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageOutcome {
    pub stage: usize,
    pub rendered: String,
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Captured {
    String(Arc<str>),
    Bytes(Arc<[u8]>),
    Value(Value),
}

#[derive(Debug, Clone)]
pub enum StreamStage {
    External(CommandSpec),
    /// Pre-materialized items fed into the pipeline by the engine
    /// (`[1, 2, 3] | x => x * 2`). `scalar` marks a non-array source so a
    /// bare capture of it stays a scalar.
    Source {
        values: Vec<Value>,
        scalar: bool,
    },
    Text,
    Json,
    Lines,
    JsonLines,
    Chunks(usize),
    Function(Arc<FunctionValue>),
    Map(Arc<FunctionValue>),
    Filter(Arc<FunctionValue>),
    Take(usize),
    TakeLast(usize),
    First,
    Collect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    pub outcomes: Vec<StageOutcome>,
    pub captured: Option<Captured>,
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("stage {stage}: command not found: {command}")]
    CommandNotFound { stage: usize, command: String },
    #[error("stage {stage}: cannot spawn `{command}`: {source}")]
    Spawn {
        stage: usize,
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("command failed: `{command}` ({description})")]
    CommandFailed {
        command: String,
        description: String,
        outcomes: Vec<StageOutcome>,
    },
    #[error("pipeline failed at stage {stage}: `{command}` ({description})")]
    PipelineFailed {
        stage: usize,
        command: String,
        description: String,
        outcomes: Vec<StageOutcome>,
    },
    #[error("cannot collect command output: {0}")]
    Collect(#[source] io::Error),
    #[error(
        "{boundary} exceeds the {limit} materialization limit; use streaming `filter`/`take` or an external consumer instead of `$()`/`collect`"
    )]
    MaterializationLimit {
        boundary: &'static str,
        limit: MaterializationLimit,
    },
    #[error("stream stage {stage} failed: {message}")]
    Stream { stage: usize, message: String },
    #[error("pipeline was cancelled")]
    Cancelled,
    #[error("cannot control the foreground terminal: {0}")]
    Terminal(#[source] io::Error),
    #[error("foreground pipeline stopped by signal {signal}; suspended jobs are not supported")]
    ForegroundStopped { signal: i32 },
    #[error("command argument cannot be represented on this platform: {0}")]
    InvalidArgument(String),
    #[error("invalid glob pattern `{pattern}`: {message}")]
    InvalidGlob { pattern: String, message: String },
    #[error("glob pattern `{pattern}` matched no paths")]
    GlobNoMatch { pattern: String },
    #[error("cannot open redirection target {path}: {source}")]
    RedirectionOpen {
        path: String,
        #[source]
        source: io::Error,
    },
}

pub trait ExecutionHost {
    fn execute(
        &mut self,
        commands: Vec<CommandSpec>,
        capture: bool,
        cancellation: CancellationToken,
        context: ShellContext,
    ) -> Result<ExecutionResult, ExecutionError>;

    fn execute_stream(
        &mut self,
        stages: Vec<StreamStage>,
        capture: bool,
        cancellation: CancellationToken,
        context: ShellContext,
    ) -> Result<ExecutionResult, ExecutionError>;

    fn glob(&self, pattern: &[u8], context: &ShellContext) -> Result<Vec<Vec<u8>>, ExecutionError>;
}
