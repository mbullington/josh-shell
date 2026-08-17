# agent-terminal security

<a id="AT-SEC-001"></a>
## Local trust boundary

Launching a command grants ordinary local code-execution authority. agent-terminal is not a sandbox. Never expose its control socket through TCP, a web service, or an untrusted same-user boundary.

The runtime directory comes from explicit `AGENT_TERMINAL_RUNTIME_DIR`, then `$XDG_RUNTIME_DIR/agent-terminal`, then a short `$TMPDIR/agent-terminal-$UID` path. Runtime and session directories must be real owner-owned 0700 directories, not symlinks; sockets and metadata are 0600. Socket paths are length-checked.

Protocol versioning prevents accidental schema confusion but does not authenticate beyond filesystem permissions. Requests are capped at 1 MiB and responses at 16 MiB. Grids are capped at 300 columns, 200 rows, and 20,000 cells; style tables at 1,024 unique entries; hyperlinked cells at 1,024; graphemes at 256 bytes; hyperlinks at 2,048 bytes; formatter text at 256 KiB; and all snapshot strings at 512 KiB aggregate. These limits fit the response budget by construction. Borrowed Ghostty data is copied before mutation or await. Clients validate request IDs, handshake session/Ghostty identity, snapshot session/schema, complete row-major coverage, cursor and wide-cell invariants, and all bounds before allocating at most 10,240,000 raw RGBA bytes. Malformed state is rejected rather than clamped or painted.

An explicit screenshot path uses the client's ordinary filesystem authority. The write truncates an existing file, follows normal platform symlink semantics, and does not create parent directories. The default path is predictable from the session ID in the platform temporary directory. Choose a protected explicit directory when terminal pixels are sensitive, and do not treat a successful render as safe publication.

Close/reap handling is lifecycle hygiene, not hostile-process containment. A process can ignore HUP, fork, or establish a new session. Stronger guarantees require cgroups, process contracts, or another OS containment mechanism.
