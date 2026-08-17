use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io,
    os::fd::AsFd,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use josh_runtime::{
    CancellationToken, Captured, CommandSpec, Engine, ExecutionError, ExecutionHost,
    ExecutionResult, MAX_MATERIALIZED_BYTES, RedirectionSpec, ShellContext, ShellSnapshot,
    StageOutcome, StreamStage, read_bounded,
};
use josh_streams::{PlannedExternal, PlannedRedirection, PlannedStage, configure_external_stdio};
use nix::poll::{PollFd, PollFlags, poll};
use os_pipe::PipeWriter;
#[cfg(not(unix))]
use std::{process::ExitStatus, sync::mpsc};

#[cfg(unix)]
mod terminal;
#[cfg(unix)]
use terminal::TerminalController;

#[derive(Debug, Clone)]
pub struct PlannedCommand {
    pub executable: PathBuf,
    pub argv: Vec<OsString>,
    pub rendered: String,
    pub redirections: Vec<PlannedRedirection>,
}

#[derive(Debug, Default)]
pub struct ProcessHost {
    #[cfg(unix)]
    terminal: Option<TerminalController>,
}

impl ProcessHost {
    pub fn interactive() -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::io::IsTerminal;

            if !io::stdin().is_terminal() {
                return Ok(Self::default());
            }
            Ok(Self {
                terminal: TerminalController::open()?,
            })
        }
        #[cfg(not(unix))]
        Ok(Self::default())
    }
}

impl ExecutionHost for ProcessHost {
    fn execute(
        &mut self,
        commands: Vec<CommandSpec>,
        capture: bool,
        cancellation: CancellationToken,
        context: ShellContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        let snapshot = context.snapshot();
        let planned = plan_specs(commands, &snapshot)?;
        run_with_cancellation(
            &planned,
            capture,
            cancellation,
            &snapshot,
            #[cfg(unix)]
            self.terminal.as_ref(),
        )
    }

    fn execute_stream(
        &mut self,
        stages: Vec<StreamStage>,
        capture: bool,
        cancellation: CancellationToken,
        context: ShellContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        let snapshot = context.snapshot();
        enum PreflightStage {
            External(PlannedCommand, Vec<RedirectionSpec>),
            Ready(PlannedStage),
        }
        let preflight = stages
            .into_iter()
            .enumerate()
            .map(|(stage, item)| {
                Ok(match item {
                    StreamStage::External(command) => {
                        let argv = command
                            .argv
                            .into_iter()
                            .map(command_arg)
                            .collect::<Result<Vec<_>, _>>()?;
                        PreflightStage::External(
                            plan_command(stage, argv, &snapshot)?,
                            command.redirections,
                        )
                    }
                    StreamStage::Source { values, scalar } => {
                        PreflightStage::Ready(PlannedStage::Source { values, scalar })
                    }
                    StreamStage::Text => PreflightStage::Ready(PlannedStage::Text),
                    StreamStage::Json => PreflightStage::Ready(PlannedStage::Json),
                    StreamStage::Lines => PreflightStage::Ready(PlannedStage::Lines),
                    StreamStage::JsonLines => PreflightStage::Ready(PlannedStage::JsonLines),
                    StreamStage::Chunks(size) => PreflightStage::Ready(PlannedStage::Chunks(size)),
                    StreamStage::Function(function) => {
                        PreflightStage::Ready(PlannedStage::Function(function))
                    }
                    StreamStage::Map(function) => {
                        PreflightStage::Ready(PlannedStage::Map(function))
                    }
                    StreamStage::Filter(function) => {
                        PreflightStage::Ready(PlannedStage::Filter(function))
                    }
                    StreamStage::Take(count) => PreflightStage::Ready(PlannedStage::Take(count)),
                    StreamStage::TakeLast(count) => {
                        PreflightStage::Ready(PlannedStage::TakeLast(count))
                    }
                    StreamStage::First => PreflightStage::Ready(PlannedStage::First),
                    StreamStage::Collect => PreflightStage::Ready(PlannedStage::Collect),
                })
            })
            .collect::<Result<Vec<_>, ExecutionError>>()?;
        let planned = preflight
            .into_iter()
            .map(|stage| match stage {
                PreflightStage::External(mut command, redirections) => {
                    command.redirections = open_redirections(redirections, snapshot.cwd())?;
                    Ok(PlannedStage::External(PlannedExternal {
                        executable: command.executable,
                        argv: command.argv,
                        rendered: command.rendered,
                        redirections: command.redirections,
                    }))
                }
                PreflightStage::Ready(stage) => Ok(stage),
            })
            .collect::<Result<Vec<_>, ExecutionError>>()?;
        let graph_cancellation = cancellation.child();
        let runner_cancellation = graph_cancellation.clone();
        let function_context = context.clone();
        let run_function = Arc::new(move |function, value| {
            Engine::with_shell_context_and_cancellation_token(
                ProcessHost::default(),
                function_context.clone(),
                runner_cancellation.clone(),
            )
            .call_stream_function(function, value)
        });
        josh_streams::run_with_cancellation(
            planned,
            capture,
            run_function,
            graph_cancellation,
            snapshot,
        )
    }

    fn glob(&self, pattern: &[u8], context: &ShellContext) -> Result<Vec<Vec<u8>>, ExecutionError> {
        expand_glob(pattern, context.snapshot().cwd())
    }
}

pub fn plan(argvs: Vec<Vec<OsString>>) -> Result<Vec<PlannedCommand>, ExecutionError> {
    let snapshot = ShellContext::from_process().snapshot();
    argvs
        .into_iter()
        .enumerate()
        .map(|(stage, argv)| plan_command(stage, argv, &snapshot))
        .collect()
}

fn plan_specs(
    commands: Vec<CommandSpec>,
    snapshot: &ShellSnapshot,
) -> Result<Vec<PlannedCommand>, ExecutionError> {
    let converted = commands
        .into_iter()
        .map(|command| {
            let argv = command
                .argv
                .into_iter()
                .map(command_arg)
                .collect::<Result<Vec<_>, _>>()?;
            Ok((argv, command.redirections))
        })
        .collect::<Result<Vec<_>, ExecutionError>>()?;
    let mut planned = converted
        .iter()
        .enumerate()
        .map(|(stage, (argv, _))| plan_command(stage, argv.clone(), snapshot))
        .collect::<Result<Vec<_>, _>>()?;
    for (command, (_, redirections)) in planned.iter_mut().zip(converted) {
        command.redirections = open_redirections(redirections, snapshot.cwd())?;
    }
    Ok(planned)
}

fn open_redirections(
    redirections: Vec<RedirectionSpec>,
    cwd: &Path,
) -> Result<Vec<PlannedRedirection>, ExecutionError> {
    redirections
        .into_iter()
        .map(|redirection| {
            Ok(match redirection {
                RedirectionSpec::Input(path) => {
                    PlannedRedirection::Input(Arc::new(open_path(&path, OpenMode::Read, cwd)?))
                }
                RedirectionSpec::Output(path) => {
                    PlannedRedirection::Output(Arc::new(open_path(&path, OpenMode::Truncate, cwd)?))
                }
                RedirectionSpec::Append(path) => {
                    PlannedRedirection::Output(Arc::new(open_path(&path, OpenMode::Append, cwd)?))
                }
                RedirectionSpec::Error(path) => {
                    PlannedRedirection::Error(Arc::new(open_path(&path, OpenMode::Truncate, cwd)?))
                }
                RedirectionSpec::ErrorAppend(path) => {
                    PlannedRedirection::Error(Arc::new(open_path(&path, OpenMode::Append, cwd)?))
                }
                RedirectionSpec::ErrorToOutput => PlannedRedirection::ErrorToOutput,
                RedirectionSpec::OutputAndError(path) => PlannedRedirection::OutputAndError(
                    Arc::new(open_path(&path, OpenMode::Truncate, cwd)?),
                ),
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
enum OpenMode {
    Read,
    Truncate,
    Append,
}

fn open_path(path: &[u8], mode: OpenMode, cwd: &Path) -> Result<File, ExecutionError> {
    let path = command_arg(path.to_vec())?;
    let target = if Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        cwd.join(&path)
    };
    let mut options = OpenOptions::new();
    match mode {
        OpenMode::Read => {
            options.read(true);
        }
        OpenMode::Truncate => {
            options.write(true).create(true).truncate(true);
        }
        OpenMode::Append => {
            options.append(true).create(true);
        }
    }
    options
        .open(&target)
        .map_err(|source| ExecutionError::RedirectionOpen {
            path: path.to_string_lossy().into_owned(),
            source,
        })
}

fn plan_command(
    stage: usize,
    argv: Vec<OsString>,
    snapshot: &ShellSnapshot,
) -> Result<PlannedCommand, ExecutionError> {
    let Some(name) = argv.first() else {
        return Err(ExecutionError::CommandNotFound {
            stage,
            command: "<empty command>".into(),
        });
    };
    let executable = resolve(name, snapshot).ok_or_else(|| ExecutionError::CommandNotFound {
        stage,
        command: name.to_string_lossy().into_owned(),
    })?;
    let rendered = argv
        .iter()
        .map(|x| shell_render(x))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(PlannedCommand {
        executable,
        argv,
        rendered,
        redirections: Vec::new(),
    })
}

pub fn run(planned: &[PlannedCommand], capture: bool) -> Result<ExecutionResult, ExecutionError> {
    let snapshot = ShellContext::from_process().snapshot();
    run_with_cancellation(
        planned,
        capture,
        CancellationToken::default(),
        &snapshot,
        #[cfg(unix)]
        None,
    )
}

fn run_with_cancellation(
    planned: &[PlannedCommand],
    capture: bool,
    cancellation: CancellationToken,
    snapshot: &ShellSnapshot,
    #[cfg(unix)] terminal: Option<&TerminalController>,
) -> Result<ExecutionResult, ExecutionError> {
    run_with_cancellation_and_limit(
        planned,
        capture,
        cancellation,
        snapshot,
        MAX_MATERIALIZED_BYTES,
        #[cfg(unix)]
        terminal,
    )
}

fn run_with_cancellation_and_limit(
    planned: &[PlannedCommand],
    capture: bool,
    cancellation: CancellationToken,
    snapshot: &ShellSnapshot,
    byte_limit: usize,
    #[cfg(unix)] terminal: Option<&TerminalController>,
) -> Result<ExecutionResult, ExecutionError> {
    run_graph_with_limit(
        planned,
        capture,
        cancellation.child(),
        snapshot,
        byte_limit,
        #[cfg(unix)]
        terminal,
    )
}

fn run_graph_with_limit(
    planned: &[PlannedCommand],
    capture: bool,
    cancellation: CancellationToken,
    snapshot: &ShellSnapshot,
    byte_limit: usize,
    #[cfg(unix)] terminal: Option<&TerminalController>,
) -> Result<ExecutionResult, ExecutionError> {
    #[cfg(unix)]
    {
        run_graph_unix(
            planned,
            capture,
            cancellation,
            snapshot,
            byte_limit,
            terminal,
        )
    }
    #[cfg(not(unix))]
    {
        let mut children: Vec<(String, Child)> = Vec::with_capacity(planned.len());
        let mut previous_stdin = None;
        let mut captured_stdout = None;
        let mut pipe_monitors = Vec::with_capacity(planned.len().saturating_sub(1));
        for (stage, item) in planned.iter().enumerate() {
            let mut command = Command::new(&item.executable);
            command
                .args(&item.argv[1..])
                .current_dir(snapshot.cwd())
                .env_clear()
                .envs(snapshot.environment());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
            let input = previous_stdin.take();
            let output = if stage + 1 < planned.len() {
                let (reader, writer) = match os_pipe::pipe() {
                    Ok(pipe) => pipe,
                    Err(error) => {
                        terminate_and_reap(&mut children);
                        return Err(ExecutionError::Collect(error));
                    }
                };
                let monitor = match writer.try_clone() {
                    Ok(monitor) => monitor,
                    Err(error) => {
                        terminate_and_reap(&mut children);
                        return Err(ExecutionError::Collect(error));
                    }
                };
                previous_stdin = Some(reader);
                pipe_monitors.push((stage, monitor));
                Some(writer)
            } else if capture {
                let (reader, writer) = match os_pipe::pipe() {
                    Ok(pipe) => pipe,
                    Err(error) => {
                        terminate_and_reap(&mut children);
                        return Err(ExecutionError::Collect(error));
                    }
                };
                captured_stdout = Some(reader);
                Some(writer)
            } else {
                None
            };
            if let Err(error) =
                configure_external_stdio(&mut command, input, output, &item.redirections)
            {
                terminate_and_reap(&mut children);
                return Err(ExecutionError::Collect(error));
            }
            match command.spawn() {
                Ok(child) => children.push((item.rendered.clone(), child)),
                Err(source) => {
                    terminate_and_reap(&mut children);
                    return Err(ExecutionError::Spawn {
                        stage,
                        command: item.rendered.clone(),
                        source,
                    });
                }
            }
        }

        let child_count = children.len();
        let process_groups = children
            .iter()
            .map(|(_, child)| child.id())
            .collect::<Vec<_>>();
        let completed = (0..child_count)
            .map(|_| Arc::new(AtomicBool::new(false)))
            .collect::<Vec<_>>();
        let cancellation_done = Arc::new(AtomicBool::new(false));
        let watcher_done = Arc::clone(&cancellation_done);
        let watcher_cancellation = cancellation.clone();
        let watcher_completed = completed.clone();
        let cancellation_watcher = thread::spawn(move || {
            while !watcher_done.load(Ordering::Acquire) {
                if watcher_completed
                    .iter()
                    .all(|completed| completed.load(Ordering::Acquire))
                {
                    return false;
                }
                if watcher_cancellation.is_cancelled() {
                    for pid in process_groups {
                        let _ = kill_process_group(pid);
                    }
                    return true;
                }
                thread::sleep(std::time::Duration::from_millis(2));
            }
            false
        });
        let (sender, receiver) = mpsc::channel();
        let waiter_threads = children
            .into_iter()
            .enumerate()
            .map(|(stage, (rendered, mut child))| {
                let sender = sender.clone();
                let completed = Arc::clone(&completed[stage]);
                thread::spawn(move || {
                    let status = child.wait();
                    completed.store(true, Ordering::Release);
                    let _ = sender.send((stage, rendered, status));
                })
            })
            .collect::<Vec<_>>();
        drop(sender);
        let monitor_threads = pipe_monitors
            .into_iter()
            .map(|(stage, writer)| {
                let completed = Arc::clone(&completed[stage]);
                (
                    stage,
                    thread::spawn(move || downstream_closed_before_upstream(writer, &completed)),
                )
            })
            .collect::<Vec<_>>();

        let capture_result = captured_stdout.as_mut().map_or_else(
            || Ok(Vec::new()),
            |stdout| read_bounded(stdout, "external capture", byte_limit),
        );
        if capture_result.is_err() {
            cancellation.cancel();
        }
        let mut statuses = (0..child_count).map(|_| None).collect::<Vec<_>>();
        for _ in 0..child_count {
            let Ok((stage, rendered, status)) = receiver.recv() else {
                cancellation.cancel();
                for waiter in waiter_threads {
                    let _ = waiter.join();
                }
                cancellation_done.store(true, Ordering::Release);
                let _ = cancellation_watcher.join();
                return Err(ExecutionError::Collect(io::Error::other(
                    "process waiter failed before reporting every child",
                )));
            };
            statuses[stage] = Some((rendered, status));
        }
        let mut waiter_panicked = false;
        for waiter in waiter_threads {
            waiter_panicked |= waiter.join().is_err();
        }
        let mut downstream_closed = vec![false; child_count];
        for (stage, thread) in monitor_threads {
            downstream_closed[stage] = thread.join().unwrap_or(false);
        }
        cancellation_done.store(true, Ordering::Release);
        let was_cancelled = cancellation_watcher.join().unwrap_or(false);
        if waiter_panicked {
            return Err(ExecutionError::Collect(io::Error::other(
                "process waiter panicked",
            )));
        }
        let bytes = capture_result?;
        let mut outcomes = statuses
            .into_iter()
            .enumerate()
            .map(|(stage, status)| {
                let (rendered, status) = status.expect("every child waiter reported a status");
                Ok(outcome(
                    stage,
                    rendered,
                    status.map_err(ExecutionError::Collect)?,
                ))
            })
            .collect::<Result<Vec<_>, ExecutionError>>()?;
        if was_cancelled {
            return Err(ExecutionError::Cancelled);
        }
        for stage in (0..outcomes.len().saturating_sub(1)).rev() {
            if outcomes[stage].signal == Some(13)
                && downstream_closed[stage]
                && outcomes[stage + 1].success
            {
                outcomes[stage].success = true;
            }
        }
        if let Some(failed) = outcomes.iter().find(|x| !x.success) {
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

        let captured = capture.then(|| decode_capture(bytes));
        Ok(ExecutionResult { outcomes, captured })
    }
}

#[cfg(unix)]
fn run_graph_unix(
    planned: &[PlannedCommand],
    capture: bool,
    cancellation: CancellationToken,
    snapshot: &ShellSnapshot,
    byte_limit: usize,
    terminal: Option<&TerminalController>,
) -> Result<ExecutionResult, ExecutionError> {
    use nix::{
        errno::Errno,
        sys::{
            signal::{Signal, killpg},
            wait::{WaitPidFlag, WaitStatus, waitpid},
        },
        unistd::Pid,
    };

    #[derive(Clone, Copy)]
    enum ProcessCompletion {
        Exited(i32),
        Signaled(i32),
    }

    let mut children: Vec<(String, Child)> = Vec::with_capacity(planned.len());
    let mut previous_stdin = None;
    let mut captured_stdout = None;
    let mut pipe_monitors = Vec::with_capacity(planned.len().saturating_sub(1));
    let mut pipeline_pgid = None;
    let mut child_pids = Vec::with_capacity(planned.len());

    for (stage, item) in planned.iter().enumerate() {
        let mut command = Command::new(&item.executable);
        command
            .args(&item.argv[1..])
            .current_dir(snapshot.cwd())
            .env_clear()
            .envs(snapshot.environment());
        use std::os::unix::process::CommandExt;
        command.process_group(pipeline_pgid.map_or(0, Pid::as_raw));

        let input = previous_stdin.take();
        let output = if stage + 1 < planned.len() {
            let (reader, writer) = match os_pipe::pipe() {
                Ok(pipe) => pipe,
                Err(error) => {
                    terminate_group_and_reap(&mut children, pipeline_pgid);
                    return Err(ExecutionError::Collect(error));
                }
            };
            let monitor = match writer.try_clone() {
                Ok(monitor) => monitor,
                Err(error) => {
                    terminate_group_and_reap(&mut children, pipeline_pgid);
                    return Err(ExecutionError::Collect(error));
                }
            };
            previous_stdin = Some(reader);
            pipe_monitors.push((stage, monitor));
            Some(writer)
        } else if capture {
            let (reader, writer) = match os_pipe::pipe() {
                Ok(pipe) => pipe,
                Err(error) => {
                    terminate_group_and_reap(&mut children, pipeline_pgid);
                    return Err(ExecutionError::Collect(error));
                }
            };
            captured_stdout = Some(reader);
            Some(writer)
        } else {
            None
        };
        if let Err(error) =
            configure_external_stdio(&mut command, input, output, &item.redirections)
        {
            terminate_group_and_reap(&mut children, pipeline_pgid);
            return Err(ExecutionError::Collect(error));
        }
        match command.spawn() {
            Ok(mut child) => {
                let pid = match i32::try_from(child.id()) {
                    Ok(pid) => Pid::from_raw(pid),
                    Err(_) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        terminate_group_and_reap(&mut children, pipeline_pgid);
                        return Err(ExecutionError::Collect(io::Error::other(
                            "child PID does not fit in i32",
                        )));
                    }
                };
                if pipeline_pgid.is_none() {
                    pipeline_pgid = Some(pid);
                }
                child_pids.push(pid);
                children.push((item.rendered.clone(), child));
            }
            Err(source) => {
                terminate_group_and_reap(&mut children, pipeline_pgid);
                return Err(ExecutionError::Spawn {
                    stage,
                    command: item.rendered.clone(),
                    source,
                });
            }
        }
    }

    let Some(pgid) = pipeline_pgid else {
        return Err(ExecutionError::Collect(io::Error::other(
            "external pipeline has no processes",
        )));
    };
    let mut foreground = if capture {
        None
    } else {
        match terminal
            .map(|controller| controller.handoff(pgid))
            .transpose()
        {
            Ok(guard) => guard,
            Err(error) => {
                terminate_group_and_reap(&mut children, Some(pgid));
                return Err(ExecutionError::Terminal(error));
            }
        }
    };
    if foreground.is_some() {
        let _ = killpg(pgid, Signal::SIGCONT);
    }

    let child_count = children.len();
    let completed = (0..child_count)
        .map(|_| Arc::new(AtomicBool::new(false)))
        .collect::<Vec<_>>();
    let monitor_threads = pipe_monitors
        .into_iter()
        .map(|(stage, writer)| {
            let completed = Arc::clone(&completed[stage]);
            (
                stage,
                thread::spawn(move || downstream_closed_before_upstream(writer, &completed)),
            )
        })
        .collect::<Vec<_>>();
    let capture_cancellation = cancellation.clone();
    let capture_worker = thread::spawn(move || {
        let result = captured_stdout.as_mut().map_or_else(
            || Ok(Vec::new()),
            |stdout| read_bounded(stdout, "external capture", byte_limit),
        );
        if result.is_err() {
            capture_cancellation.cancel();
        }
        result
    });

    let mut statuses = vec![None; child_count];
    let mut remaining = child_count;
    let mut was_cancelled = false;
    let mut stop_signal = None;
    let mut wait_error = None;
    while remaining > 0 {
        if let Some(controller) = terminal {
            controller.forward_pending(pgid);
        }
        if cancellation.is_cancelled() && !was_cancelled {
            was_cancelled = true;
            let _ = killpg(pgid, Signal::SIGKILL);
        }
        match waitpid(
            Pid::from_raw(-pgid.as_raw()),
            Some(WaitPidFlag::WNOHANG | WaitPidFlag::WUNTRACED | WaitPidFlag::WCONTINUED),
        ) {
            Ok(WaitStatus::Exited(pid, code)) => {
                if let Some(stage) = child_pids.iter().position(|candidate| *candidate == pid)
                    && statuses[stage].is_none()
                {
                    statuses[stage] = Some(ProcessCompletion::Exited(code));
                    completed[stage].store(true, Ordering::Release);
                    remaining -= 1;
                }
            }
            Ok(WaitStatus::Signaled(pid, signal, _)) => {
                if let Some(stage) = child_pids.iter().position(|candidate| *candidate == pid)
                    && statuses[stage].is_none()
                {
                    statuses[stage] = Some(ProcessCompletion::Signaled(signal as i32));
                    completed[stage].store(true, Ordering::Release);
                    remaining -= 1;
                }
            }
            Ok(WaitStatus::Stopped(_, signal)) => {
                if stop_signal.is_none() {
                    stop_signal = Some(signal as i32);
                    let _ = killpg(pgid, Signal::SIGKILL);
                }
            }
            Ok(WaitStatus::Continued(_) | WaitStatus::StillAlive) => {
                thread::sleep(std::time::Duration::from_millis(2));
            }
            #[allow(unreachable_patterns)]
            Ok(_) => {}
            Err(Errno::EINTR) => {}
            Err(error) => {
                wait_error = Some(io::Error::other(error));
                let _ = killpg(pgid, Signal::SIGKILL);
                break;
            }
        }
    }

    if wait_error.is_some() {
        for (_, child) in &mut children {
            let _ = child.wait();
        }
    }
    let restore_error = foreground.as_mut().and_then(|guard| guard.restore().err());
    drop(foreground);

    let capture_result = capture_worker.join().unwrap_or_else(|_| {
        Err(ExecutionError::Collect(io::Error::other(
            "capture worker panicked",
        )))
    });
    let mut downstream_closed = vec![false; child_count];
    for (stage, monitor) in monitor_threads {
        downstream_closed[stage] = monitor.join().unwrap_or(false);
    }

    if let Some(error) = restore_error {
        return Err(ExecutionError::Terminal(error));
    }
    if let Some(error) = wait_error {
        return Err(ExecutionError::Collect(error));
    }
    let bytes = capture_result?;
    if let Some(signal) = stop_signal {
        return Err(ExecutionError::ForegroundStopped { signal });
    }
    if was_cancelled {
        return Err(ExecutionError::Cancelled);
    }

    let mut outcomes = statuses
        .into_iter()
        .enumerate()
        .map(|(stage, status)| {
            let rendered = children[stage].0.clone();
            match status.expect("every child in the pipeline was reaped") {
                ProcessCompletion::Exited(code) => StageOutcome {
                    stage,
                    rendered,
                    code: Some(code),
                    signal: None,
                    success: code == 0,
                },
                ProcessCompletion::Signaled(signal) => StageOutcome {
                    stage,
                    rendered,
                    code: None,
                    signal: Some(signal),
                    success: false,
                },
            }
        })
        .collect::<Vec<_>>();
    for stage in (0..outcomes.len().saturating_sub(1)).rev() {
        if outcomes[stage].signal == Some(13)
            && downstream_closed[stage]
            && outcomes[stage + 1].success
        {
            outcomes[stage].success = true;
        }
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

    let captured = capture.then(|| decode_capture(bytes));
    Ok(ExecutionResult { outcomes, captured })
}

#[cfg(unix)]
fn terminate_group_and_reap(children: &mut [(String, Child)], pgid: Option<nix::unistd::Pid>) {
    use nix::sys::signal::{Signal, killpg};

    if let Some(pgid) = pgid {
        let _ = killpg(pgid, Signal::SIGKILL);
    } else {
        for (_, child) in children.iter_mut() {
            let _ = child.kill();
        }
    }
    for (_, child) in children.iter_mut() {
        let _ = child.wait();
    }
}

fn downstream_closed_before_upstream(writer: PipeWriter, upstream_completed: &AtomicBool) -> bool {
    let mut descriptors = [PollFd::new(writer.as_fd(), PollFlags::POLLOUT)];
    loop {
        if poll(&mut descriptors, 10_u16).is_err() {
            return false;
        }
        if descriptors[0]
            .revents()
            .is_some_and(|events| events.intersects(PollFlags::POLLERR | PollFlags::POLLHUP))
        {
            return true;
        }
        if upstream_completed.load(Ordering::Acquire) {
            return false;
        }
        thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[cfg(not(unix))]
fn kill_process_group(_pid: u32) -> io::Result<()> {
    Ok(())
}

fn expand_glob(pattern: &[u8], cwd: &Path) -> Result<Vec<Vec<u8>>, ExecutionError> {
    let pattern_text =
        std::str::from_utf8(pattern).map_err(|error| ExecutionError::InvalidGlob {
            pattern: String::from_utf8_lossy(pattern).into_owned(),
            message: error.to_string(),
        })?;
    let pattern_path = Path::new(pattern_text);
    let absolute = pattern_path.is_absolute();
    let search_pattern = if absolute {
        pattern_path.to_path_buf()
    } else {
        cwd.join(pattern_path)
    };
    let search_pattern = search_pattern.to_string_lossy();
    let options = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: true,
    };
    let entries =
        glob::glob_with(&search_pattern, options).map_err(|error| ExecutionError::InvalidGlob {
            pattern: pattern_text.to_owned(),
            message: error.to_string(),
        })?;
    let mut matches = entries
        .map(|entry| {
            entry
                .map(|path| {
                    if absolute {
                        path_bytes(path)
                    } else {
                        path.strip_prefix(cwd).map_or_else(
                            |_| path_bytes(path.clone()),
                            |path| path_bytes(path.into()),
                        )
                    }
                })
                .map_err(|error| ExecutionError::InvalidGlob {
                    pattern: pattern_text.to_owned(),
                    message: error.to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    matches.sort();
    if matches.is_empty() {
        return Err(ExecutionError::GlobNoMatch {
            pattern: pattern_text.to_owned(),
        });
    }
    Ok(matches)
}

#[cfg(unix)]
fn path_bytes(path: PathBuf) -> Vec<u8> {
    use std::os::unix::ffi::OsStringExt;
    path.into_os_string().into_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: PathBuf) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(unix)]
fn command_arg(bytes: Vec<u8>) -> Result<OsString, ExecutionError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn command_arg(bytes: Vec<u8>) -> Result<OsString, ExecutionError> {
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|error| ExecutionError::InvalidArgument(error.to_string()))
}

fn resolve(name: &OsStr, snapshot: &ShellSnapshot) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.components().count() > 1 || path.is_absolute() {
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            snapshot.cwd().join(path)
        };
        return is_executable(&candidate).then_some(candidate);
    }
    env::split_paths(
        &snapshot
            .environment_variable(OsStr::new("PATH"))
            .unwrap_or_default(),
    )
    .map(|dir| {
        let candidate = dir.join(name);
        if candidate.is_absolute() {
            candidate
        } else {
            snapshot.cwd().join(candidate)
        }
    })
    .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
pub fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(not(unix))]
fn terminate_and_reap(children: &mut [(String, Child)]) {
    for (_, child) in children.iter_mut() {
        let _ = child.kill();
    }
    for (_, child) in children.iter_mut() {
        let _ = child.wait();
    }
}

#[cfg(not(unix))]
fn outcome(stage: usize, rendered: String, status: ExitStatus) -> StageOutcome {
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    };
    #[cfg(not(unix))]
    let signal = None;
    let success = status.success();
    StageOutcome {
        stage,
        rendered,
        code: status.code(),
        signal,
        success,
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

fn shell_render(value: &OsStr) -> String {
    let text = value.to_string_lossy();
    if text
        .chars()
        .all(|c| c.is_alphanumeric() || "_./-".contains(c))
    {
        text.into_owned()
    } else {
        format!("'{text}'", text = text.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command, time::Instant};

    use josh_runtime::{
        CancellationToken, Captured, ExecutionError, MaterializationLimit, ShellContext,
    };
    use tempfile::tempdir;

    use super::{plan, run_with_cancellation_and_limit};

    #[test]
    fn external_capture_accepts_exact_limit() {
        let planned = plan(vec![vec!["printf".into(), "1234".into()]]).unwrap();
        let result = run_with_cancellation_and_limit(
            &planned,
            true,
            CancellationToken::default(),
            &ShellContext::from_process().snapshot(),
            4,
            #[cfg(unix)]
            None,
        )
        .unwrap();
        assert_eq!(result.captured, Some(Captured::String("1234".into())));
    }

    #[cfg(unix)]
    #[test]
    fn external_capture_overflow_kills_and_reaps_the_producer_group() {
        let temp = tempdir().unwrap();
        let pid_file = temp.path().join("pid");
        let script = format!(
            "printf '%s' $$ > {}; printf 12345; sleep 30",
            pid_file.display()
        );
        let planned = plan(vec![vec!["sh".into(), "-c".into(), script.into()]]).unwrap();
        let started = Instant::now();
        let cancellation = CancellationToken::default();
        let error = run_with_cancellation_and_limit(
            &planned,
            true,
            cancellation.clone(),
            &ShellContext::from_process().snapshot(),
            4,
            #[cfg(unix)]
            None,
        )
        .expect_err("limit plus one");
        assert!(
            !cancellation.is_cancelled(),
            "graph overflow leaked cancellation into the session"
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(matches!(
            error,
            ExecutionError::MaterializationLimit {
                boundary: "external capture",
                limit: MaterializationLimit::Bytes(4)
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
}
