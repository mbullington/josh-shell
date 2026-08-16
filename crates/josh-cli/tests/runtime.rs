use std::{
    ffi::OsString,
    fs,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use josh_exec::{ProcessHost, plan, run};
use josh_runtime::{
    CancellationToken, Captured, CommandSpec, Engine, EngineError, ExecutionError, ExecutionHost,
    ExecutionResult, MaterializationLimit, RunResult, ShellContext, ShellContextError, StreamStage,
    Value,
};
use tempfile::tempdir;

#[test]
fn captures_trim_all_terminal_lf_and_crlf() {
    let planned = plan(vec![vec![
        OsString::from("printf"),
        OsString::from("a\r\n\n"),
    ]])
    .unwrap();
    let result = run(&planned, true).unwrap();
    assert_eq!(result.captured, Some(Captured::String(Arc::from("a"))));
}

#[cfg(unix)]
#[test]
fn capture_retains_invalid_utf8() {
    let planned = plan(vec![vec![
        OsString::from("printf"),
        OsString::from("\\377"),
    ]])
    .unwrap();
    let result = run(&planned, true).unwrap();
    assert_eq!(result.captured, Some(Captured::Bytes(Arc::from([0xff]))));
}

#[test]
fn pipeline_uses_pipefail_and_ignores_nonfinal_sigpipe() {
    let early_failure = plan(vec![
        vec!["sh".into(), "-c".into(), "exit 7".into()],
        vec!["cat".into()],
    ])
    .unwrap();
    let error = run(&early_failure, true).expect_err("pipefail");
    let ExecutionError::PipelineFailed {
        stage, outcomes, ..
    } = error
    else {
        panic!("wrong error")
    };
    assert_eq!(stage, 0);
    assert_eq!(outcomes[0].code, Some(7));

    let downstream_close = plan(vec![
        vec!["yes".into()],
        vec!["head".into(), "-n".into(), "1".into()],
    ])
    .unwrap();
    assert!(run(&downstream_close, true).is_ok());

    let deliberate_sigpipe = plan(vec![
        vec!["sh".into(), "-c".into(), "kill -PIPE $$".into()],
        vec!["cat".into()],
    ])
    .unwrap();
    let error = run(&deliberate_sigpipe, true).expect_err("deliberate SIGPIPE must fail");
    let ExecutionError::PipelineFailed {
        stage, outcomes, ..
    } = error
    else {
        panic!("wrong error")
    };
    assert_eq!(stage, 0);
    assert_eq!(outcomes[0].signal, Some(13));
}

#[test]
fn planning_resolves_every_stage_before_side_effects() {
    let temp = tempdir().unwrap();
    let marker = temp.path().join("spawned");
    let argvs = vec![
        vec![
            "sh".into(),
            "-c".into(),
            format!("touch {}", marker.display()).into(),
        ],
        vec!["josh-command-that-does-not-exist-42".into()],
    ];
    assert!(plan(argvs).is_err());
    assert!(!marker.exists());
}

#[test]
fn engine_capture_commits_only_on_success() {
    let mut engine = Engine::new(ProcessHost::default());
    let value = engine.run_source("x = $(printf 'hello\n\n')").unwrap();
    assert_eq!(value, RunResult::Value(Value::String(Arc::from("hello"))));
    assert_eq!(
        engine
            .run_source("x = $(printf left |\nprintf right)")
            .unwrap(),
        RunResult::Value(Value::String(Arc::from("right")))
    );
    let error = engine
        .run_source("y = $(sh -c 'exit 9')")
        .expect_err("capture failure");
    assert!(matches!(error, EngineError::Uncaught(Value::Error(_))));
    assert!(!engine.variable_names().contains(&"y".to_owned()));
}

#[derive(Debug)]
struct MaterializationFailureHost;

impl ExecutionHost for MaterializationFailureHost {
    fn execute(
        &mut self,
        _commands: Vec<CommandSpec>,
        _capture: bool,
        _cancellation: CancellationToken,
        _context: ShellContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        Err(ExecutionError::MaterializationLimit {
            boundary: "test capture",
            limit: MaterializationLimit::Bytes(4),
        })
    }

    fn execute_stream(
        &mut self,
        _stages: Vec<StreamStage>,
        _capture: bool,
        _cancellation: CancellationToken,
        _context: ShellContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        self.execute(
            Vec::new(),
            true,
            CancellationToken::default(),
            ShellContext::from_process(),
        )
    }

    fn glob(
        &self,
        _pattern: &[u8],
        _context: &ShellContext,
    ) -> Result<Vec<Vec<u8>>, ExecutionError> {
        unreachable!("test does not glob")
    }
}

#[test]
fn materialization_overflow_discards_the_capture_assignment() {
    let mut engine = Engine::new(MaterializationFailureHost);
    let error = engine
        .run_source("kept = \"before\"; partial = $(printf ignored)")
        .expect_err("capture overflow");
    assert!(matches!(
        error,
        EngineError::Process(ExecutionError::MaterializationLimit {
            boundary: "test capture",
            limit: MaterializationLimit::Bytes(4)
        })
    ));
    assert!(!engine.variable_names().contains(&"partial".to_owned()));
    assert_eq!(
        engine.run_source("(kept)").unwrap(),
        RunResult::Value(Value::String(Arc::from("before")))
    );
}

#[test]
fn partial_spawn_failure_terminates_started_children() {
    let mut planned = plan(vec![
        vec!["sleep".into(), "30".into()],
        vec!["printf".into(), "never".into()],
    ])
    .unwrap();
    planned[1].executable = "/definitely/missing/josh-executable".into();
    let started = std::time::Instant::now();
    assert!(run(&planned, false).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
fn builtins_are_dispatched_from_evaluated_argv_and_validate_arity() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(engine.run_source("\"exit\" 7").unwrap(), RunResult::Exit(7));
    assert_eq!(
        engine.run_source("cmd = \"exit\"\n$cmd 9").unwrap(),
        RunResult::Exit(9)
    );
    assert!(matches!(
        engine.run_source("printf x | \"cd\" /"),
        Err(EngineError::Unsupported(_))
    ));
    assert!(matches!(
        engine.run_source("cd / /definitely-not-an-option"),
        Err(EngineError::Type(_))
    ));
    assert!(matches!(
        engine.run_source("exit 1 2"),
        Err(EngineError::Type(_))
    ));
}

#[test]
fn command_conditions_suppress_only_completed_nonzero_statuses() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        engine
            .run_source("if sh -c 'exit 7' { printf no }")
            .unwrap(),
        RunResult::Value(Value::Null)
    );
    assert!(matches!(
        engine.run_source("if josh-command-that-does-not-exist-42 { printf no }"),
        Err(EngineError::Process(ExecutionError::CommandNotFound { .. }))
    ));
    assert!(matches!(
        engine.run_source("if printf $JOSH_DEFINITELY_UNSET_42 { printf no }"),
        Err(EngineError::Undefined { .. })
    ));
}

#[test]
fn integer_arithmetic_errors_are_structured() {
    let mut engine = Engine::new(ProcessHost::default());
    for source in [
        "(1 // 0)",
        "(1 % 0)",
        "(9223372036854775807 + 1)",
        "(-9223372036854775807 - 2)",
        "(9223372036854775807 * 2)",
        "x = (-9223372036854775807 - 1)\n(-x)",
        "x = (-9223372036854775807 - 1)\n(x // -1)",
        "x = (-9223372036854775807 - 1)\n(x % -1)",
        "Number([])",
    ] {
        assert!(
            matches!(engine.run_source(source), Err(EngineError::Type(_))),
            "{source}"
        );
    }
}

#[test]
fn script_like_source_shares_engine_path() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("sample.josh");
    fs::write(&path, "x = $(printf hello)\ny = 1 + 2\n").unwrap();
    let source = fs::read_to_string(path).unwrap();
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        engine.run_source(source).unwrap(),
        RunResult::Value(Value::Int(3))
    );
}

fn evaluated(engine: &mut Engine, source: &str) -> Value {
    let RunResult::Value(value) = engine.run_source(source).unwrap() else {
        panic!("unexpected exit")
    };
    value
}

fn string(value: &str) -> Value {
    Value::String(Arc::from(value))
}

#[test]
fn value_pipelines_stream_expressions_through_closure_stages() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        evaluated(&mut engine, "v = [1, 2, 3] | x => x * 2\n(v)"),
        Value::Array(Arc::new(vec![Value::Int(2), Value::Int(4), Value::Int(6)]))
    );
    assert_eq!(
        evaluated(&mut engine, "v = 5 | x => x * 2\n(v)"),
        Value::Int(10)
    );
    assert_eq!(
        evaluated(&mut engine, "v = $([1, 2, 3] | x => x + 1)\n(v)"),
        Value::Array(Arc::new(vec![Value::Int(2), Value::Int(3), Value::Int(4)]))
    );
    assert_eq!(
        evaluated(&mut engine, "v = [10, 20] | x => x * 3 | take 1\n(v)"),
        Value::Array(Arc::new(vec![Value::Int(30)]))
    );
    assert_eq!(
        evaluated(&mut engine, "v = [1, 4] | x => x * 2 | x => x + 1\n(v)"),
        Value::Array(Arc::new(vec![Value::Int(3), Value::Int(9)]))
    );
    assert_eq!(evaluated(&mut engine, "v = $((5))\n(v)"), Value::Int(5));
    // A bare statement pipeline also leaves its value for the REPL to print.
    assert_eq!(
        evaluated(&mut engine, "[1, 2, 3] | x => x * 2"),
        Value::Array(Arc::new(vec![Value::Int(2), Value::Int(4), Value::Int(6)]))
    );
    // Closure stream stages stay equivalent to `map`.
    assert_eq!(
        evaluated(&mut engine, "v = [1, 2] | x => x + 1\n(v)"),
        evaluated(&mut engine, "v = [1, 2] | map (x => x + 1)\n(v)"),
    );
}

#[test]
fn prototype_namespaces_methods_and_statics_are_first_class() {
    // Regression contract: capitalized namespaces are first-class values whose
    // `.prototype` tables drive method dispatch with the receiver first, plain
    // objects link prototypes through Object.setPrototype, missing members are
    // null, and Object statics operate without receivers.
    let mut engine = Engine::new(ProcessHost::default());
    let value = evaluated(
        &mut engine,
        "p = { greet: (this) => \"hi \" + this.name }; \
         u = { name: \"Ada\" }; Object.setPrototype(u, p); \
         u2 = { name: \"Bob\" }; Object.setPrototype(u2, u); \
         [String(42), Number(\"12\") + 1, Boolean(0), typeof Object.keys({b: 1, a: 2}), \
          u.greet(), u2.greet(), String.prototype.toUpperCase(\"oy\"), \"ab\".missing, \
          Object.isSealed(u), Number.MAX_INT > 0]",
    );
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            string("42"),
            Value::Int(13),
            Value::Bool(false),
            string("array"),
            string("hi Ada"),
            string("hi Bob"),
            string("OY"),
            Value::Null,
            Value::Bool(false),
            Value::Bool(true),
        ]))
    );

    // Own fields shadow prototypes and are called without a receiver.
    let shadow = evaluated(
        &mut engine,
        "o = { double: (x) => x * 2 }; Object.setPrototype(o, { double: (this, x) => x + 1 }); \
         (o.double(21))",
    );
    assert_eq!(shadow, Value::Int(42));

    // Statics and conversions keep their documented shapes.
    let statics = evaluated(
        &mut engine,
        "e = Object.entries({b: 2, a: 1}); f = Object.fromEntries([[\"k\", 9]]); \
         [(e.at(1)).at(0), f.k, Object.keys(Object.create(null)), Array(7).join(\"<\")]",
    );
    assert_eq!(
        statics,
        Value::Array(Arc::new(vec![
            string("a"),
            Value::Int(9),
            Value::Array(Arc::new(Vec::new())),
            string("7"),
        ]))
    );
    // `Object.create(null)` yields no own keys; assert that explicitly.
    let created = evaluated(&mut engine, "(Object.keys(Object.create(null)).length)");
    assert_eq!(created, Value::Int(0));

    // Prototype cycles are rejected.
    let cycle = engine
        .run_source("a = {}; b = {}; Object.setPrototype(a, b); Object.setPrototype(b, a)")
        .unwrap_err();
    assert!(cycle.to_string().contains("cycle"), "{cycle}");

    // `Function()` is not a constructor, and sealed objects reject new members
    // once member assignment lands (M4b); seal state is still observable now.
    let function_call = engine.run_source("Function(x => x)").unwrap_err();
    assert!(
        function_call.to_string().contains("not supported"),
        "{function_call}"
    );
}

#[test]
fn member_assignment_updates_objects_in_place() {
    // Regression contract: statement-level `expr.name = value` and
    // `expr[key] = value` mutate the shared object, sealed objects reject new
    // members, and non-object receivers are type errors.
    let mut engine = Engine::new(ProcessHost::default());
    let value = evaluated(
        &mut engine,
        "o = { a: 1 }; o.b = 2; o.a = 42; o[\"c\"] = 3; \
         alias = o; alias.d = 4; \
         [o.a, o.b, o[\"c\"], o.d, Object.keys(o).join(\",\")]",
    );
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            Value::Int(42),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
            string("a,b,c,d"),
        ]))
    );

    let sealed = engine
        .run_source("s = Object.seal({ a: 1 })\ns.a = 5\ns.new = 9")
        .unwrap_err();
    assert!(sealed.to_string().contains("sealed"), "{sealed}");

    let array_target = engine.run_source("a = [1, 2]\na[0] = 9").unwrap_err();
    assert!(
        array_target.to_string().contains("cannot assign member"),
        "{array_target}"
    );

    // Prototype mutation is visible through every value sharing the table.
    let proto = evaluated(
        &mut engine,
        "String.prototype.shout = (this) => this.toUpperCase(); (\"ok\".shout())",
    );
    assert_eq!(proto, string("OK"));
}

#[test]
fn file_date_and_math_namespaces_cover_the_flattened_surface() {
    let mut engine = Engine::new(ProcessHost::default());
    let value = evaluated(
        &mut engine,
        "[Math.floor(2.7), Math.abs(-3), Math.trunc(2.7), Math.pow(2, 8), \
          Math.max(1, 2.5), Math.min(2, 8), File.exists(\"Cargo.toml\"), \
          typeof File.stat(\"Cargo.toml\"), Date.now() > 0, \
          typeof Date.toLocaleString(0), Math.PI > 3.14]",
    );
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            Value::Float(2.0),
            Value::Int(3),
            Value::Float(2.0),
            Value::Float(256.0),
            Value::Float(2.5),
            Value::Int(2),
            Value::Bool(true),
            string("object"),
            Value::Bool(true),
            string("string"),
            Value::Bool(true),
        ]))
    );
}

#[test]
fn evaluator_polling_unwinds_on_cancellation() {
    // Regression contract for the REPL Ctrl+C path: pure-evaluation spins
    // (loop {}, while without side effects) bail with the shared cancellation
    // error instead of hard-hanging (repl investigation, B1).
    use std::sync::atomic::{AtomicBool, Ordering};
    let flag = Arc::new(AtomicBool::new(false));
    let mut engine = Engine::with_execution_cancellation(ProcessHost::default(), Arc::clone(&flag));
    flag.store(true, Ordering::Relaxed);
    let error = engine.run_source("x = 1").unwrap_err();
    assert!(error.to_string().contains("cancelled"), "{error}");

    for source in [
        "loop { }",
        "x = 0
while x < 1000000000000 { x = x + 1 }",
    ] {
        let flag = Arc::new(AtomicBool::new(false));
        let mut engine =
            Engine::with_execution_cancellation(ProcessHost::default(), Arc::clone(&flag));
        flag.store(true, Ordering::Relaxed);
        let error = engine.run_source(source).unwrap_err();
        assert!(error.to_string().contains("cancelled"), "{source}: {error}");
    }
}

#[test]
fn value_pipeline_parse_and_eval_errors_are_focused() {
    let mut engine = Engine::new(ProcessHost::default());
    let error = engine.run_source("(x => x) | x => x").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("pipeline stage 0 function requires a value stream"),
        "{error}"
    );
    let error = engine.run_source("v = [1] | (5)").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("pipeline stage 1 expression must evaluate to a function"),
        "{error}"
    );
    let error = engine
        .run_source("v = [1, 2] | this-command-does-not-exist-josh")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("command not found: this-command-does-not-exist-josh"),
        "{error}"
    );
}

fn captured(engine: &mut Engine, pipeline: &str) -> Value {
    evaluated(
        engine,
        &format!("captured_value = $({pipeline}); (captured_value)"),
    )
}

#[cfg(unix)]
#[test]
fn tilde_expands_to_session_home_before_globbing() {
    let temp = tempdir().unwrap();
    let context = ShellContext::new(
        temp.path(),
        [(OsString::from("HOME"), temp.path().as_os_str().to_owned())],
    );
    let mut engine = Engine::with_shell_context(ProcessHost::default(), context);
    let home = temp.path().to_string_lossy().into_owned();

    fs::write(temp.path().join("data.joshtilde"), "x").unwrap();
    assert_eq!(
        evaluated(
            &mut engine,
            "result = $(/bin/echo ~ ~/sub ~root \"~\" a~); (result)"
        ),
        string(&format!("{home} {home}/sub ~root ~ a~"))
    );
    assert_eq!(
        evaluated(
            &mut engine,
            "globbed = $(/bin/echo ~/*.joshtilde); (globbed)"
        ),
        string(&format!("{home}/data.joshtilde"))
    );
    engine.run_source("cd ~").unwrap();
    let canonical = temp.path().canonicalize().unwrap();
    assert_eq!(
        evaluated(&mut engine, "cwd = $(/bin/pwd); (cwd)"),
        string(&canonical.to_string_lossy())
    );
}

#[test]
fn if_and_try_are_value_producing_expressions() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        evaluated(&mut engine, "x = if (true) { 1 } else { 2 }; (x)"),
        Value::Int(1)
    );
    assert_eq!(
        evaluated(&mut engine, "x = if (false) { 1 }; (x)"),
        Value::Null
    );
    assert_eq!(
        evaluated(
            &mut engine,
            "x = if (false) { 1 } else if (true) { 2 } else { 3 }; (x)"
        ),
        Value::Int(2)
    );
    assert_eq!(
        evaluated(
            &mut engine,
            "x = if /usr/bin/true { \"ok\" } else { \"no\" }; (x)"
        ),
        string("ok")
    );
    assert_eq!(
        evaluated(&mut engine, "x = 1 + if (true) { 2 } else { 3 }; (x)"),
        Value::Int(3)
    );
    assert_eq!(
        evaluated(&mut engine, "x = try { throw \"boom\" } catch e { e }; (x)"),
        string("boom")
    );
    assert_eq!(
        evaluated(&mut engine, "x = try { 5 } catch e { 0 }; (x)"),
        Value::Int(5)
    );
    assert_eq!(
        evaluated(
            &mut engine,
            "x = try { status = $(/bin/sh -c 'exit 7') } catch e { e.status.code }; (x)"
        ),
        Value::Int(7)
    );
}

#[test]
fn environment_namespace_is_dynamic_exported_and_scalar_canonical() {
    let context = ShellContext::from_process();
    let mut engine = Engine::with_shell_context(ProcessHost::default(), context.clone());
    let value = evaluated(
        &mut engine,
        "env.JOSH_SESSION_TEXT = \"before\"; \
         fn read_env() { return env[\"JOSH_SESSION_TEXT\"] }; \
         fn write_env(value) { env.JOSH_SESSION_TEXT = value }; write_env(\"after\"); \
         env.JOSH_SESSION_INT = 42; env.JOSH_SESSION_FLOAT = 1.5; \
         env.JOSH_SESSION_BOOL = true; \
         child = $(/bin/sh -c 'printf \"%s|%s|%s|%s\" \"$JOSH_SESSION_TEXT\" \"$JOSH_SESSION_INT\" \"$JOSH_SESSION_FLOAT\" \"$JOSH_SESSION_BOOL\"'); \
         fallback = $(/usr/bin/printf '%s' $JOSH_SESSION_TEXT); \
         dynamic = read_env(); before_unset = env.JOSH_SESSION_TEXT; env.JOSH_SESSION_TEXT = null; \
         [dynamic, before_unset, env.JOSH_SESSION_TEXT, child, fallback, typeof env]",
    );
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            string("after"),
            string("after"),
            Value::Null,
            string("after|42|1.5|true"),
            string("after"),
            string("object"),
        ]))
    );
    assert!(
        context
            .environment_variable(std::ffi::OsStr::new("JOSH_SESSION_TEXT"))
            .is_none()
    );
    engine
        .run_source("env.JOSH_FAILED_ASSIGNMENT = \"kept\"")
        .unwrap();
    assert!(
        engine
            .run_source("env.JOSH_FAILED_ASSIGNMENT = $(/bin/sh -c 'exit 7')")
            .is_err()
    );
    assert_eq!(
        evaluated(&mut engine, "env.JOSH_FAILED_ASSIGNMENT"),
        string("kept")
    );

    for source in [
        "env = {}",
        "let env = 1",
        "fn env() { return null }",
        "fn takes(env) { return env }; takes(1)",
        "env.JOSH_BAD_OBJECT = {}",
        "env.JOSH_BAD_ARRAY = []",
    ] {
        assert!(
            matches!(engine.run_source(source), Err(EngineError::Type(_))),
            "{source}"
        );
    }
}

#[cfg(unix)]
#[test]
fn environment_bytes_path_views_and_validation_preserve_os_values() {
    use std::os::unix::ffi::OsStringExt;

    let context = ShellContext::from_process();
    context
        .set_environment_variable("JOSH_NON_UTF8", Some(OsString::from_vec(vec![b'a', 0xff])))
        .unwrap();
    let mut engine = Engine::with_shell_context(ProcessHost::default(), context.clone());
    assert_eq!(
        evaluated(&mut engine, "env.JOSH_NON_UTF8"),
        Value::Bytes(Arc::from([b'a', 0xff]))
    );

    assert_eq!(
        evaluated(
            &mut engine,
            "env.JOSH_NON_UTF8 = $(/usr/bin/printf '\\377'); child_bytes = $(/bin/sh -c 'printf %s \"$JOSH_NON_UTF8\"'); (child_bytes)",
        ),
        Value::Bytes(Arc::from([0xff]))
    );

    assert_eq!(
        evaluated(
            &mut engine,
            "env.PATH = [\"/bin\", \"/usr/bin\"]; [env.PATH, $(/bin/sh -c 'printf %s \"$PATH\"')]",
        ),
        Value::Array(Arc::new(vec![
            Value::Array(Arc::new(vec![string("/bin"), string("/usr/bin")])),
            string("/bin:/usr/bin"),
        ]))
    );
    assert_eq!(
        evaluated(&mut engine, "env.PATH = \"/raw/one:/raw/two\"; env.PATH",),
        Value::Array(Arc::new(vec![string("/raw/one"), string("/raw/two")]))
    );
    assert_eq!(
        evaluated(
            &mut engine,
            "env.PATH = $(/usr/bin/printf '\\377'); env.PATH",
        ),
        Value::Array(Arc::new(vec![Value::Bytes(Arc::from([0xff]))]))
    );

    for source in [
        "env[\"\"] = \"x\"",
        "env[\"BAD=NAME\"] = \"x\"",
        "env.PATH = [\"bad:component\"]",
        "env.PATH = [1]",
        "env.JOSH_NUL = $(/usr/bin/printf '\\0')",
    ] {
        let error = engine.run_source(source).expect_err(source);
        assert!(
            matches!(
                error,
                EngineError::ShellContext(
                    ShellContextError::InvalidEnvironmentName { .. }
                        | ShellContextError::InvalidEnvironmentValue { .. }
                )
            ),
            "{source}: {error}"
        );
    }
    assert_eq!(
        evaluated(&mut engine, "env.PATH"),
        Value::Array(Arc::new(vec![Value::Bytes(Arc::from([0xff]))]))
    );
}

#[cfg(unix)]
#[test]
fn session_cwd_path_globs_redirections_and_stream_functions_share_context() {
    use std::os::unix::fs::PermissionsExt;

    let original_cwd = std::env::current_dir().unwrap();
    let temp = tempdir().unwrap();
    let command = temp.path().join("session-command");
    fs::write(&command, "#!/bin/sh\nprintf session-path").unwrap();
    fs::set_permissions(&command, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(temp.path().join("a.txt"), "a").unwrap();

    let context = ShellContext::from_process();
    let mut engine = Engine::with_shell_context(ProcessHost::default(), context);
    let value = evaluated(
        &mut engine,
        &format!(
            "cd '{}'; env.PATH = [\".\"]; command = $(session-command); \
             cwd = $(/bin/pwd); /usr/bin/printf redirected > output.txt; \
             matches = glob(\"*.txt\"); \
             fn worker(x) {{ return $(/bin/pwd) }}; \
             streamed = $(/usr/bin/printf 'item\\n' | lines | map worker); \
             [command, cwd, matches, streamed]",
            temp.path().display()
        ),
    );
    let cwd = temp
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            string("session-path"),
            string(&cwd),
            Value::Array(Arc::new(vec![string("a.txt"), string("output.txt")])),
            Value::Array(Arc::new(vec![string(&cwd)])),
        ]))
    );
    assert_eq!(
        fs::read(temp.path().join("output.txt")).unwrap(),
        b"redirected"
    );
    assert_eq!(std::env::current_dir().unwrap(), original_cwd);
}

#[test]
fn engines_are_isolated_and_children_receive_only_their_session_snapshot() {
    let original_cwd = std::env::current_dir().unwrap();
    let original_global = std::env::var_os("JOSH_ENGINE_ISOLATION_PROBE");
    let first_dir = tempdir().unwrap();
    let second_dir = tempdir().unwrap();
    let first_context = ShellContext::new(first_dir.path(), Vec::<(OsString, OsString)>::new());
    let second_context = ShellContext::new(second_dir.path(), Vec::<(OsString, OsString)>::new());
    let mut first = Engine::with_shell_context(ProcessHost::default(), first_context);
    let mut second = Engine::with_shell_context(ProcessHost::default(), second_context);

    evaluated(
        &mut first,
        "env.JOSH_ENGINE_ISOLATION_PROBE = \"first\"; cd /; null",
    );
    assert_eq!(
        evaluated(&mut first, "env.JOSH_ENGINE_ISOLATION_PROBE"),
        string("first")
    );
    assert_eq!(
        evaluated(&mut second, "env.JOSH_ENGINE_ISOLATION_PROBE"),
        Value::Null
    );
    assert_eq!(evaluated(&mut second, "env.PATH"), Value::Null);
    assert_eq!(
        captured(
            &mut second,
            "/bin/sh -c 'printf %s \"${HOME-session-unset}\"'",
        ),
        string("session-unset")
    );
    assert_eq!(second.shell_snapshot().cwd(), second_dir.path());
    assert_eq!(std::env::current_dir().unwrap(), original_cwd);
    assert_eq!(
        std::env::var_os("JOSH_ENGINE_ISOLATION_PROBE"),
        original_global
    );
}

#[test]
fn lexical_prompt_functions_validate_arity_result_and_errors() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(engine.prompt().unwrap(), None);
    engine
        .run_source("prefix = \"ready\"; fn prompt() { return prefix + \"> \" }")
        .unwrap();
    assert_eq!(engine.prompt().unwrap().as_deref(), Some("ready> "));

    let mut invalid_value = Engine::new(ProcessHost::default());
    invalid_value
        .run_source("fn prompt() { return 42 }")
        .unwrap();
    assert!(matches!(invalid_value.prompt(), Err(EngineError::Type(_))));

    let mut invalid_arity = Engine::new(ProcessHost::default());
    invalid_arity
        .run_source("fn prompt(value) { return value }")
        .unwrap();
    assert!(matches!(invalid_arity.prompt(), Err(EngineError::Type(_))));

    let mut throwing = Engine::new(ProcessHost::default());
    throwing
        .run_source("fn prompt() { throw \"broken\" }")
        .unwrap();
    assert!(matches!(throwing.prompt(), Err(EngineError::Uncaught(_))));
}

#[test]
fn objects_spread_and_destructuring_preserve_value_order() {
    // Regression contract: object key order is insertion order, overwrites retain their slot,
    // spreads are value copies, and object/array rest patterns bind independent values.
    let mut engine = Engine::new(ProcessHost::default());
    let value = evaluated(
        &mut engine,
        "base = {a: 1, b: 2}; merged = {z: 0, ...base, a: 9}; \
         let {a, ...rest} = merged; let [first, ...tail] = [4, 5, 6]; \
         [a, Object.keys(rest), Object.keys(merged), Object.entries(merged).length, first, tail]",
    );
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            Value::Int(9),
            Value::Array(Arc::new(vec![string("z"), string("b")])),
            Value::Array(Arc::new(vec![string("z"), string("a"), string("b")])),
            Value::Int(3),
            Value::Int(4),
            Value::Array(Arc::new(vec![Value::Int(5), Value::Int(6)])),
        ]))
    );

    let original = Value::Array(Arc::new(vec![Value::Int(1)]));
    let cloned = original.clone();
    let (Value::Array(left), Value::Array(right)) = (&original, &cloned) else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(left, right));
}

#[test]
fn closures_recursion_arrows_spread_calls_and_ufcs_share_one_function_model() {
    // Regression contract: closures capture snapshots, named functions recurse without an Arc
    // cycle, method dispatch prefers builtins, and unknown methods use lexical UFCS.
    let mut engine = Engine::new(ProcessHost::default());
    let value = evaluated(
        &mut engine,
        "x = 10; snapshot = y => x + y; x = 100; \
         fn fact(n) { if (n <= 1) { return 1 } else { return n * fact(n - 1) } }; \
         fn suffix(value, tail) { return value + tail }; \
         fn contains(value, part) { return false }; \
         fn pair([a, b], {tail}) { return a + b + tail }; \
         fn make(base) { return value => base + value }; add = make(7); \
         args = [[1, 2], {tail: 3}]; \
         [snapshot(1), fact(5), add(2), \"x\".suffix(\"!\"), \
          \"abc\".contains(\"b\"), pair(...args), \
          (({tail}, [a, b]) => tail + a + b)({tail: 4}, [5, 6])]",
    );
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            Value::Int(11),
            Value::Int(120),
            Value::Int(9),
            string("x!"),
            Value::Bool(true),
            Value::Int(6),
            Value::Int(15),
        ]))
    );
    assert_eq!(
        evaluated(
            &mut engine,
            "fn command_arg(x) { return x + \"!\" }; command_arg hi"
        ),
        string("hi!")
    );
}

#[test]
fn common_methods_conversions_and_typeof_have_stable_nonmutating_results() {
    // Regression contract: the documented finite method set remains literal, nonmutating, and
    // Unicode-scalar based for string length/at instead of accidentally using UTF-8 bytes.
    let mut engine = Engine::new(ProcessHost::default());
    let value = evaluated(
        &mut engine,
        "xs = [1, 2, 3]; \
         [\"éa\".length, \"éa\".at(0), \"abc\".includes(\"b\"), \
          \"abc\".startsWith(\"a\"), \"abc\".endsWith(\"c\"), \
          \"a,b\".split(\",\").join(\"|\"), \"abba\".replace(\"b\", \"x\"), \
          \"aba\".replaceAll(\"a\", \"x\"), \" Ab \".trim(), \
          \"Ab\".toUpperCase(), \"Ab\".toLowerCase(), \
          xs.map(x => x * 2).join(\",\"), xs.filter(x => x > 1).join(\",\"), \
          xs.reduce((sum, x) => sum + x, 0), [1, [2, [3]]].flat(2).join(\"-\"), \
          xs.slice(1, -1).join(\",\"), xs.includes(2), xs.join(\",\"), \
          Object.entries({b: 2, a: 1}).length, {\"0\": \"zero\"}[0], xs[-1], \
          typeof {a: 1}, String(2), Number(\"3\"), Number(2.5), Boolean(0)]",
    );
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            Value::Int(2),
            string("é"),
            Value::Bool(true),
            Value::Bool(true),
            Value::Bool(true),
            string("a|b"),
            string("axba"),
            string("xbx"),
            string("Ab"),
            string("AB"),
            string("ab"),
            string("2,4,6"),
            string("2,3"),
            Value::Int(6),
            string("1-2-3"),
            string("2"),
            Value::Bool(true),
            string("1,2,3"),
            Value::Int(2),
            string("zero"),
            Value::Int(3),
            string("object"),
            string("2"),
            Value::Int(3),
            Value::Float(2.5),
            Value::Bool(false),
        ]))
    );
}

#[test]
fn typed_unwinding_handles_loops_returns_throws_and_runtime_errors() {
    // Regression contract: one unwind path carries return/break/continue/throw through nested
    // blocks, while catch consumes only throws/errors and preserves structured process status.
    let mut engine = Engine::new(ProcessHost::default());
    let value = evaluated(
        &mut engine,
        "x = 0; loop { x += 1; if (x === 2) { continue }; if (x === 4) { break } }; \
         y = 0; while (y < 3) { y += 1 }; caught = null; \
         try { throw \"boom\" } catch (problem) { caught = problem }; process = null; \
         try { sh -c 'exit 7' } catch (problem) { process = problem }; kind = null; \
         try { (1 + \"bad\") } catch (problem) { kind = problem.kind }; \
         fn early() { try { return 9 } catch (ignored) { return 0 } }; \
         [x, y, caught, process.kind, process.status.code, kind, early()]",
    );
    assert_eq!(
        value,
        Value::Array(Arc::new(vec![
            Value::Int(4),
            Value::Int(3),
            string("boom"),
            string("command"),
            Value::Int(7),
            string("type"),
            Value::Int(9),
        ]))
    );
}

#[test]
fn structured_graph_validation_precedes_spawn_and_byte_functions_have_a_transformer_hint() {
    let temp = tempdir().unwrap();
    let marker = temp.path().join("spawned");
    let mut engine = Engine::new(ProcessHost::default());
    let direct_function = format!(
        "sh -c 'touch {}; printf value' | (x => x)",
        marker.display()
    );
    let error = engine
        .run_source(direct_function)
        .expect_err("bytes to function");
    assert!(
        error
            .to_string()
            .contains("add `lines`, `jsonl`, `json`, `text`, or `chunks(n)`")
    );
    assert!(!marker.exists());

    let invalid_transition = format!(
        "sh -c 'touch {}; printf value' | lines | json",
        marker.display()
    );
    assert!(matches!(
        engine.run_source(invalid_transition),
        Err(EngineError::Type(_))
    ));
    assert!(!marker.exists());

    let capture_in_valid_prefix = format!(
        "printf $(sh -c 'touch {}; printf value') | lines | json",
        marker.display()
    );
    assert!(matches!(
        engine.run_source(capture_in_valid_prefix),
        Err(EngineError::Type(_))
    ));
    assert!(!marker.exists());
}

#[test]
#[test]
fn member_assignment_updates_objects_and_respects_sealed_objects() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        evaluated(
            &mut engine,
            "o = {a: 1}; o.a = 9; o.b = 2; o[\"c\"] = 3; k = \"d\"; o[k] = 4; [o.a, o.b, o.c, o.d]",
        ),
        Value::Array(Arc::new(vec![
            Value::Int(9),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
        ]))
    );
    assert_eq!(
        evaluated(
            &mut engine,
            "o = {a: 1}; Object.seal(o); o.a = 2; [o.a, Object.isSealed(o)]",
        ),
        Value::Array(Arc::new(vec![Value::Int(2), Value::Bool(true)]))
    );
    let error = engine
        .run_source("o = {a: 1}; Object.seal(o); o.b = 2")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("object is sealed; cannot add member `b`"),
        "{error}"
    );
    let error = engine.run_source("x = 5; x.y = 2").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cannot assign member `y` on int"),
        "{error}"
    );
}

#[test]
fn file_date_and_math_namespaces_expose_deterministic_utilities() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        evaluated(
            &mut engine,
            "[Math.floor(2.7), Math.abs(0 - 4), Math.min(3, 1, 2), Math.max(3, 1, 2), Math.trunc(2.5)]",
        ),
        Value::Array(Arc::new(vec![
            Value::Float(2.0),
            Value::Int(4),
            Value::Int(1),
            Value::Int(3),
            Value::Float(2.0),
        ]))
    );
    assert_eq!(
        evaluated(
            &mut engine,
            "[File.exists(\"nope.josh\"), File.exists(\"Cargo.toml\")]"
        ),
        Value::Array(Arc::new(vec![Value::Bool(false), Value::Bool(true)]))
    );
    assert_eq!(
        evaluated(&mut engine, "File.stat(\"Cargo.toml\").kind"),
        string("file")
    );
    assert_eq!(
        evaluated(&mut engine, "File.stat(\".\").kind"),
        string("directory")
    );
    let error = engine.run_source("File.stat(\"nope.josh\")").unwrap_err();
    assert!(error.to_string().contains("No such file"), "{error}");
    assert_eq!(evaluated(&mut engine, "typeof (Date.now())"), string("int"));
    let error = engine
        .run_source("Date.toLocaleString(\"now\")")
        .unwrap_err();
    assert!(error.to_string().contains("expected int"), "{error}");
}

#[test]
fn json_boundaries_preserve_object_insertion_order() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        evaluated(
            &mut engine,
            "value = $(printf '{\"z\":0,\"a\":1,\"m\":2}' | json); Object.keys(value).join(\",\")",
        ),
        string("z,a,m")
    );
    assert_eq!(
        captured(
            &mut engine,
            "printf '{\"z\":0,\"a\":1,\"m\":2}' | json | cat",
        ),
        string("{\"z\":0,\"a\":1,\"m\":2}")
    );
}

#[test]
fn chunks_rejects_oversized_buffers_before_spawn() {
    let temp = tempdir().unwrap();
    let marker = temp.path().join("spawned");
    let mut engine = Engine::new(ProcessHost::default());
    let source = format!(
        "sh -c 'touch {}; printf x' | chunks {}",
        marker.display(),
        josh_runtime::MAX_CHUNK_SIZE + 1
    );
    assert!(matches!(
        engine.run_source(source),
        Err(EngineError::Type(_))
    ));
    assert!(!marker.exists());
    assert_eq!(
        captured(
            &mut engine,
            &format!("printf x | chunks {}", josh_runtime::MAX_CHUNK_SIZE),
        ),
        Value::Array(Arc::new(vec![Value::Bytes(Arc::from(b"x".as_slice()))]))
    );
}

#[test]
fn structured_capture_cardinality_and_decode_policies_are_stable() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        captured(&mut engine, "printf '' | lines"),
        Value::Array(Arc::new(Vec::new()))
    );
    assert_eq!(
        captured(&mut engine, "printf one | lines"),
        Value::Array(Arc::new(vec![string("one")]))
    );
    assert_eq!(captured(&mut engine, "printf 1 | json"), Value::Int(1));
    assert_eq!(
        captured(&mut engine, "printf ab | chunks(8)"),
        Value::Array(Arc::new(vec![Value::Bytes(Arc::from(b"ab".as_slice()))]))
    );
    assert_eq!(
        captured(&mut engine, "printf '' | chunks 8"),
        Value::Array(Arc::new(Vec::new()))
    );
    assert_eq!(
        captured(&mut engine, "printf 'a\\nb\\n' | lines | collect"),
        Value::Array(Arc::new(vec![string("a"), string("b")]))
    );
    assert_eq!(
        captured(&mut engine, "printf '{\"a\":1}'"),
        string("{\"a\":1}")
    );

    #[cfg(unix)]
    assert_eq!(
        captured(&mut engine, "printf '\\377' | text"),
        Value::Bytes(Arc::from([0xff]))
    );
    #[cfg(unix)]
    assert!(
        engine
            .run_source("bad = $(printf '\\377' | lines); (bad)")
            .is_err()
    );
    #[cfg(unix)]
    assert!(
        engine
            .run_source("bad = $(printf '\\377' | json); (bad)")
            .is_err()
    );
    assert!(
        engine
            .run_source("bad = $(printf '1\\nnot-json\\n' | jsonl); (bad)")
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn structured_sigpipe_requires_a_causal_downstream_close() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        captured(&mut engine, "yes | lines | take 1"),
        Value::Array(Arc::new(vec![string("y")]))
    );
    let error = engine
        .run_source("value = $(sh -c 'kill -PIPE $$' | lines)")
        .expect_err("deliberate SIGPIPE must fail");
    assert!(error.to_string().contains("signal 13"));
}

#[test]
fn structured_functions_serialization_and_bounded_termination_cross_real_processes() {
    let mut engine = Engine::new(ProcessHost::default());
    assert_eq!(
        captured(
            &mut engine,
            "printf '1\\n2\\n3\\n' | lines | map (x => Number(x) * 2) | filter (x => x > 2) | take 2"
        ),
        Value::Array(Arc::new(vec![Value::Int(4), Value::Int(6)]))
    );
    assert_eq!(
        captured(
            &mut engine,
            "printf 'a\\n2\\n' | lines | map (x => x === \"a\" ? x : Number(x)) | cat"
        ),
        string("a\n2")
    );
    assert_eq!(
        captured(&mut engine, "printf 'a\\nb\\n' | lines | text"),
        string("a\nb")
    );

    let started = std::time::Instant::now();
    assert_eq!(
        captured(&mut engine, "yes | lines | take 5"),
        Value::Array(Arc::new(vec![string("y"); 5]))
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
fn take_and_first_stop_before_another_function_invocation() {
    let temp = tempdir().unwrap();
    let probe = temp.path().join("invoke.sh");
    fs::write(&probe, "#!/bin/sh\nprintf 'call\\n' >> \"$1\"\n").unwrap();

    for terminal in ["take 1", "first"] {
        let count = temp.path().join(terminal.replace(' ', "-"));
        let mut engine = Engine::new(ProcessHost::default());
        let source = format!(
            "fn inspect(x) {{ sh '{}' '{}'; return x }}; result = $(printf 'a\\nb\\nc\\n' | lines | map inspect | {terminal}); (result)",
            probe.display(),
            count.display()
        );
        let value = evaluated(&mut engine, &source);
        if terminal == "first" {
            assert_eq!(value, string("a"));
        } else {
            assert_eq!(value, Value::Array(Arc::new(vec![string("a")])));
        }
        assert_eq!(fs::read_to_string(count).unwrap(), "call\n");
    }
}

#[test]
fn cancellation_reaches_commands_called_by_stream_functions() {
    let temp = tempdir().unwrap();
    let probe = temp.path().join("block.sh");
    let pid_file = temp.path().join("pid");
    fs::write(&probe, "#!/bin/sh\nprintf '%s' \"$$\" > \"$1\"\nsleep 30\n").unwrap();

    let cancelled = Arc::new(AtomicBool::new(false));
    let trigger = Arc::clone(&cancelled);
    let watched_pid = pid_file.clone();
    let canceller = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !watched_pid.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        trigger.store(true, Ordering::Release);
    });
    let mut engine = Engine::with_execution_cancellation(ProcessHost::default(), cancelled);
    let started = Instant::now();
    let source = format!(
        "fn blocked(x) {{ sh '{}' '{}'; return x }}; result = $(printf 'one\\n' | lines | map blocked)",
        probe.display(),
        pid_file.display()
    );
    assert!(engine.run_source(source).is_err());
    canceller.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));

    let pid = fs::read_to_string(pid_file).unwrap();
    let alive = Command::new("kill")
        .args(["-0", pid.trim()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    assert!(!alive, "cancelled function child {pid} survived");
}

#[test]
fn redirections_preserve_descriptor_order_and_open_before_spawn() {
    let temp = tempdir().unwrap();
    let stdout = temp.path().join("stdout");
    let stderr = temp.path().join("stderr");
    let both = temp.path().join("both");
    let input = temp.path().join("input");
    fs::write(&input, "from-input").unwrap();
    let mut engine = Engine::new(ProcessHost::default());

    engine
        .run_source(format!(
            "printf first > {}; printf second >> {}",
            stdout.display(),
            stdout.display()
        ))
        .unwrap();
    assert_eq!(fs::read(&stdout).unwrap(), b"firstsecond");
    engine
        .run_source(format!(
            "sh -c 'printf err >&2' 2> {}; sh -c 'printf more >&2' 2>> {}",
            stderr.display(),
            stderr.display()
        ))
        .unwrap();
    assert_eq!(fs::read(&stderr).unwrap(), b"errmore");
    assert_eq!(
        captured(&mut engine, &format!("cat < {}", input.display())),
        string("from-input")
    );

    engine
        .run_source(format!(
            "sh -c 'printf out; printf err >&2' > {} 2>&1",
            both.display()
        ))
        .unwrap();
    assert_eq!(fs::read(&both).unwrap(), b"outerr");
    assert_eq!(
        captured(
            &mut engine,
            &format!(
                "sh -c 'printf out; printf err >&2' 2>&1 > {}",
                stdout.display()
            )
        ),
        string("err")
    );
    assert_eq!(fs::read(&stdout).unwrap(), b"out");
    engine
        .run_source(format!(
            "sh -c 'printf out; printf err >&2' &> {}",
            both.display()
        ))
        .unwrap();
    assert_eq!(fs::read(&both).unwrap(), b"outerr");

    assert!(matches!(
        engine.run_source("paths = [\"one\", \"two\"]; printf x > (paths)"),
        Err(EngineError::Type(_))
    ));
    let first_target = temp.path().join("first.target");
    let second_target = temp.path().join("second.target");
    fs::write(&first_target, "first").unwrap();
    fs::write(&second_target, "second").unwrap();
    assert!(matches!(
        engine.run_source(format!("printf x > {}/*.target", temp.path().display())),
        Err(EngineError::Type(_))
    ));
    assert_eq!(fs::read(&first_target).unwrap(), b"first");
    assert_eq!(fs::read(&second_target).unwrap(), b"second");

    let marker = temp.path().join("spawned");
    let unopened = temp.path().join("unopened");
    let missing = "josh-command-that-does-not-exist-redirection-preflight";
    let source = format!(
        "sh -c 'touch {}' > {} | {missing}",
        marker.display(),
        unopened.display()
    );
    assert!(matches!(
        engine.run_source(source),
        Err(EngineError::Process(ExecutionError::CommandNotFound { .. }))
    ));
    assert!(!marker.exists());
    assert!(!unopened.exists());

    let source = format!(
        "sh -c 'touch {}; printf value' > {} | lines | {missing}",
        marker.display(),
        unopened.display()
    );
    assert!(matches!(
        engine.run_source(source),
        Err(EngineError::Process(ExecutionError::CommandNotFound { .. }))
    ));
    assert!(!marker.exists());
    assert!(!unopened.exists());
}

#[test]
fn status_and_command_chains_suppress_only_completed_failures() {
    // Regression contract: completed nonzero outcomes become Status/Error values, but PATH
    // planning and interpolation failures are never mistaken for boolean command statuses.
    let mut engine = Engine::new(ProcessHost::default());
    let status = evaluated(
        &mut engine,
        "fn check() { status sh -c 'exit 7' }; result = check(); \
         [result.success, result.code, result.outcomes.length]",
    );
    assert_eq!(
        status,
        Value::Array(Arc::new(vec![
            Value::Bool(false),
            Value::Int(7),
            Value::Int(1),
        ]))
    );
    assert_eq!(
        evaluated(&mut engine, "sh -c 'exit 7' || true"),
        Value::Null
    );
    assert_eq!(
        evaluated(&mut engine, "sh -c 'exit 7' | lines || true"),
        Value::Null
    );
    let structured_status = evaluated(
        &mut engine,
        "fn structured_check() { status sh -c 'exit 8' | lines }; structured_check().code",
    );
    assert_eq!(structured_status, Value::Int(8));
    assert_eq!(
        evaluated(
            &mut engine,
            "sh -c 'exit 7' && josh-command-that-does-not-exist-42",
        ),
        Value::Null
    );
    assert!(matches!(
        engine.run_source("josh-command-that-does-not-exist-42 || true"),
        Err(EngineError::Process(ExecutionError::CommandNotFound { .. }))
    ));
    assert!(matches!(
        engine.run_source("status josh-command-that-does-not-exist-42"),
        Err(EngineError::Process(ExecutionError::CommandNotFound { .. }))
    ));
    assert!(matches!(
        engine.run_source("printf $JOSH_DEFINITELY_UNSET_42 || true"),
        Err(EngineError::Undefined { .. })
    ));
    assert!(matches!(
        engine.run_source("sh -c 'exit 7'"),
        Err(EngineError::Uncaught(Value::Error(_)))
    ));
}
