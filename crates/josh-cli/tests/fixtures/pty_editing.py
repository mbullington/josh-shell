#!/usr/bin/env python3
"""Exercise history replacement and prompt clicks through the real REPL."""
import contextlib
import errno
import fcntl
import os
import pathlib
import re
import select
import signal
import struct
import sys
import tempfile
import termios
import time

TIMEOUT = 8.0
PROMPT = b"josh> "
CLICK_PROMPT = b"\x1b]133;A;k=i;click_events=1\x1b\\"
CONTINUATION_PROMPT = b"\x1b]133;A;k=s;click_events=1\x1b\\"
COMMAND_START = b"\x1b]133;C\x1b\\"
COMMAND_END = b"\x1b]133;D\x1b\\"
TITLE = b"\x1b]2;"
CWD_REPORT = b"\x1b]7;file://"


class Session:
    def __init__(self, fd, root, respond_to_cursor_queries=True):
        self.fd = fd
        self.history = root / "history"
        self.output = bytearray()
        self.respond_to_cursor_queries = respond_to_cursor_queries
        self.cursor_queries = 0

    def read(self, needle=None):
        output = bytearray()
        deadline = time.monotonic() + TIMEOUT
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.fd], [], [], 0.1)
            if not ready:
                if output and (needle is None or needle in output):
                    return bytes(output)
                continue
            try:
                chunk = os.read(self.fd, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            output.extend(chunk)
            self.output.extend(chunk)
            # Each read_line starts at row 1 in this simulated terminal.
            queries = output.count(b"\x1b[6n")
            if queries:
                self.cursor_queries += queries
                if self.respond_to_cursor_queries:
                    os.write(self.fd, b"\x1b[1;1R" * queries)
                output[:] = output.replace(b"\x1b[6n", b"")
        if output and (needle is None or needle in output):
            return bytes(output)
        raise AssertionError(f"waiting for {needle!r}: {bytes(self.output)!r}")

    def send(self, data):
        os.write(self.fd, data)
        return self.read()

    def paste(self, text):
        return self.send(b"\x1b[200~" + text.encode() + b"\x1b[201~")

    def submit(self, expected):
        output = self.send(b"\r")
        assert output.index(COMMAND_START) < output.index(COMMAND_END), output
        actual = self.history.read_text().splitlines()[-1].replace("<\\n>", "\n")
        assert actual == expected, (actual, expected, bytes(self.output))

    def click(self, column, row, button=0):
        return self.send(
            f"\x1b[<{button};{column};{row}M\x1b[<{button};{column};{row}m".encode()
        )


@contextlib.contextmanager
def session(josh, width=80, respond_to_cursor_queries=True):
    with tempfile.TemporaryDirectory(prefix="josh-editing-") as directory:
        root = pathlib.Path(directory)
        (root / "history").write_text("echo history-target\necho unrelated-newest\n")
        pid, fd = os.forkpty()
        if pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 24, width, 0, 0))
            environment = os.environ.copy()
            environment.update(
                HOME=directory,
                TERM="xterm-256color",
                JOSH_HISTORY=str(root / "history"),
                JOSH_CARAPACE="0",
            )
            os.execve(josh, [josh, "--no-config"], environment)
        shell = Session(fd, root, respond_to_cursor_queries)
        try:
            shell.read(PROMPT)
            yield shell
        finally:
            os.close(fd)
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.waitpid(pid, 0)


def history_replaces_buffer(josh):
    for before, after, move in [
        ("history-", "target", b""),
        ("history-target", "", b""),
        ("", "history-target", b""),
        ("history-targetTAIL", "", b"\x1b[D" * 4),
    ]:
        with session(josh) as shell:
            if before:
                shell.paste(before)
            if move:
                shell.send(move)
            shell.send(b"\x12")
            if after:
                shell.paste(after)
            original_history = shell.history.read_text()
            shell.send(b"\r")
            assert shell.history.read_text() == original_history
            # Acceptance must leave the command editable, not execute it.
            shell.send(b"\x05")
            shell.paste(" # accepted")
            shell.submit("echo history-target # accepted")


def prompt_clicks_move_cursor(josh):
    for text, column, row, expected, width in [
        ("echo abcd", 14, 1, "echo abXcd", 80),
        ("echo a界b", 14, 1, "echo aX界b", 80),
        ("echo ae\u0301b", 14, 1, "echo ae\u0301Xb", 80),
        ("echo abcdefghijklmnopqrstuvwxyz", 4, 2,
         "echo abcdefghijklXmnopqrstuvwxyz", 20),
        ('echo "ab\ncd"', 7, 2, 'echo "ab\ncXd"', 80),
        ('echo "ab\n  cd"', 9, 2, 'echo "ab\n  cXd"', 80),
        ('echo "ab\na界b"', 9, 2, 'echo "ab\na界Xb"', 80),
        ('echo "ab\nabcdefghijklmnopqr"', 2, 3,
         'echo "ab\nabcdefghijklmnopXqr"', 20),
        ('echo "ab\ncd\nef"', 7, 3, 'echo "ab\ncd\neXf"', 80),
    ]:
        with session(josh, width) as shell:
            assert CLICK_PROMPT in shell.output, bytes(shell.output)
            rendered = shell.paste(text)
            if "\n" in text:
                assert CONTINUATION_PROMPT in rendered, rendered
            # Map a click when the target is after, not just before, the caret.
            shell.send(b"\x1b[1;5H")
            shell.click(column, row)
            shell.paste("X")
            shell.submit(expected)

    with session(josh) as shell:
        shell.paste("echo abcd")
        shell.click(1, 1)  # Prompt text is not editable.
        shell.click(14, 1, button=2)  # Right click must not move the caret.
        shell.click(14, 4)  # Output below the buffer is not editable.
        shell.paste("X")
        shell.submit("echo abcdX")

    with session(josh) as shell:
        shell.paste("echo abcdefghijklmnopqrstuvwxyz")
        fcntl.ioctl(shell.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 20, 0, 0))
        shell.read(PROMPT)
        shell.click(4, 2)
        shell.paste("X")
        shell.submit("echo abcdefghijklXmnopqrstuvwxyz")


def prompt_markers_bound_execution(josh):
    with session(josh) as shell:
        shell.paste("1 + 2")
        output = shell.send(b"\r")
        start = output.index(COMMAND_START)
        end = output.index(COMMAND_END)
        command_output = re.sub(rb"\x1b\[[0-9;]*m", b"", output[start:end])
        assert b"3\r\n" in command_output, output
        assert CLICK_PROMPT in output[end:], output

        shell.paste("no-such-josh-click-test-command")
        output = shell.send(b"\r")
        assert output.index(COMMAND_START) < output.index(b"error:") < output.index(COMMAND_END)

        shell.paste("echo cancelled")
        output = shell.send(b"\x03")
        assert COMMAND_END in output and COMMAND_START not in output, output
        assert CLICK_PROMPT in output, output

        output = shell.send(b"\x04")
        assert COMMAND_END in output and COMMAND_START not in output, output

    with session(josh) as shell:
        shell.paste("exit")
        output = shell.send(b"\r")
        assert output.index(COMMAND_START) < output.index(COMMAND_END), output
        assert CLICK_PROMPT not in output[output.index(COMMAND_END):], output


def missing_cursor_report(josh):
    with session(josh, respond_to_cursor_queries=False) as shell:
        assert shell.cursor_queries == 1, shell.cursor_queries

        shell.paste("1 + 2")
        output = shell.send(b"\r")
        assert b"3\r\n" in re.sub(rb"\x1b\[[0-9;]*m", b"", output) and PROMPT in output, output
        assert shell.cursor_queries == 1, shell.cursor_queries

        # Editing, resize and a cleared screen must not restart the timeout.
        shell.paste("1 + 4")
        shell.send(b"\x1b[D\x1b[3~")
        shell.paste("5")
        output = shell.send(b"\r")
        assert b"6\r\n" in re.sub(rb"\x1b\[[0-9;]*m", b"", output) and PROMPT in output, output
        fcntl.ioctl(shell.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 80, 0, 0))
        shell.read(PROMPT)
        shell.send(b"\x0c")
        assert shell.cursor_queries == 1, shell.cursor_queries

        shell.paste("1 + 7")
        output = shell.send(b"\r")
        assert b"8\r\n" in re.sub(rb"\x1b\[[0-9;]*m", b"", output) and PROMPT in output, output
        assert shell.cursor_queries == 1, shell.cursor_queries

    # The terminal can stop replying after a successful first prompt.
    with session(josh) as shell:
        shell.respond_to_cursor_queries = False
        shell.paste("1 + 2")
        output = shell.send(b"\r")
        assert b"3\r\n" in re.sub(rb"\x1b\[[0-9;]*m", b"", output) and PROMPT in output, output
        assert shell.cursor_queries == 2, shell.cursor_queries
        shell.paste("1 + 7")
        output = shell.send(b"\r")
        assert b"8\r\n" in re.sub(rb"\x1b\[[0-9;]*m", b"", output) and PROMPT in output, output
        assert shell.cursor_queries == 2, shell.cursor_queries


def title_reporting(josh):
    with session(josh) as shell:
        # Defaults are reported before the first read.
        assert TITLE in shell.output, bytes(shell.output)
        assert CWD_REPORT in shell.output, bytes(shell.output)

        # The submitted line is the title while it runs; the idle title
        # and cwd are reported again at the next prompt.
        shell.paste("1 + 2")
        output = shell.send(b"\r")
        assert output.index(TITLE + b"1 + 2\x1b\\") < output.index(COMMAND_START)
        after = output[output.index(COMMAND_END):]
        assert TITLE in after and CWD_REPORT in after, output

        # A lexical title() hook supplies both titles.
        shell.paste(
            'fn title(command) { return if (command === null) '
            '{ "idle-title" } else { "run-title" } }'
        )
        output = shell.send(b"\r")
        after = output[output.index(COMMAND_END):]
        assert TITLE + b"idle-title\x1b\\" in after, output

        shell.paste("1 + 2")
        output = shell.send(b"\r")
        assert output.index(TITLE + b"run-title\x1b\\") < output.index(COMMAND_START)
        after = output[output.index(COMMAND_END):]
        assert TITLE + b"idle-title\x1b\\" in after, output

    with session(josh) as shell:
        shell.paste("cd /")
        output = shell.send(b"\r")
        after = output[output.index(COMMAND_END):]
        assert CWD_REPORT + b"/\x1b\\" in after, output
        assert TITLE + b"/\x1b\\" in after, output


if __name__ == "__main__":
    josh = os.path.abspath(sys.argv[1])
    {
        "history": history_replaces_buffer,
        "clicks": prompt_clicks_move_cursor,
        "markers": prompt_markers_bound_execution,
        "title": title_reporting,
        "missing_cursor_report": missing_cursor_report,
    }[sys.argv[2]](josh)
