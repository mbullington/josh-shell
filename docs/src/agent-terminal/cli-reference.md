# agent-terminal CLI reference

<a id="AT-CLI-001"></a>
## Command surface

| Command | Source-defined usage | Output |
|---|---|---|
| `launch` | `launch [--cols N] [--rows N] [--cwd PATH] -- COMMAND [ARG...]` | Session ID |
| `snapshot` | `snapshot [SESSION] [--json]` | Metadata + text, or schema v2 JSON |
| `screenshot` | `screenshot [SESSION] [PATH] [--json]` | Output path, or render-result JSON |
| `type` | `type [SESSION] TEXT` | `revision=N` |
| `key` | `key [SESSION] CHORD` | `revision=N` |
| `resize` | `resize [SESSION] COLS ROWS` | `revision=N` |
| `wait` | `wait [SESSION] (--text TEXT \| --stable DURATION) [--timeout DURATION] [--json]` | Session/revision/process, or metadata JSON |
| `list` | `list [--json]` | Handshaken sessions |
| `close` | `close [SESSION]` | No success output |

`launch` defaults to 80×24 and the caller's cwd. Grids are limited to 300 columns, 200 rows, and 20,000 total cells. Durations accept bare milliseconds or `ms`, `s`, and `m` suffixes. `wait` requires exactly one of `--text` and `--stable` and defaults to 5 seconds. Timeout exits 124; other client/protocol errors exit 1.

`screenshot` gets the semantic snapshot, renders in the invoking client, and writes PATH. If PATH is absent, it uses the platform temporary directory's `agent-terminal-SESSION.png`. The non-JSON result is that path. JSON includes `path`, `session_id`, semantic `revision`, `width`, `height`, encoded `bytes`, `dpi: 96`, `cell_width: 8`, and `cell_height: 16`. See [Deterministic PNG screenshots](screenshots.md#AT-PNG-001) for rendering and overwrite rules.

Session selection accepts a full 32-hexadecimal ID, with or without UUID hyphens, or a unique hexadecimal prefix of at least eight characters. An explicit selector may identify a starting, running, or exited healthy daemon. An omitted selector requires exactly one running or exited healthy session. Read-only failure paths do not auto-start a daemon.
