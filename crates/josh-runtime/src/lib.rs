#![forbid(unsafe_code)]

mod context;
mod engine;
mod host;
mod materialize;
mod natives;
mod pretty;
mod value;

pub use context::{ShellContext, ShellContextError, ShellSnapshot};
pub use engine::{Engine, EngineError, MAX_CHUNK_SIZE, ParseFailure, RunResult};
pub use host::{
    CancellationToken, Captured, CommandSpec, ExecutionError, ExecutionHost, ExecutionResult,
    RedirectionSpec, StageOutcome, StreamStage,
};
pub use materialize::{
    BoundedBytes, MAX_MATERIALIZED_BYTES, MAX_MATERIALIZED_ITEMS, MaterializationLimit,
    materialization_limit, read_bounded, value_materialized_bytes, value_materialized_items,
};
pub use pretty::{PrettyOptions, render_value};
pub use value::{ErrorValue, FunctionValue, JoshStr, ObjectValue, StatusValue, Value};
