use std::{fs, process::Command};
use tempfile::tempdir;

fn josh() -> Command {
    Command::new(env!("CARGO_BIN_EXE_josh"))
}

fn josh_no_config() -> Command {
    let mut command = josh();
    command.arg("--no-config");
    command
}

#[test]
fn command_and_script_modes_share_observable_behavior() {
    let command = josh_no_config()
        .args(["-c", "printf hello | tr a-z A-Z"])
        .output()
        .unwrap();
    assert!(command.status.success());
    assert_eq!(command.stdout, b"HELLO");

    let temp = tempdir().unwrap();
    let script = temp.path().join("smoke.josh");
    fs::write(&script, "printf hello | tr a-z A-Z\n").unwrap();
    let scripted = josh_no_config().arg(script).output().unwrap();
    assert!(scripted.status.success());
    assert_eq!(scripted.stdout, b"HELLO");
}

#[test]
fn batch_errors_are_nonzero_and_exit_status_is_honored() {
    let parse_error = josh_no_config().args(["-c", "echo |"]).output().unwrap();
    assert_eq!(parse_error.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&parse_error.stderr).contains("pipeline cannot end"));

    let command_error = josh_no_config()
        .args(["-c", "sh -c 'exit 9'"])
        .output()
        .unwrap();
    assert_eq!(command_error.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&command_error.stderr).contains("exit 9"));

    let explicit_exit = josh_no_config().args(["-c", "exit 7"]).status().unwrap();
    assert_eq!(explicit_exit.code(), Some(7));
}

#[test]
fn unquoted_globs_are_sorted_loud_and_quote_aware() {
    let temp = tempdir().unwrap();
    fs::create_dir_all(temp.path().join("nested/deep")).unwrap();
    for path in [
        "z.txt",
        "a.txt",
        "space name.txt",
        "*a.mix",
        "nested/b.txt",
        "nested/deep/c.txt",
    ] {
        fs::write(temp.path().join(path), "").unwrap();
    }

    let expanded = josh_no_config()
        .current_dir(temp.path())
        .args(["-c", "printf '%s\\n' *.txt"])
        .output()
        .unwrap();
    assert!(expanded.status.success());
    assert_eq!(expanded.stdout, b"a.txt\nspace name.txt\nz.txt\n");

    let quoted = josh_no_config()
        .current_dir(temp.path())
        .args(["-c", "printf '%s' '*.txt'"])
        .output()
        .unwrap();
    assert!(quoted.status.success());
    assert_eq!(quoted.stdout, b"*.txt");

    let bracket_class = josh_no_config()
        .current_dir(temp.path())
        .args(["-c", "printf '%s\\n' [az].txt"])
        .output()
        .unwrap();
    assert!(bracket_class.status.success());
    assert_eq!(bracket_class.stdout, b"a.txt\nz.txt\n");

    let mixed_quote = josh_no_config()
        .current_dir(temp.path())
        .args(["-c", "printf '%s' \"*\"?.mix"])
        .output()
        .unwrap();
    assert!(mixed_quote.status.success());
    assert_eq!(mixed_quote.stdout, b"*a.mix");

    let escaped = josh_no_config()
        .current_dir(temp.path())
        .args(["-c", "printf '%s' \\*.txt"])
        .output()
        .unwrap();
    assert!(escaped.status.success());
    assert_eq!(escaped.stdout, b"*.txt");

    let recursive = josh_no_config()
        .current_dir(temp.path())
        .args(["-c", "files = glob(\"**/*.txt\"); printf '%s\\n' $files"])
        .output()
        .unwrap();
    assert!(recursive.status.success());
    assert_eq!(
        recursive.stdout,
        b"a.txt\nnested/b.txt\nnested/deep/c.txt\nspace name.txt\nz.txt\n"
    );

    let no_match = josh_no_config()
        .current_dir(temp.path())
        .args(["-c", "printf '%s' no-match-*.txt"])
        .output()
        .unwrap();
    assert_eq!(no_match.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&no_match.stderr).contains("matched no paths"));
}

#[test]
fn startup_files_have_session_scope_order_and_batch_error_policy() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("josh");
    fs::create_dir(&config).unwrap();
    fs::write(config.join("env.josh"), "startup = \"env\"\n").unwrap();
    fs::write(config.join("init.josh"), "startup += \"-init\"\n").unwrap();

    let batch = josh()
        .env("XDG_CONFIG_HOME", temp.path())
        .args(["-c", "printf '%s' $startup"])
        .output()
        .unwrap();
    assert!(batch.status.success());
    assert_eq!(batch.stdout, b"env");

    let disabled = josh()
        .env("XDG_CONFIG_HOME", temp.path())
        .args(["--no-config", "-c", "printf '%s' $startup"])
        .output()
        .unwrap();
    assert_eq!(disabled.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&disabled.stderr).contains("undefined identifier `startup`"));

    fs::write(config.join("env.josh"), "broken =\n").unwrap();
    let invalid = josh()
        .env("XDG_CONFIG_HOME", temp.path())
        .args(["-c", "true"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("startup file"));

    let missing = josh()
        .env("XDG_CONFIG_HOME", temp.path().join("missing"))
        .args(["-c", "true"])
        .status()
        .unwrap();
    assert!(missing.success());

    let home = temp.path().join("home");
    let home_config = home.join(".config/josh");
    fs::create_dir_all(&home_config).unwrap();
    fs::write(home_config.join("env.josh"), "fallback = \"home\"\n").unwrap();
    let fallback = josh()
        .env_remove("XDG_CONFIG_HOME")
        .env("HOME", &home)
        .args(["-c", "printf '%s' $fallback"])
        .output()
        .unwrap();
    assert!(fallback.status.success());
    assert_eq!(fallback.stdout, b"home");
}

#[cfg(unix)]
#[test]
fn startup_environment_mutation_persists_for_path_lookup_and_children() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let config = temp.path().join("josh");
    let bin = temp.path().join("bin");
    fs::create_dir(&config).unwrap();
    fs::create_dir(&bin).unwrap();
    let executable = bin.join("startup-command");
    fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s' \"$JOSH_FROM_STARTUP\"\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        config.join("env.josh"),
        format!(
            "env.JOSH_FROM_STARTUP = \"exported\"\nenv.PATH = [...env.PATH, \"{}\"]\n",
            bin.display()
        ),
    )
    .unwrap();

    let configured = josh()
        .env("XDG_CONFIG_HOME", temp.path())
        .args(["-c", "startup-command"])
        .output()
        .unwrap();
    assert!(
        configured.status.success(),
        "{}",
        String::from_utf8_lossy(&configured.stderr)
    );
    assert_eq!(configured.stdout, b"exported");

    let disabled = josh()
        .env("XDG_CONFIG_HOME", temp.path())
        .args(["--no-config", "-c", "startup-command"])
        .output()
        .unwrap();
    assert!(!disabled.status.success());
}

#[test]
fn excluded_jobs_and_module_surfaces_remain_unavailable() {
    for source in [
        "sleep 0 &",
        "jobs",
        "fg",
        "bg",
        "source file.josh",
        "import package",
        "export value",
        "import https://example.invalid/module.josh",
    ] {
        let output = josh_no_config().args(["-c", source]).output().unwrap();
        assert!(
            !output.status.success(),
            "excluded syntax unexpectedly ran: {source}"
        );
    }
}

#[cfg(unix)]
#[test]
fn inherited_stdout_broken_pipe_is_graceful() {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
        thread,
        time::{Duration, Instant},
    };

    let mut child = josh_no_config()
        .args(["-c", "yes | lines | text"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut output)
        .unwrap();
    assert_eq!(output, "y\n");

    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("Josh did not stop after inherited stdout closed");
        }
        thread::sleep(Duration::from_millis(5));
    };
    assert!(
        status.success(),
        "Josh treated inherited BrokenPipe as failure"
    );
}

#[cfg(unix)]
#[test]
fn configured_prompt_is_visible_through_a_pty() {
    use std::{
        io::{Read, Write},
        os::fd::AsFd,
        process::Stdio,
        time::{Duration, Instant},
    };

    use nix::{
        poll::{PollFd, PollFlags, poll},
        pty::openpty,
    };

    let temp = tempdir().unwrap();
    let config = temp.path().join("josh");
    fs::create_dir(&config).unwrap();
    fs::write(
        config.join("env.josh"),
        "prefix = \"env\"\njosh-config-command-that-does-not-exist\n",
    )
    .unwrap();
    fs::write(
        config.join("init.josh"),
        "prefix += \"-init\"\nfn prompt() { return prefix + \"> \" }\n",
    )
    .unwrap();

    let pty = openpty(None, None).unwrap();
    let mut master = fs::File::from(pty.master);
    let slave = fs::File::from(pty.slave);
    let mut child = josh()
        .env("XDG_CONFIG_HOME", temp.path())
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave))
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = Vec::new();
    while Instant::now() < deadline
        && !output
            .windows(b"env-init> ".len())
            .any(|x| x == b"env-init> ")
    {
        let mut descriptors = [PollFd::new(master.as_fd(), PollFlags::POLLIN)];
        if poll(&mut descriptors, 100_u16).unwrap() > 0 {
            let mut buffer = [0_u8; 1024];
            if let Ok(count) = master.read(&mut buffer) {
                output.extend_from_slice(&buffer[..count]);
                if buffer[..count]
                    .windows(4)
                    .any(|window| window == b"\x1b[6n")
                {
                    master.write_all(b"\x1b[1;1R").unwrap();
                }
            }
        }
    }
    assert!(
        output
            .windows(b"env-init> ".len())
            .any(|x| x == b"env-init> "),
        "PTY output did not contain configured prompt: {}",
        String::from_utf8_lossy(&output)
    );
    assert!(
        String::from_utf8_lossy(&output).contains("startup file")
            && String::from_utf8_lossy(&output).contains("continuing"),
        "interactive startup error was not diagnosed: {}",
        String::from_utf8_lossy(&output)
    );
    master.write_all(&[4]).unwrap();
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_a_structured_graph_and_reaps_its_child() {
    use std::{
        io::{Read, Write},
        os::fd::AsFd,
        process::Stdio,
        thread,
        time::{Duration, Instant},
    };

    use nix::{
        poll::{PollFd, PollFlags, poll},
        sys::signal::{Signal, kill, killpg},
        unistd::Pid,
    };

    let temp = tempdir().unwrap();
    let pid_file = temp.path().join("producer.pid");
    let pty = nix::pty::openpty(None, None).unwrap();
    let mut master = fs::File::from(pty.master);
    let slave = fs::File::from(pty.slave);
    let mut shell = josh_no_config()
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave))
        .spawn()
        .unwrap();

    fn read_available(master: &mut fs::File, output: &mut Vec<u8>) {
        let mut descriptors = [PollFd::new(master.as_fd(), PollFlags::POLLIN)];
        if poll(&mut descriptors, 25_u16).unwrap() > 0 {
            let mut buffer = [0_u8; 2048];
            if let Ok(count) = master.read(&mut buffer) {
                output.extend_from_slice(&buffer[..count]);
                if buffer[..count]
                    .windows(4)
                    .any(|window| window == b"\x1b[6n")
                {
                    master.write_all(b"\x1b[1;1R").unwrap();
                }
            }
        }
    }

    let mut output = Vec::new();
    let prompt_deadline = Instant::now() + Duration::from_secs(3);
    while !output.windows(6).any(|window| window == b"josh> ") && Instant::now() < prompt_deadline {
        read_available(&mut master, &mut output);
    }
    let command = format!(
        "sh -c 'echo $$ > {}; while true; do printf x; sleep 1; done' | lines | map (x => x)\n",
        pid_file.display()
    );
    master.write_all(command.as_bytes()).unwrap();

    let start_deadline = Instant::now() + Duration::from_secs(3);
    while !pid_file.exists() && Instant::now() < start_deadline {
        read_available(&mut master, &mut output);
    }
    let producer_pid = fs::read_to_string(&pid_file)
        .ok()
        .and_then(|pid| pid.trim().parse::<i32>().ok());
    kill(
        Pid::from_raw(i32::try_from(shell.id()).unwrap()),
        Signal::SIGINT,
    )
    .unwrap();

    let interrupt_deadline = Instant::now() + Duration::from_secs(3);
    let mut interrupted = Vec::new();
    while !(interrupted
        .windows(b"pipeline was cancelled".len())
        .any(|window| window == b"pipeline was cancelled")
        && interrupted.windows(6).any(|window| window == b"josh> "))
        && Instant::now() < interrupt_deadline
    {
        read_available(&mut master, &mut interrupted);
    }
    let producer_alive = producer_pid.is_some_and(|pid| {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    });

    master.write_all(&[4]).unwrap();
    let exit_deadline = Instant::now() + Duration::from_secs(2);
    while shell.try_wait().unwrap().is_none() && Instant::now() < exit_deadline {
        thread::sleep(Duration::from_millis(5));
    }
    if shell.try_wait().unwrap().is_none() {
        shell.kill().unwrap();
    }
    let _ = shell.wait();
    if producer_alive && let Some(pid) = producer_pid {
        let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
    }

    assert!(producer_pid.is_some(), "structured producer never started");
    assert!(
        interrupted
            .windows(b"pipeline was cancelled".len())
            .any(|window| window == b"pipeline was cancelled")
            && interrupted.windows(6).any(|window| window == b"josh> "),
        "Josh did not return to its prompt after Ctrl-C: {}",
        String::from_utf8_lossy(&interrupted)
    );
    assert!(!producer_alive, "structured producer survived Ctrl-C");
}

#[cfg(unix)]
#[test]
fn foreground_pipelines_own_and_restore_the_pty() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let driver = fixtures.join("pty_foreground.py");
    let probe = fixtures.join("foreground_probe.sh");

    let output = Command::new("python3")
        .args([
            driver.as_os_str(),
            std::ffi::OsStr::new(env!("CARGO_BIN_EXE_josh")),
            probe.as_os_str(),
        ])
        .output()
        .expect("python3 is required for Unix PTY integration tests");
    assert!(
        output.status.success(),
        "PTY foreground driver failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
