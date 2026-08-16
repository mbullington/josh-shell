#!/usr/bin/env python3
import errno
import fcntl
import os
import pathlib
import select
import shutil
import signal
import struct
import sys
import tempfile
import termios
import time

PROMPT = b"josh> "
TIMEOUT = 8.0


def fail(message, output=b""):
    rendered = output.decode("utf-8", "replace")
    raise AssertionError(f"{message}\nPTY output:\n{rendered}")


def read_until(fd, needle, timeout=TIMEOUT):
    deadline = time.monotonic() + timeout
    output = bytearray()
    while time.monotonic() < deadline:
        readable, _, _ = select.select([fd], [], [], 0.05)
        if not readable:
            continue
        try:
            chunk = os.read(fd, 4096)
        except OSError as error:
            if error.errno == errno.EIO:
                break
            raise
        if not chunk:
            break
        output.extend(chunk)
        if b"\x1b[6n" in chunk:
            os.write(fd, b"\x1b[1;1R")
        if needle in output:
            return bytes(output)
    fail(f"timed out waiting for {needle!r}", bytes(output))


def read_command_prompt(fd, timeout=TIMEOUT):
    deadline = time.monotonic() + timeout
    output = bytearray()
    command_finished = False
    while time.monotonic() < deadline:
        readable, _, _ = select.select([fd], [], [], 0.05)
        if not readable:
            continue
        try:
            chunk = os.read(fd, 4096)
        except OSError as error:
            if error.errno == errno.EIO:
                break
            raise
        if not chunk:
            break
        output.extend(chunk)
        if b"\x1b[6n" in chunk:
            os.write(fd, b"\x1b[1;1R")
        if b"\r\n" in output or b"\n" in output:
            command_finished = True
        if command_finished:
            line_end = max(output.rfind(b"\r\n"), output.rfind(b"\n"))
            if PROMPT in output[line_end + 1 :]:
                return bytes(output)
    fail("timed out waiting for a post-command prompt", bytes(output))


def wait_for_file(path, timeout=TIMEOUT):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists() and path.stat().st_size:
            return path.read_text()
        time.sleep(0.02)
    fail(f"timed out waiting for {path}")


def send_line(fd, command):
    os.write(fd, command.encode() + b"\n")


def parse_report(path):
    fields = dict(field.split("=", 1) for field in wait_for_file(path).strip().split())
    return {name: int(value) for name, value in fields.items()}


def assert_foreground_report(path):
    report = parse_report(path)
    if report["pgid"] != report["tpgid"]:
        fail(f"probe did not own its terminal: {report}")
    return report


def process_exists(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def await_gone(pid, timeout=TIMEOUT):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not process_exists(pid):
            return
        time.sleep(0.02)
    fail(f"process {pid} survived foreground cleanup")


def assert_shell_owns_terminal(fd, shell_pid, output=b""):
    owner = os.tcgetpgrp(fd)
    shell_group = os.getpgid(shell_pid)
    if owner != shell_group:
        fail(f"terminal owner {owner} was not restored to Josh group {shell_group}", output)


def run():
    if len(sys.argv) != 3:
        raise SystemExit("usage: pty_foreground.py JOSH PROBE")
    josh = os.path.abspath(sys.argv[1])
    probe = os.path.abspath(sys.argv[2])
    with tempfile.TemporaryDirectory(prefix="josh-pty-") as root_text:
        root = pathlib.Path(root_text)
        pid, fd = os.forkpty()
        if pid == 0:
            environment = os.environ.copy()
            environment.update(
                {
                    "TERM": "xterm-256color",
                    "HOME": root_text,
                    "JOSH_HISTORY": str(root / "history"),
                }
            )
            os.execve(josh, [josh, "--no-config"], environment)

        transcript = bytearray()
        try:
            transcript.extend(read_until(fd, PROMPT))
            assert_shell_owns_terminal(fd, pid)

            producer = root / "producer.report"
            consumer = root / "consumer.report"
            send_line(fd, f"{probe} producer {producer} | {probe} consumer {consumer}")
            transcript.extend(read_command_prompt(fd))
            first = assert_foreground_report(producer)
            second = assert_foreground_report(consumer)
            if first["pgid"] != second["pgid"]:
                fail(f"pipeline stages used different process groups: {first}, {second}")
            assert_shell_owns_terminal(fd, pid, bytes(transcript))

            dirty = root / "dirty.report"
            modes = root / "modes.report"
            send_line(fd, f"{probe} dirty-terminal {dirty}")
            transcript.extend(read_command_prompt(fd))
            send_line(fd, f"{probe} terminal-modes {modes}")
            mode_output = read_command_prompt(fd)
            transcript.extend(mode_output)
            if b" echo " not in mode_output.replace(b"-echo", b""):
                fail("Josh did not restore terminal echo after the child exited", mode_output)
            assert_shell_owns_terminal(fd, pid)

            interrupt = root / "interrupt.report"
            resized = root / "resized.report"
            send_line(fd, f"{probe} interrupt {interrupt} {resized}")
            running = assert_foreground_report(interrupt)
            fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 41, 101, 0, 0))
            wait_for_file(resized)
            os.write(fd, b"\x03")
            transcript.extend(read_command_prompt(fd))
            await_gone(running["pid"])
            assert_shell_owns_terminal(fd, pid)

            stopped = root / "stopped.report"
            send_line(fd, f"{probe} stop {stopped}")
            stopped_process = assert_foreground_report(stopped)
            stopped_output = read_command_prompt(fd)
            transcript.extend(stopped_output)
            if b"suspended jobs are not supported" not in stopped_output:
                fail("stopped foreground pipeline was not diagnosed", stopped_output)
            await_gone(stopped_process["pid"])
            assert_shell_owns_terminal(fd, pid)

            document = root / "screen.txt"
            document.write_text("josh-screen-probe\n")
            less = shutil.which("less")
            if less:
                send_line(fd, f"{less} {document}")
                transcript.extend(read_until(fd, b"josh-screen-probe"))
                os.write(fd, b"q")
                transcript.extend(read_until(fd, PROMPT))
                assert_shell_owns_terminal(fd, pid)

            vim = shutil.which("vim")
            if vim:
                send_line(fd, f"{vim} -Nu NONE -n {document}")
                transcript.extend(read_until(fd, b"josh-screen-probe"))
                os.write(fd, b":q!\r")
                transcript.extend(read_until(fd, PROMPT))
                assert_shell_owns_terminal(fd, pid)

            time.sleep(0.1)
            send_line(fd, "exit")
            deadline = time.monotonic() + TIMEOUT
            while time.monotonic() < deadline:
                readable, _, _ = select.select([fd], [], [], 0.02)
                if readable:
                    try:
                        chunk = os.read(fd, 4096)
                        transcript.extend(chunk)
                        if b"\x1b[6n" in chunk:
                            os.write(fd, b"\x1b[1;1R")
                    except OSError as error:
                        if error.errno != errno.EIO:
                            raise
                waited, status = os.waitpid(pid, os.WNOHANG)
                if waited == pid:
                    if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                        fail(f"Josh exited with wait status {status}", bytes(transcript))
                    return
                time.sleep(0.02)
            fail("Josh did not exit after the exit builtin", bytes(transcript))
        finally:
            try:
                os.close(fd)
            except OSError:
                pass
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                os.waitpid(pid, 0)
            except ChildProcessError:
                pass


if __name__ == "__main__":
    try:
        run()
    except Exception as error:
        print(error, file=sys.stderr)
        raise
