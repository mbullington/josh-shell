#![forbid(unsafe_code)]

//! Typed byte/value pipeline execution with bounded in-shell channels.

use std::{
    ffi::OsString,
    fs::File,
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};

use josh_runtime::{
    BoundedBytes, CancellationToken, Captured, EngineError, ExecutionError, ExecutionResult,
    FunctionValue, MAX_CHUNK_SIZE, MAX_MATERIALIZED_BYTES, MAX_MATERIALIZED_ITEMS,
    MaterializationLimit, ObjectValue, ShellContext, ShellSnapshot, StageOutcome, Value,
    materialization_limit, read_bounded, value_materialized_bytes, value_materialized_items,
};
use os_pipe::{PipeReader, PipeWriter};
use serde_json::Value as JsonValue;

pub const VALUE_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy)]
struct MaterializationLimits {
    bytes: usize,
    items: usize,
}

impl MaterializationLimits {
    const DEFAULT: Self = Self {
        bytes: MAX_MATERIALIZED_BYTES,
        items: MAX_MATERIALIZED_ITEMS,
    };
}

#[derive(Debug, Clone)]
pub struct PlannedExternal {
    pub executable: PathBuf,
    pub argv: Vec<OsString>,
    pub rendered: String,
    pub redirections: Vec<PlannedRedirection>,
}

#[derive(Debug, Clone)]
pub enum PlannedRedirection {
    Input(Arc<File>),
    Output(Arc<File>),
    Error(Arc<File>),
    ErrorToOutput,
    OutputAndError(Arc<File>),
}

#[derive(Debug, Clone)]
pub enum PlannedStage {
    External(PlannedExternal),
    Source { values: Vec<Value>, scalar: bool },
    Text,
    Json,
    Lines,
    JsonLines,
    Chunks(usize),
    Function(Arc<FunctionValue>),
    Map(Arc<FunctionValue>),
    Filter(Arc<FunctionValue>),
    Take(usize),
    First,
    Collect,
}

pub type FunctionRunner =
    Arc<dyn Fn(Arc<FunctionValue>, Value) -> Result<Value, EngineError> + Send + Sync>;

enum InputDescriptor {
    Pipe(PipeReader),
    File(Arc<File>),
}

impl InputDescriptor {
    fn into_stdio(self) -> io::Result<Stdio> {
        match self {
            Self::Pipe(reader) => Ok(Stdio::from(reader)),
            Self::File(file) => Ok(Stdio::from(file.try_clone()?)),
        }
    }
}

enum OutputDescriptor {
    Pipe(PipeWriter),
    File(Arc<File>),
}

impl OutputDescriptor {
    fn try_clone(&self) -> io::Result<Self> {
        match self {
            Self::Pipe(writer) => writer.try_clone().map(Self::Pipe),
            Self::File(file) => Ok(Self::File(Arc::clone(file))),
        }
    }

    fn into_stdio(self) -> io::Result<Stdio> {
        match self {
            Self::Pipe(writer) => Ok(Stdio::from(writer)),
            Self::File(file) => Ok(Stdio::from(file.try_clone()?)),
        }
    }
}

pub fn configure_external_stdio(
    process: &mut Command,
    input: Option<PipeReader>,
    output: Option<PipeWriter>,
    redirections: &[PlannedRedirection],
) -> io::Result<()> {
    let mut stdin = InputDescriptor::Pipe(match input {
        Some(reader) => reader,
        None => os_pipe::dup_stdin()?,
    });
    let mut stdout = OutputDescriptor::Pipe(match output {
        Some(writer) => writer,
        None => os_pipe::dup_stdout()?,
    });
    let mut stderr = OutputDescriptor::Pipe(os_pipe::dup_stderr()?);
    for redirection in redirections {
        match redirection {
            PlannedRedirection::Input(file) => stdin = InputDescriptor::File(Arc::clone(file)),
            PlannedRedirection::Output(file) => stdout = OutputDescriptor::File(Arc::clone(file)),
            PlannedRedirection::Error(file) => stderr = OutputDescriptor::File(Arc::clone(file)),
            PlannedRedirection::ErrorToOutput => stderr = stdout.try_clone()?,
            PlannedRedirection::OutputAndError(file) => {
                stdout = OutputDescriptor::File(Arc::clone(file));
                stderr = stdout.try_clone()?;
            }
        }
    }
    process.stdin(stdin.into_stdio()?);
    process.stdout(stdout.into_stdio()?);
    process.stderr(stderr.into_stdio()?);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Port {
    Bytes,
    Values,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cardinality {
    One,
    Many,
}

#[derive(Debug)]
enum Stage {
    External(PlannedExternal),
    Source(Vec<Value>),
    BytesToText,
    ValuesToText,
    Json,
    Lines,
    JsonLines,
    Chunks(usize),
    Function(Arc<FunctionValue>),
    Map(Arc<FunctionValue>),
    Filter(Arc<FunctionValue>),
    Take(usize),
    First,
    Collect,
}

#[derive(Debug)]
struct StreamValue {
    value: Value,
    acknowledgement: Option<mpsc::Sender<()>>,
}

impl std::ops::Deref for StreamValue {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl Drop for StreamValue {
    fn drop(&mut self) {
        if let Some(acknowledgement) = self.acknowledgement.take() {
            let _ = acknowledgement.send(());
        }
    }
}

#[derive(Debug)]
enum Input {
    Bytes(PipeReader),
    Values(Receiver<StreamValue>),
}

#[derive(Debug)]
enum Output {
    Bytes(PipeWriter),
    Values(SyncSender<StreamValue>),
    Inherit,
}

struct Adapter {
    stage: usize,
    input: Receiver<StreamValue>,
    output: PipeWriter,
}

struct WorkerReport {
    stage: usize,
    result: Result<(), WorkerError>,
}

#[derive(Debug)]
enum WorkerError {
    Message(String),
    Execution(ExecutionError),
}

impl From<String> for WorkerError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<ExecutionError> for WorkerError {
    fn from(error: ExecutionError) -> Self {
        Self::Execution(error)
    }
}

pub fn run(
    stages: Vec<PlannedStage>,
    capture: bool,
    run_function: FunctionRunner,
) -> Result<ExecutionResult, ExecutionError> {
    run_with_cancellation(
        stages,
        capture,
        run_function,
        CancellationToken::default(),
        ShellContext::from_process().snapshot(),
    )
}

pub fn run_with_cancellation(
    stages: Vec<PlannedStage>,
    capture: bool,
    run_function: FunctionRunner,
    cancellation: CancellationToken,
    snapshot: ShellSnapshot,
) -> Result<ExecutionResult, ExecutionError> {
    run_with_materialization_limits(
        stages,
        capture,
        run_function,
        cancellation,
        snapshot,
        MaterializationLimits::DEFAULT,
    )
}

fn run_with_materialization_limits(
    stages: Vec<PlannedStage>,
    capture: bool,
    run_function: FunctionRunner,
    cancellation: CancellationToken,
    snapshot: ShellSnapshot,
    limits: MaterializationLimits,
) -> Result<ExecutionResult, ExecutionError> {
    let (stages, ports, cardinality) = resolve(stages)?;
    let count = stages.len();
    let mut inputs = (0..count).map(|_| None).collect::<Vec<Option<Input>>>();
    let mut outputs = (0..count).map(|_| None).collect::<Vec<Option<Output>>>();
    let mut adapters = Vec::new();

    for index in 0..count.saturating_sub(1) {
        match ports[index] {
            Port::Bytes => {
                let (reader, writer) = os_pipe::pipe().map_err(ExecutionError::Collect)?;
                outputs[index] = Some(Output::Bytes(writer));
                inputs[index + 1] = Some(Input::Bytes(reader));
            }
            Port::Values if matches!(stages[index + 1], Stage::External(_)) => {
                let (sender, receiver) = mpsc::sync_channel(VALUE_CHANNEL_CAPACITY);
                let (reader, writer) = os_pipe::pipe().map_err(ExecutionError::Collect)?;
                outputs[index] = Some(Output::Values(sender));
                inputs[index + 1] = Some(Input::Bytes(reader));
                adapters.push(Adapter {
                    stage: index + 1,
                    input: receiver,
                    output: writer,
                });
            }
            Port::Values => {
                let (sender, receiver) = mpsc::sync_channel(VALUE_CHANNEL_CAPACITY);
                outputs[index] = Some(Output::Values(sender));
                inputs[index + 1] = Some(Input::Values(receiver));
            }
        }
    }

    let mut terminal_bytes = None;
    let mut terminal_values = None;
    match ports.last().copied() {
        Some(Port::Bytes) if capture => {
            let (reader, writer) = os_pipe::pipe().map_err(ExecutionError::Collect)?;
            outputs[count - 1] = Some(Output::Bytes(writer));
            terminal_bytes = Some(reader);
        }
        Some(Port::Bytes) => outputs[count - 1] = Some(Output::Inherit),
        Some(Port::Values) => {
            let (sender, receiver) = mpsc::sync_channel(VALUE_CHANNEL_CAPACITY);
            outputs[count - 1] = Some(Output::Values(sender));
            terminal_values = Some(receiver);
        }
        None => {
            return Err(ExecutionError::Stream {
                stage: 0,
                message: "pipeline has no stages".into(),
            });
        }
    }

    let mut children = Vec::<(usize, String, Child)>::new();
    for (index, stage) in stages.iter().enumerate() {
        let Stage::External(command) = stage else {
            continue;
        };
        let mut process = Command::new(&command.executable);
        process
            .args(&command.argv[1..])
            .current_dir(snapshot.cwd())
            .env_clear()
            .envs(snapshot.environment());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            process.process_group(0);
        }
        let input = match inputs[index].take() {
            Some(Input::Bytes(reader)) => Some(reader),
            Some(Input::Values(_)) => unreachable!("value-to-external adapter was omitted"),
            None => None,
        };
        let output = match outputs[index].take().expect("every stage has an output") {
            Output::Bytes(writer) => Some(writer),
            Output::Inherit => None,
            Output::Values(_) => unreachable!("external commands produce bytes"),
        };
        configure_external_stdio(&mut process, input, output, &command.redirections)
            .map_err(ExecutionError::Collect)?;
        match process.spawn() {
            Ok(child) => children.push((index, command.rendered.clone(), child)),
            Err(source) => {
                terminate_and_reap(&mut children);
                return Err(ExecutionError::Spawn {
                    stage: index,
                    command: command.rendered.clone(),
                    source,
                });
            }
        }
    }

    let early = Arc::new(AtomicBool::new(false));
    let mut workers = Vec::new();
    for adapter in adapters {
        let cancellation = cancellation.clone();
        let early = Arc::clone(&early);
        workers.push(thread::spawn(move || {
            let result = write_values_to_external(
                adapter.input,
                adapter.output,
                &cancellation,
                &early,
                limits.bytes,
            );
            if result.is_err() {
                cancellation.cancel();
            }
            WorkerReport {
                stage: adapter.stage,
                result,
            }
        }));
    }
    for (index, stage) in stages.into_iter().enumerate() {
        if matches!(stage, Stage::External(_)) {
            continue;
        }
        let output = outputs[index].take().expect("every stage has an output");
        let cancellation = cancellation.clone();
        let early = Arc::clone(&early);
        if let Stage::Source(values) = stage {
            workers.push(thread::spawn(move || {
                let result = run_source_worker(values, output, &cancellation, &early);
                if result.is_err() {
                    cancellation.cancel();
                }
                WorkerReport {
                    stage: index,
                    result,
                }
            }));
            continue;
        }
        let input = inputs[index].take().expect("in-shell stages require input");
        let run_function = Arc::clone(&run_function);
        workers.push(thread::spawn(move || {
            let result = run_worker(
                stage,
                input,
                output,
                run_function,
                &cancellation,
                &early,
                limits,
            );
            if result.is_err() {
                cancellation.cancel();
            }
            WorkerReport {
                stage: index,
                result,
            }
        }));
    }

    let terminal = if let Some(mut reader) = terminal_bytes {
        let terminal_cancellation = cancellation.clone();
        thread::spawn(move || {
            let result =
                read_bounded(&mut reader, "byte capture", limits.bytes).map(Terminal::Bytes);
            if result.is_err() {
                terminal_cancellation.cancel();
            }
            result
        })
    } else if let Some(receiver) = terminal_values {
        let terminal_cancellation = cancellation.clone();
        thread::spawn(move || {
            let result = capture_terminal_values(receiver, limits).map(Terminal::Values);
            if result.is_err() {
                terminal_cancellation.cancel();
            }
            result
        })
    } else {
        thread::spawn(|| Ok(Terminal::None))
    };

    let mut statuses = vec![None; count];
    let mut killed = vec![false; count];
    let mut stopped = false;
    while !terminal.is_finished() {
        if cancellation.is_cancelled() && !stopped {
            stop_after_grace(&mut children, &mut statuses, &mut killed, &early);
            stopped = true;
        }
        thread::sleep(Duration::from_millis(2));
    }
    let terminal = terminal.join().unwrap_or_else(|_| {
        Err(ExecutionError::Collect(io::Error::other(
            "capture worker panicked",
        )))
    });
    if terminal.is_err() {
        cancellation.cancel();
    }

    if cancellation.is_cancelled() && !stopped {
        stop_after_grace(&mut children, &mut statuses, &mut killed, &early);
        stopped = true;
    }

    let mut worker_error = None;
    for worker in workers {
        match worker.join() {
            Ok(report) => {
                if let Err(error) = report.result
                    && worker_error.is_none()
                {
                    worker_error = Some(match error {
                        WorkerError::Message(message) => ExecutionError::Stream {
                            stage: report.stage,
                            message,
                        },
                        WorkerError::Execution(error) => error,
                    });
                }
            }
            Err(_) if worker_error.is_none() => {
                cancellation.cancel();
                worker_error = Some(ExecutionError::Stream {
                    stage: 0,
                    message: "pipeline worker panicked".into(),
                });
            }
            Err(_) => cancellation.cancel(),
        }
    }

    if cancellation.is_cancelled() && !stopped {
        stop_after_grace(&mut children, &mut statuses, &mut killed, &early);
    }
    let reap_result = reap_all(&mut children, &mut statuses);
    let mut outcomes = statuses
        .into_iter()
        .enumerate()
        .filter_map(|(stage, status)| {
            status.map(|(rendered, status)| outcome(stage, rendered, status))
        })
        .collect::<Vec<_>>();
    outcomes.sort_by_key(|outcome| outcome.stage);

    if early.load(Ordering::Acquire) {
        for outcome in &mut outcomes {
            if outcome.signal == Some(13) || killed[outcome.stage] {
                outcome.success = true;
            }
        }
    }

    let terminal = terminal?;
    if let Some(error) = worker_error {
        return Err(error);
    }
    reap_result?;
    if cancellation.is_cancelled() && !early.load(Ordering::Acquire) {
        return Err(ExecutionError::Cancelled);
    }
    if let Some(failed) = outcomes.iter().find(|outcome| !outcome.success) {
        let description = status_description(failed);
        if outcomes.len() == 1 {
            return Err(ExecutionError::CommandFailed {
                command: failed.rendered.clone(),
                description,
                outcomes,
            });
        }
        return Err(ExecutionError::PipelineFailed {
            stage: failed.stage,
            command: failed.rendered.clone(),
            description,
            outcomes,
        });
    }

    let captured = match terminal {
        Terminal::None => None,
        Terminal::Bytes(bytes) => Some(decode_capture(bytes)),
        Terminal::Values(values) => Some(Captured::Value(capture_values(values, cardinality)?)),
    };
    Ok(ExecutionResult { outcomes, captured })
}

fn resolve(
    stages: Vec<PlannedStage>,
) -> Result<(Vec<Stage>, Vec<Port>, Cardinality), ExecutionError> {
    let mut port = None;
    let mut cardinality = Cardinality::Many;
    let mut resolved = Vec::with_capacity(stages.len());
    let mut ports = Vec::with_capacity(stages.len());
    for (index, stage) in stages.into_iter().enumerate() {
        let (stage, output) = match stage {
            PlannedStage::External(command) => (Stage::External(command), Port::Bytes),
            PlannedStage::Source { values, scalar } => {
                if port.is_some() {
                    return Err(transition_error(
                        index,
                        "source must be the first pipeline stage",
                    ));
                }
                if scalar {
                    cardinality = Cardinality::One;
                }
                (Stage::Source(values), Port::Values)
            }
            PlannedStage::Text => match port {
                Some(Port::Bytes) => {
                    cardinality = Cardinality::One;
                    (Stage::BytesToText, Port::Values)
                }
                Some(Port::Values) => (Stage::ValuesToText, Port::Bytes),
                None => return Err(transition_error(index, "text requires an input")),
            },
            PlannedStage::Json => {
                require_port(index, port, Port::Bytes)?;
                cardinality = Cardinality::One;
                (Stage::Json, Port::Values)
            }
            PlannedStage::Lines => {
                require_port(index, port, Port::Bytes)?;
                cardinality = Cardinality::Many;
                (Stage::Lines, Port::Values)
            }
            PlannedStage::JsonLines => {
                require_port(index, port, Port::Bytes)?;
                cardinality = Cardinality::Many;
                (Stage::JsonLines, Port::Values)
            }
            PlannedStage::Chunks(size) => {
                require_port(index, port, Port::Bytes)?;
                if size == 0 {
                    return Err(transition_error(index, "chunks size must be positive"));
                }
                if size > MAX_CHUNK_SIZE {
                    return Err(transition_error(
                        index,
                        &format!("chunks size {size} exceeds the {MAX_CHUNK_SIZE}-byte limit"),
                    ));
                }
                cardinality = Cardinality::Many;
                (Stage::Chunks(size), Port::Values)
            }
            PlannedStage::Function(function) => {
                require_values(index, port)?;
                (Stage::Function(function), Port::Values)
            }
            PlannedStage::Map(function) => {
                require_values(index, port)?;
                (Stage::Map(function), Port::Values)
            }
            PlannedStage::Filter(function) => {
                require_values(index, port)?;
                cardinality = Cardinality::Many;
                (Stage::Filter(function), Port::Values)
            }
            PlannedStage::Take(count) => {
                require_values(index, port)?;
                cardinality = Cardinality::Many;
                (Stage::Take(count), Port::Values)
            }
            PlannedStage::First => {
                require_values(index, port)?;
                cardinality = Cardinality::One;
                (Stage::First, Port::Values)
            }
            PlannedStage::Collect => {
                require_values(index, port)?;
                cardinality = Cardinality::One;
                (Stage::Collect, Port::Values)
            }
        };
        port = Some(output);
        ports.push(output);
        resolved.push(stage);
    }
    Ok((resolved, ports, cardinality))
}

fn require_port(stage: usize, actual: Option<Port>, expected: Port) -> Result<(), ExecutionError> {
    if actual == Some(expected) {
        Ok(())
    } else {
        Err(transition_error(
            stage,
            match expected {
                Port::Bytes => "stage requires a byte stream",
                Port::Values => "stage requires a value stream",
            },
        ))
    }
}

fn require_values(stage: usize, actual: Option<Port>) -> Result<(), ExecutionError> {
    if actual == Some(Port::Bytes) {
        Err(transition_error(
            stage,
            "cannot apply a function stage to bytes; add lines, jsonl, json, text, or chunks(n) first",
        ))
    } else {
        require_port(stage, actual, Port::Values)
    }
}

fn transition_error(stage: usize, message: &str) -> ExecutionError {
    ExecutionError::Stream {
        stage,
        message: message.into(),
    }
}

enum Terminal {
    None,
    Bytes(Vec<u8>),
    Values(Vec<Value>),
}

struct ValueAccumulator {
    boundary: &'static str,
    limits: MaterializationLimits,
    bytes: usize,
    items: usize,
    values: Vec<Value>,
}

impl ValueAccumulator {
    const fn new(boundary: &'static str, limits: MaterializationLimits) -> Self {
        Self {
            boundary,
            limits,
            bytes: 0,
            items: 0,
            values: Vec::new(),
        }
    }

    fn push(&mut self, value: Value) -> Result<(), ExecutionError> {
        let remaining_items = self.limits.items.saturating_sub(self.items);
        let Some(items) = value_materialized_items(&value, remaining_items) else {
            return Err(materialization_limit(
                self.boundary,
                MaterializationLimit::Items(self.limits.items),
            ));
        };
        let remaining = self.limits.bytes.saturating_sub(self.bytes);
        let Some(bytes) = value_materialized_bytes(&value, remaining) else {
            return Err(materialization_limit(
                self.boundary,
                MaterializationLimit::Bytes(self.limits.bytes),
            ));
        };
        self.values.try_reserve(1).map_err(|error| {
            ExecutionError::Collect(io::Error::other(format!(
                "cannot allocate {} value buffer: {error}",
                self.boundary
            )))
        })?;
        self.bytes += bytes;
        self.items += items;
        self.values.push(value);
        Ok(())
    }

    fn into_values(self) -> Vec<Value> {
        self.values
    }
}

fn capture_terminal_values(
    receiver: Receiver<StreamValue>,
    limits: MaterializationLimits,
) -> Result<Vec<Value>, ExecutionError> {
    let mut values = ValueAccumulator::new("value capture", limits);
    for value in receiver {
        values.push(value.value.clone())?;
    }
    Ok(values.into_values())
}

fn run_source_worker(
    values: Vec<Value>,
    output: Output,
    cancellation: &CancellationToken,
    early: &AtomicBool,
) -> Result<(), WorkerError> {
    let Output::Values(output) = output else {
        return Err(WorkerError::Message(
            "pipeline source requires a value output".into(),
        ));
    };
    for value in values {
        if cancellation.is_cancelled() || !send_value(&output, value, cancellation, early) {
            break;
        }
    }
    Ok(())
}

fn run_worker(
    stage: Stage,
    input: Input,
    output: Output,
    run_function: FunctionRunner,
    cancellation: &CancellationToken,
    early: &AtomicBool,
    limits: MaterializationLimits,
) -> Result<(), WorkerError> {
    match (stage, input, output) {
        (Stage::BytesToText, Input::Bytes(mut input), Output::Values(output)) => {
            let bytes = read_bounded(&mut input, "`text` input", limits.bytes)?;
            let value = match String::from_utf8(bytes) {
                Ok(text) => Value::String(Arc::from(text)),
                Err(error) => Value::Bytes(Arc::from(error.into_bytes())),
            };
            let _ = send_value(&output, value, cancellation, early);
            Ok(())
        }
        (Stage::ValuesToText, Input::Values(input), Output::Bytes(mut output)) => {
            let mut first = true;
            for value in input {
                if !first && !write_graceful(&mut output, b"\n", cancellation, early)? {
                    return Ok(());
                }
                first = false;
                let bytes = text_bytes(&value, limits.bytes)?;
                if !write_graceful(&mut output, &bytes, cancellation, early)? {
                    return Ok(());
                }
            }
            Ok(())
        }
        (Stage::ValuesToText, Input::Values(input), Output::Inherit) => {
            let stdout = io::stdout();
            let mut output = stdout.lock();
            let mut first = true;
            for value in input {
                if !first && !write_graceful(&mut output, b"\n", cancellation, early)? {
                    return Ok(());
                }
                first = false;
                if !write_graceful(
                    &mut output,
                    &text_bytes(&value, limits.bytes)?,
                    cancellation,
                    early,
                )? {
                    return Ok(());
                }
            }
            flush_graceful(&mut output, cancellation, early)?;
            Ok(())
        }
        (Stage::Json, Input::Bytes(mut input), Output::Values(output)) => {
            let bytes = read_bounded(&mut input, "`json` input", limits.bytes)?;
            let json: JsonValue =
                serde_json::from_slice(&bytes).map_err(|error| format!("invalid JSON: {error}"))?;
            let _ = send_value(&output, from_json(json)?, cancellation, early);
            Ok(())
        }
        (Stage::Lines, Input::Bytes(input), Output::Values(output)) => stream_lines(
            input,
            output,
            cancellation,
            early,
            "`lines` item",
            limits.bytes,
            |line, number| {
                String::from_utf8(line)
                    .map(|text| Value::String(Arc::from(text)))
                    .map_err(|error| format!("line {number} is not valid UTF-8: {error}"))
            },
        ),
        (Stage::JsonLines, Input::Bytes(input), Output::Values(output)) => stream_lines(
            input,
            output,
            cancellation,
            early,
            "`jsonl` item",
            limits.bytes,
            |line, number| {
                let json = serde_json::from_slice(&line)
                    .map_err(|error| format!("line {number} is not valid JSONL: {error}"))?;
                from_json(json)
            },
        ),
        (Stage::Chunks(size), Input::Bytes(mut input), Output::Values(output)) => {
            let mut buffer = Vec::new();
            buffer
                .try_reserve_exact(size)
                .map_err(|error| format!("cannot allocate {size}-byte chunk buffer: {error}"))?;
            buffer.resize(size, 0);
            loop {
                let mut used = 0;
                while used < size {
                    let read = input.read(&mut buffer[used..]).map_err(io_error)?;
                    if read == 0 {
                        break;
                    }
                    used += read;
                }
                if used == 0 {
                    break;
                }
                if !send_value(
                    &output,
                    Value::Bytes(Arc::from(&buffer[..used])),
                    cancellation,
                    early,
                ) {
                    break;
                }
            }
            Ok(())
        }
        (
            Stage::Function(function) | Stage::Map(function),
            Input::Values(input),
            Output::Values(output),
        ) => {
            for value in input {
                if cancellation.is_cancelled() {
                    break;
                }
                let value = run_function(Arc::clone(&function), value.value.clone())
                    .map_err(|error| error.to_string())?;
                if !send_value(&output, value, cancellation, early) {
                    break;
                }
            }
            Ok(())
        }
        (Stage::Filter(function), Input::Values(input), Output::Values(output)) => {
            for value in input {
                if cancellation.is_cancelled() {
                    break;
                }
                let keep = run_function(Arc::clone(&function), value.value.clone())
                    .map_err(|error| error.to_string())?;
                if keep.truthy() && !send_value(&output, value.value.clone(), cancellation, early) {
                    break;
                }
            }
            Ok(())
        }
        (Stage::Take(count), Input::Values(input), Output::Values(output)) => {
            if count == 0 {
                early.store(true, Ordering::Release);
                cancellation.cancel();
                return Ok(());
            }
            for (index, value) in input.into_iter().enumerate() {
                if index == count {
                    early.store(true, Ordering::Release);
                    cancellation.cancel();
                    break;
                }
                if !send_value(&output, value.value.clone(), cancellation, early) {
                    break;
                }
                if index + 1 == count {
                    early.store(true, Ordering::Release);
                    cancellation.cancel();
                    break;
                }
            }
            Ok(())
        }
        (Stage::First, Input::Values(input), Output::Values(output)) => {
            if let Some(value) = input.into_iter().next() {
                let _ = send_value(&output, value.value.clone(), cancellation, early);
                early.store(true, Ordering::Release);
                cancellation.cancel();
            }
            Ok(())
        }
        (Stage::Collect, Input::Values(input), Output::Values(output)) => {
            let mut values = ValueAccumulator::new("`collect`", limits);
            for value in input {
                values.push(value.value.clone())?;
            }
            let _ = send_value(
                &output,
                Value::Array(Arc::new(values.into_values())),
                cancellation,
                early,
            );
            Ok(())
        }
        (stage, input, output) => {
            Err(format!("invalid planned worker ports: {stage:?}, {input:?}, {output:?}").into())
        }
    }
}

fn stream_lines(
    input: PipeReader,
    output: SyncSender<StreamValue>,
    cancellation: &CancellationToken,
    early: &AtomicBool,
    boundary: &'static str,
    byte_limit: usize,
    mut parse: impl FnMut(Vec<u8>, usize) -> Result<Value, String>,
) -> Result<(), WorkerError> {
    let mut input = BufReader::new(input);
    let mut number = 0;
    while let Some(line) = read_bounded_line(&mut input, boundary, byte_limit)? {
        number += 1;
        let value = parse(line, number)?;
        if !send_value(&output, value, cancellation, early) {
            break;
        }
    }
    Ok(())
}

fn read_bounded_line(
    input: &mut impl BufRead,
    boundary: &'static str,
    byte_limit: usize,
) -> Result<Option<Vec<u8>>, ExecutionError> {
    let mut line = BoundedBytes::new(boundary, byte_limit);
    let mut pending_carriage_return = false;
    loop {
        let available = input.fill_buf().map_err(ExecutionError::Collect)?;
        if available.is_empty() {
            if pending_carriage_return {
                line.extend_from_slice(b"\r")?;
            }
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line.into_inner()))
            };
        }
        if let Some(newline) = available.iter().position(|byte| *byte == b'\n') {
            append_line_segment(
                &mut line,
                &available[..newline],
                &mut pending_carriage_return,
            )?;
            input.consume(newline + 1);
            return Ok(Some(line.into_inner()));
        }
        let consumed = available.len();
        append_line_segment(&mut line, available, &mut pending_carriage_return)?;
        input.consume(consumed);
    }
}

fn append_line_segment(
    line: &mut BoundedBytes,
    segment: &[u8],
    pending_carriage_return: &mut bool,
) -> Result<(), ExecutionError> {
    if *pending_carriage_return && !segment.is_empty() {
        line.extend_from_slice(b"\r")?;
        *pending_carriage_return = false;
    }
    if segment.last() == Some(&b'\r') {
        line.extend_from_slice(&segment[..segment.len() - 1])?;
        *pending_carriage_return = true;
    } else {
        line.extend_from_slice(segment)?;
    }
    Ok(())
}

fn send_value(
    output: &SyncSender<StreamValue>,
    value: Value,
    cancellation: &CancellationToken,
    early: &AtomicBool,
) -> bool {
    let (acknowledgement, acknowledged) = mpsc::channel();
    let mut item = StreamValue {
        value,
        acknowledgement: Some(acknowledgement),
    };
    loop {
        match output.try_send(item) {
            Ok(()) => break,
            Err(TrySendError::Disconnected(_)) => {
                early.store(true, Ordering::Release);
                cancellation.cancel();
                return false;
            }
            Err(TrySendError::Full(returned)) => {
                item = returned;
                if cancellation.is_cancelled() {
                    return false;
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
    loop {
        match acknowledged.recv_timeout(Duration::from_millis(1)) {
            Ok(()) => return true,
            Err(mpsc::RecvTimeoutError::Disconnected) => return false,
            Err(mpsc::RecvTimeoutError::Timeout) if cancellation.is_cancelled() => return false,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn write_values_to_external(
    input: Receiver<StreamValue>,
    mut output: PipeWriter,
    cancellation: &CancellationToken,
    early: &AtomicBool,
    byte_limit: usize,
) -> Result<(), WorkerError> {
    for value in input {
        let bytes = external_line(&value, byte_limit)?;
        if !write_graceful(&mut output, &bytes, cancellation, early)?
            || !write_graceful(&mut output, b"\n", cancellation, early)?
        {
            break;
        }
    }
    Ok(())
}

fn write_graceful(
    output: &mut impl Write,
    bytes: &[u8],
    cancellation: &CancellationToken,
    early: &AtomicBool,
) -> Result<bool, String> {
    match output.write_all(bytes) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
            early.store(true, Ordering::Release);
            cancellation.cancel();
            Ok(false)
        }
        Err(error) => Err(io_error(error)),
    }
}

fn flush_graceful(
    output: &mut impl Write,
    cancellation: &CancellationToken,
    early: &AtomicBool,
) -> Result<(), String> {
    match output.flush() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
            early.store(true, Ordering::Release);
            cancellation.cancel();
            Ok(())
        }
        Err(error) => Err(io_error(error)),
    }
}

fn external_line(value: &Value, byte_limit: usize) -> Result<Vec<u8>, WorkerError> {
    match value {
        Value::String(value) => bounded_copy("value-to-text", value.as_bytes(), byte_limit),
        _ => json_bytes(value, byte_limit),
    }
}

fn text_bytes(value: &Value, byte_limit: usize) -> Result<Vec<u8>, WorkerError> {
    match value {
        Value::String(value) => bounded_copy("value-to-text", value.as_bytes(), byte_limit),
        Value::Bytes(value) => bounded_copy("value-to-text", value, byte_limit),
        _ => json_bytes(value, byte_limit),
    }
}

fn bounded_copy(
    boundary: &'static str,
    bytes: &[u8],
    byte_limit: usize,
) -> Result<Vec<u8>, WorkerError> {
    let mut output = BoundedBytes::new(boundary, byte_limit);
    output.extend_from_slice(bytes)?;
    Ok(output.into_inner())
}

fn json_bytes(value: &Value, byte_limit: usize) -> Result<Vec<u8>, WorkerError> {
    let mut output = BoundedBytes::new("value-to-text", byte_limit);
    write_json_value(&mut output, value)?;
    Ok(output.into_inner())
}

fn write_json_value(output: &mut BoundedBytes, value: &Value) -> Result<(), WorkerError> {
    match value {
        Value::Null => output.extend_from_slice(b"null")?,
        Value::Bool(value) => {
            output.extend_from_slice(if *value { &b"true"[..] } else { &b"false"[..] })?;
        }
        Value::Int(value) => output.extend_from_slice(value.to_string().as_bytes())?,
        Value::Float(value) if value.is_finite() => {
            output.extend_from_slice(value.to_string().as_bytes())?;
        }
        Value::Float(_) => {
            return Err(WorkerError::Message(
                "non-finite floats cannot be serialized as JSON".into(),
            ));
        }
        Value::String(value) => write_json_string(output, value)?,
        Value::Array(values) => {
            output.extend_from_slice(b"[")?;
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.extend_from_slice(b",")?;
                }
                write_json_value(output, value)?;
            }
            output.extend_from_slice(b"]")?;
        }
        Value::Object(object) => {
            output.extend_from_slice(b"{")?;
            for (index, (key, value)) in object.iter().enumerate() {
                if index > 0 {
                    output.extend_from_slice(b",")?;
                }
                write_json_string(output, key)?;
                output.extend_from_slice(b":")?;
                write_json_value(output, value)?;
            }
            output.extend_from_slice(b"}")?;
        }
        Value::Bytes(_) => {
            return Err(WorkerError::Message(
                "bytes cannot be serialized as JSON; use `text` explicitly".into(),
            ));
        }
        Value::Environment => {
            return Err(WorkerError::Message(
                "the environment namespace cannot be serialized as JSON".into(),
            ));
        }
        Value::Function(_) => {
            return Err(WorkerError::Message(
                "functions cannot be serialized as JSON".into(),
            ));
        }
        Value::Error(_) => {
            return Err(WorkerError::Message(
                "errors cannot be serialized as JSON".into(),
            ));
        }
        Value::Status(_) => {
            return Err(WorkerError::Message(
                "statuses cannot be serialized as JSON".into(),
            ));
        }
    }
    Ok(())
}

fn write_json_string(output: &mut BoundedBytes, value: &str) -> Result<(), ExecutionError> {
    output.extend_from_slice(b"\"")?;
    for character in value.chars() {
        match character {
            '"' => output.extend_from_slice(b"\\\"")?,
            '\\' => output.extend_from_slice(b"\\\\")?,
            '\u{08}' => output.extend_from_slice(b"\\b")?,
            '\u{0c}' => output.extend_from_slice(b"\\f")?,
            '\n' => output.extend_from_slice(b"\\n")?,
            '\r' => output.extend_from_slice(b"\\r")?,
            '\t' => output.extend_from_slice(b"\\t")?,
            character if character <= '\u{1f}' => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let code = character as u8;
                output.extend_from_slice(&[
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[(code >> 4) as usize],
                    HEX[(code & 0x0f) as usize],
                ])?;
            }
            character => {
                let mut encoded = [0_u8; 4];
                output.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes())?;
            }
        }
    }
    output.extend_from_slice(b"\"")
}

fn from_json(value: JsonValue) -> Result<Value, String> {
    match value {
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Bool(value) => Ok(Value::Bool(value)),
        JsonValue::Number(value) => value
            .as_i64()
            .map(Value::Int)
            .or_else(|| {
                value
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .map(Value::Float)
            })
            .ok_or_else(|| format!("JSON number {value} is outside Josh's numeric range")),
        JsonValue::String(value) => Ok(Value::String(Arc::from(value))),
        JsonValue::Array(values) => {
            let mut converted = Vec::new();
            for value in values {
                converted.try_reserve(1).map_err(|error| {
                    format!("cannot allocate JSON array materialization: {error}")
                })?;
                converted.push(from_json(value)?);
            }
            Ok(Value::Array(Arc::new(converted)))
        }
        JsonValue::Object(entries) => {
            let mut converted = ObjectValue::new();
            for (key, value) in entries {
                converted
                    .try_insert(Arc::from(key), from_json(value)?)
                    .map_err(|error| {
                        format!("cannot allocate JSON object materialization: {error}")
                    })?;
            }
            Ok(Value::Object(Arc::new(converted)))
        }
    }
}

fn capture_values(values: Vec<Value>, cardinality: Cardinality) -> Result<Value, ExecutionError> {
    match cardinality {
        Cardinality::Many => Ok(Value::Array(Arc::new(values))),
        Cardinality::One => match values.as_slice() {
            [] => Ok(Value::Null),
            [value] => Ok(value.clone()),
            _ => Err(ExecutionError::Stream {
                stage: 0,
                message: "a single-value stage emitted more than one value".into(),
            }),
        },
    }
}

fn stop_after_grace(
    children: &mut [(usize, String, Child)],
    statuses: &mut [Option<(String, ExitStatus)>],
    killed: &mut [bool],
    early: &AtomicBool,
) {
    let deadline = Instant::now()
        + if early.load(Ordering::Acquire) {
            Duration::from_millis(100)
        } else {
            Duration::ZERO
        };
    loop {
        poll_children(children, statuses);
        if children
            .iter()
            .all(|(stage, _, _)| statuses[*stage].is_some())
            || Instant::now() >= deadline
        {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    for (stage, _, child) in children {
        if statuses[*stage].is_none() && kill_process_tree(child).is_ok() {
            killed[*stage] = true;
        }
    }
}

fn poll_children(
    children: &mut [(usize, String, Child)],
    statuses: &mut [Option<(String, ExitStatus)>],
) {
    for (stage, rendered, child) in children {
        if statuses[*stage].is_none()
            && let Ok(Some(status)) = child.try_wait()
        {
            statuses[*stage] = Some((rendered.clone(), status));
        }
    }
}

fn reap_all(
    children: &mut [(usize, String, Child)],
    statuses: &mut [Option<(String, ExitStatus)>],
) -> Result<(), ExecutionError> {
    for (stage, rendered, child) in children {
        if statuses[*stage].is_none() {
            let status = child.wait().map_err(ExecutionError::Collect)?;
            statuses[*stage] = Some((rendered.clone(), status));
        }
    }
    Ok(())
}

fn terminate_and_reap(children: &mut [(usize, String, Child)]) {
    for (_, _, child) in children.iter_mut() {
        let _ = kill_process_tree(child);
    }
    for (_, _, child) in children.iter_mut() {
        let _ = child.wait();
    }
}

#[cfg(unix)]
fn kill_process_tree(child: &mut Child) -> io::Result<()> {
    use nix::{
        sys::signal::{Signal, killpg},
        unistd::Pid,
    };

    let pid =
        i32::try_from(child.id()).map_err(|_| io::Error::other("child PID does not fit in i32"))?;
    killpg(Pid::from_raw(pid), Signal::SIGKILL).map_err(io::Error::other)
}

#[cfg(not(unix))]
fn kill_process_tree(child: &mut Child) -> io::Result<()> {
    child.kill()
}

fn outcome(stage: usize, rendered: String, status: ExitStatus) -> StageOutcome {
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    };
    #[cfg(not(unix))]
    let signal = None;
    StageOutcome {
        stage,
        rendered,
        code: status.code(),
        signal,
        success: status.success(),
    }
}

fn status_description(outcome: &StageOutcome) -> String {
    if let Some(code) = outcome.code {
        format!("exit {code}")
    } else if let Some(signal) = outcome.signal {
        format!("signal {signal}")
    } else {
        "unknown status".into()
    }
}

fn decode_capture(mut bytes: Vec<u8>) -> Captured {
    while bytes.last() == Some(&b'\n') {
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
    }
    match String::from_utf8(bytes) {
        Ok(text) => Captured::String(Arc::from(text)),
        Err(error) => Captured::Bytes(Arc::from(error.into_bytes())),
    }
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufReader, Cursor},
        process::Command,
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };

    use josh_runtime::{
        CancellationToken, Captured, EngineError, ExecutionError, FunctionValue,
        MaterializationLimit, Value,
    };
    use tempfile::tempdir;

    use super::{
        MaterializationLimits, PlannedExternal, PlannedStage, ValueAccumulator, read_bounded_line,
        run_with_cancellation, run_with_materialization_limits, text_bytes,
    };

    fn no_function(_function: Arc<FunctionValue>, _value: Value) -> Result<Value, EngineError> {
        panic!("test graph contains no function stage")
    }

    fn external(script: impl Into<String>) -> PlannedStage {
        let script = script.into();
        PlannedStage::External(PlannedExternal {
            executable: "/bin/sh".into(),
            argv: vec!["sh".into(), "-c".into(), script.clone().into()],
            rendered: format!("sh -c '{script}'"),
            redirections: Vec::new(),
        })
    }

    fn run_limited(
        stages: Vec<PlannedStage>,
        bytes: usize,
        items: usize,
    ) -> Result<josh_runtime::ExecutionResult, ExecutionError> {
        run_with_materialization_limits(
            stages,
            true,
            Arc::new(no_function),
            CancellationToken::default(),
            josh_runtime::ShellContext::from_process().snapshot(),
            MaterializationLimits { bytes, items },
        )
    }

    #[test]
    fn byte_to_value_boundaries_accept_exact_limit_and_reject_limit_plus_one() {
        let text = run_limited(vec![external("printf 1234"), PlannedStage::Text], 4, 10)
            .expect("exact text input");
        assert_eq!(
            text.captured,
            Some(Captured::Value(Value::String(Arc::from("1234"))))
        );
        let text_error = run_limited(vec![external("printf 12345"), PlannedStage::Text], 4, 10)
            .expect_err("oversized text input");
        assert!(matches!(
            text_error,
            ExecutionError::MaterializationLimit {
                boundary: "`text` input",
                limit: MaterializationLimit::Bytes(4)
            }
        ));

        let json = run_limited(vec![external("printf 12345678"), PlannedStage::Json], 8, 10)
            .expect("exact JSON input");
        assert_eq!(json.captured, Some(Captured::Value(Value::Int(12_345_678))));
        let json_error = run_limited(
            vec![external("printf 123456789"), PlannedStage::Json],
            8,
            10,
        )
        .expect_err("oversized JSON input");
        assert!(matches!(
            json_error,
            ExecutionError::MaterializationLimit {
                boundary: "`json` input",
                limit: MaterializationLimit::Bytes(8)
            }
        ));
    }

    #[test]
    fn line_and_value_builders_share_exact_byte_and_item_limits() {
        let mut exact = BufReader::new(Cursor::new(b"1234\n"));
        assert_eq!(
            read_bounded_line(&mut exact, "line", 4).unwrap(),
            Some(b"1234".to_vec())
        );
        let mut exact_crlf = BufReader::with_capacity(5, Cursor::new(b"1234\r\n"));
        assert_eq!(
            read_bounded_line(&mut exact_crlf, "line", 4).unwrap(),
            Some(b"1234".to_vec())
        );
        let mut overflow = BufReader::new(Cursor::new(b"12345\n"));
        assert!(matches!(
            read_bounded_line(&mut overflow, "line", 4),
            Err(ExecutionError::MaterializationLimit {
                boundary: "line",
                limit: MaterializationLimit::Bytes(4)
            })
        ));

        let jsonl = run_limited(
            vec![external("printf '\"ab\"\\n'"), PlannedStage::JsonLines],
            4,
            1,
        )
        .expect("exact JSONL item");
        assert_eq!(
            jsonl.captured,
            Some(Captured::Value(Value::Array(Arc::new(vec![
                Value::String(Arc::from("ab")),
            ]))))
        );
        let jsonl_error = run_limited(
            vec![external("printf '\"abc\"\\n'"), PlannedStage::JsonLines],
            4,
            1,
        )
        .expect_err("oversized JSONL item");
        assert!(matches!(
            jsonl_error,
            ExecutionError::MaterializationLimit {
                boundary: "`jsonl` item",
                limit: MaterializationLimit::Bytes(4)
            }
        ));

        let limits = MaterializationLimits { bytes: 4, items: 2 };
        let mut values = ValueAccumulator::new("values", limits);
        values.push(Value::String(Arc::from("12"))).unwrap();
        values.push(Value::String(Arc::from("34"))).unwrap();
        assert!(matches!(
            values.push(Value::Null),
            Err(ExecutionError::MaterializationLimit {
                boundary: "values",
                limit: MaterializationLimit::Items(2)
            })
        ));
        let mut nested = ValueAccumulator::new("values", limits);
        nested
            .push(Value::Array(Arc::new(vec![Value::Null, Value::Null])))
            .unwrap();
        let mut nested_overflow = ValueAccumulator::new("values", limits);
        assert!(matches!(
            nested_overflow.push(Value::Array(Arc::new(vec![
                Value::Null,
                Value::Null,
                Value::Null,
            ]))),
            Err(ExecutionError::MaterializationLimit {
                boundary: "values",
                limit: MaterializationLimit::Items(2)
            })
        ));

        let mut bytes = ValueAccumulator::new("values", limits);
        assert!(matches!(
            bytes.push(Value::String(Arc::from("12345"))),
            Err(ExecutionError::MaterializationLimit {
                boundary: "values",
                limit: MaterializationLimit::Bytes(4)
            })
        ));
    }

    #[test]
    fn collect_chunks_and_value_to_text_are_bounded_without_changing_cardinality() {
        let collected = run_limited(
            vec![
                external("printf 'a\\nb\\n'"),
                PlannedStage::Lines,
                PlannedStage::Collect,
            ],
            2,
            2,
        )
        .expect("exact collect");
        assert_eq!(
            collected.captured,
            Some(Captured::Value(Value::Array(Arc::new(vec![
                Value::String(Arc::from("a")),
                Value::String(Arc::from("b")),
            ]))))
        );
        let collect_error = run_limited(
            vec![
                external("printf 'a\\nb\\nc\\n'"),
                PlannedStage::Lines,
                PlannedStage::Collect,
            ],
            3,
            2,
        )
        .expect_err("collect item overflow");
        assert!(matches!(
            collect_error,
            ExecutionError::MaterializationLimit {
                boundary: "`collect`",
                limit: MaterializationLimit::Items(2)
            }
        ));

        let chunks_error = run_limited(
            vec![external("printf 12345"), PlannedStage::Chunks(2)],
            4,
            10,
        )
        .expect_err("chunk capture byte overflow");
        assert!(matches!(
            chunks_error,
            ExecutionError::MaterializationLimit {
                boundary: "value capture",
                limit: MaterializationLimit::Bytes(4)
            }
        ));

        let value = Value::Array(Arc::new(vec![Value::String(Arc::from("ab"))]));
        assert_eq!(text_bytes(&value, 6).unwrap(), br#"["ab"]"#);
        assert!(matches!(
            text_bytes(&value, 5),
            Err(super::WorkerError::Execution(
                ExecutionError::MaterializationLimit {
                    boundary: "value-to-text",
                    limit: MaterializationLimit::Bytes(5)
                }
            ))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn value_capture_overflow_cancels_reaps_and_joins_the_graph() {
        let temp = tempdir().unwrap();
        let pid_file = temp.path().join("pid");
        let script = format!(
            "printf '%s' $$ > {}; while :; do printf 'x\\n'; done",
            pid_file.display()
        );
        let started = Instant::now();
        let error = run_limited(vec![external(script), PlannedStage::Lines], 16, 2)
            .expect_err("value item overflow");
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(matches!(
            error,
            ExecutionError::MaterializationLimit {
                boundary: "value capture",
                limit: MaterializationLimit::Items(2)
            }
        ));

        let pid = fs::read_to_string(pid_file).unwrap();
        let alive = Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        assert!(!alive, "overflowing producer {pid} was not reaped");
    }

    #[test]
    fn external_cancellation_reaps_the_child_and_joins_the_reader() {
        let temp = tempdir().unwrap();
        let pid_file = temp.path().join("pid");
        let script = format!("echo $$ > {}; sleep 30", pid_file.display());
        let stages = vec![
            PlannedStage::External(PlannedExternal {
                executable: "/bin/sh".into(),
                argv: vec!["sh".into(), "-c".into(), script.clone().into()],
                rendered: format!("sh -c '{script}'"),
                redirections: Vec::new(),
            }),
            PlannedStage::Lines,
        ];
        let cancellation = CancellationToken::default();
        let trigger = cancellation.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            trigger.cancel();
        });
        let error = run_with_cancellation(
            stages,
            true,
            Arc::new(no_function),
            cancellation,
            josh_runtime::ShellContext::from_process().snapshot(),
        )
        .expect_err("cancelled graph");
        canceller.join().unwrap();
        assert!(matches!(error, josh_runtime::ExecutionError::Cancelled));

        let pid = fs::read_to_string(pid_file).unwrap();
        let alive = Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        assert!(!alive, "cancelled child {pid} was not reaped");
    }
}
