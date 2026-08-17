#!/usr/bin/env bash
# Stdio smoke test for `josh lsp`: stage an LSP exchange through the binary
# and assert the publishDiagnostics payloads. Requires python3 for framing;
# exits nonzero on the first failed assertion.
set -euo pipefail
cd "$(dirname "$0")/.."

# `josh lsp` execs the sibling josh-lsp binary; `cargo test` does not prebuild
# it (no integration test references that bin), so build both explicitly.
cargo build --quiet -p josh-cli -p josh-lsp

JOSH="${JOSH:-target/debug/josh}"
if [ ! -x "$JOSH" ]; then
    echo "lsp-smoke: $JOSH not found; run \`cargo build\` first (or set JOSH=/path/to/josh)" >&2
    exit 2
fi

JOSH="$JOSH" python3 - <<'PY'
import json, os, signal, subprocess, sys

def fail(message):
    print(f"lsp-smoke: {message}", file=sys.stderr)
    sys.exit(1)

def timeout_handler(signum, frame):
    fail("timed out waiting for the server")

signal.signal(signal.SIGALRM, timeout_handler)
signal.alarm(20)

proc = subprocess.Popen(
    [os.environ["JOSH"], "lsp"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
)

def send(message):
    body = json.dumps(message).encode()
    proc.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
    proc.stdin.flush()

def read_message():
    headers = {}
    while True:
        line = proc.stdout.readline().decode().strip()
        if not line:
            break
        key, value = line.split(":", 1)
        headers[key.strip()] = value.strip()
    if "Content-Length" not in headers:
        fail("protocol frame without Content-Length")
    return json.loads(proc.stdout.read(int(headers["Content-Length"])))

URI = "file:///tmp/lsp-smoke.josh"

send({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {"processId": None, "rootUri": None, "capabilities": {}},
})
initialize = read_message()
sync = initialize.get("result", {}).get("capabilities", {}).get("textDocumentSync")
if sync != 1:
    fail(f"expected full-sync capability (1), got {initialize!r}")
if initialize["result"].get("serverInfo", {}).get("name") != "josh-lsp":
    fail(f"unexpected serverInfo: {initialize!r}")

# A UTF-16 multi-byte prefix before the error exercises span conversion:
# `==` spans bytes 11..13 but characters 10..12.
send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
send({
    "jsonrpc": "2.0", "method": "textDocument/didOpen",
    "params": {"textDocument": {
        "uri": URI, "languageId": "josh", "version": 1,
        "text": "let π = 1 == 2\n",
    }},
})
published = read_message()
if published.get("method") != "textDocument/publishDiagnostics":
    fail(f"expected publishDiagnostics, got {published!r}")
diagnostics = published["params"]["diagnostics"]
diagnostic = next((d for d in diagnostics if d.get("code") == "P162"), None)
if diagnostic is None:
    fail(f"no P162 diagnostic in {diagnostics!r}")
expected_range = {"start": {"line": 0, "character": 10}, "end": {"line": 0, "character": 12}}
if diagnostic["range"] != expected_range:
    fail(f"P162 range {diagnostic['range']!r} != {expected_range!r}")
if diagnostic["severity"] != 1 or diagnostic["source"] != "josh":
    fail(f"P162 severity/source wrong: {diagnostic!r}")

# Fixing the source (unclosed quote closed) must clear diagnostics.
send({
    "jsonrpc": "2.0", "method": "textDocument/didChange",
    "params": {
        "textDocument": {"uri": URI, "version": 2},
        "contentChanges": [{"text": "let x = 'closed'\necho ok\n"}],
    },
})
published = read_message()
if published["params"]["diagnostics"] != []:
    fail(f"expected cleared diagnostics, got {published!r}")

send({"jsonrpc": "2.0", "id": 2, "method": "shutdown"})
shutdown = read_message()
if shutdown.get("result", "missing") != None:
    fail(f"unexpected shutdown response: {shutdown!r}")
send({"jsonrpc": "2.0", "method": "exit"})
# The transport loop ends on stdin EOF (clients close the stream after `exit`).
proc.stdin.close()
code = proc.wait()
if code != 0:
    fail(f"server exited with code {code}")

print("lsp-smoke: ok")
PY
