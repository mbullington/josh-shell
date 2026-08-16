use std::{env, ffi::OsString, fs, io, path::PathBuf, process::ExitCode, sync::Arc};

use josh_exec::ProcessHost;
use josh_interactive::{print_engine_error, run_repl};
use josh_runtime::{Engine, EngineError, RunResult};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code.clamp(0, 255)).unwrap_or(1)),
        Err(message) => {
            eprintln!("josh: {message}");
            ExitCode::from(2)
        }
    }
}

enum Mode {
    Interactive,
    Command(OsString),
    Script(OsString),
    Help,
    Version,
}

fn run() -> Result<i32, String> {
    let (mode, no_config) = parse_args()?;
    match mode {
        Mode::Help => {
            print_help();
            return Ok(0);
        }
        Mode::Version => {
            println!("josh {}", env!("CARGO_PKG_VERSION"));
            return Ok(0);
        }
        _ => {}
    }

    let interactive = matches!(mode, Mode::Interactive);
    let host = if interactive {
        ProcessHost::interactive()
            .map_err(|error| format!("cannot initialize interactive terminal control: {error}"))?
    } else {
        ProcessHost::default()
    };
    let mut engine = Engine::new(host);
    if !no_config {
        load_startup_files(&mut engine, interactive)?;
    }
    match mode {
        Mode::Interactive => run_repl(&mut engine).map_err(|error| error.to_string()),
        Mode::Command(source) => run_noninteractive(
            &mut engine,
            Arc::<str>::from(source.to_string_lossy().into_owned()),
        ),
        Mode::Script(path) => {
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.to_string_lossy()))?;
            run_noninteractive(&mut engine, Arc::<str>::from(source))
        }
        Mode::Help | Mode::Version => unreachable!("handled before startup"),
    }
}

fn parse_args() -> Result<(Mode, bool), String> {
    let mut args = env::args_os().skip(1).peekable();
    let mut no_config = false;
    while args
        .peek()
        .is_some_and(|argument| argument == "--no-config")
    {
        no_config = true;
        args.next();
    }
    let mode = match args.next() {
        None => Mode::Interactive,
        Some(flag) if flag == "-h" || flag == "--help" => Mode::Help,
        Some(flag) if flag == "-V" || flag == "--version" => Mode::Version,
        Some(flag) if flag == "-c" => Mode::Command(
            args.next()
                .ok_or_else(|| "-c requires a source argument".to_owned())?,
        ),
        Some(flag) if flag.to_string_lossy().starts_with('-') => {
            return Err(format!("unknown option: {}", flag.to_string_lossy()));
        }
        Some(path) => Mode::Script(path),
    };
    if args.next().is_some() {
        return Err(match mode {
            Mode::Command(_) => "unexpected argument after -c source".into(),
            Mode::Script(_) => "scripts do not accept positional arguments in this slice".into(),
            _ => "unexpected argument".into(),
        });
    }
    Ok((mode, no_config))
}

fn load_startup_files(engine: &mut Engine, interactive: bool) -> Result<(), String> {
    let Some(root) = config_root(engine) else {
        return Ok(());
    };
    let directory = root.join("josh");
    let mut files = vec![directory.join("env.josh")];
    if interactive {
        files.push(directory.join("init.josh"));
    }
    for path in files {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) if interactive => {
                eprintln!("josh: cannot read startup file {}: {error}", path.display());
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "cannot read startup file {}: {error}",
                    path.display()
                ));
            }
        };
        match engine.run_source(Arc::<str>::from(source)) {
            Ok(RunResult::Value(_)) => {}
            Ok(RunResult::Exit(code)) if interactive => {
                eprintln!(
                    "josh: startup file {} attempted to exit with status {code}; continuing",
                    path.display()
                );
            }
            Ok(RunResult::Exit(code)) => {
                return Err(format!(
                    "startup file {} attempted to exit with status {code}",
                    path.display()
                ));
            }
            Err(error) if interactive => {
                eprintln!("josh: startup file {} failed; continuing", path.display());
                print_engine_error(&error);
            }
            Err(error) => {
                return Err(format_startup_error(&path, &error));
            }
        }
    }
    Ok(())
}

fn config_root(engine: &Engine) -> Option<PathBuf> {
    engine
        .environment_variable_os("XDG_CONFIG_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            engine
                .environment_variable_os("HOME")
                .filter(|path| !path.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
}

fn format_startup_error(path: &std::path::Path, error: &EngineError) -> String {
    match error {
        EngineError::Parse(diagnostics) => format!(
            "startup file {} failed: {}",
            path.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        ),
        _ => format!("startup file {} failed: {error}", path.display()),
    }
}

fn run_noninteractive(engine: &mut Engine, source: Arc<str>) -> Result<i32, String> {
    match engine.run_source(source) {
        Ok(RunResult::Exit(code)) => Ok(code),
        Ok(RunResult::Value(_)) => Ok(0),
        Err(error) => {
            print_engine_error(&error);
            Ok(1)
        }
    }
}

fn print_help() {
    println!(
        "Josh — JavaScript Object Shell\n\nUsage:\n  josh [--no-config]\n  josh [--no-config] -c <source>\n  josh [--no-config] <script.josh>\n\nOptions:\n  --no-config  Skip env.josh and interactive init.josh startup files\n  -h, --help   Show this help\n  -V, --version\n               Show the version\n\nJosh supports external commands and structured pipelines, redirections and globs,\nvariables, functions/closures/UFCS, and non-job control flow. Jobs and modules are unavailable."
    );
}
